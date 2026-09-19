//! Resolve static Luau payload types without running game code.
use super::sourcemap::Node;
use super::types::{Field, Leaf, NetworkPayloadType as Type, is_class, is_enum};
use anyhow::{Context, Result, anyhow, bail};
use full_moon::ast::luau::{IndexedTypeInfo, TypeDeclaration, TypeFieldKey, TypeInfo};
use full_moon::ast::{Ast, Call, Expression, FunctionArgs, Index, Prefix, Stmt, Suffix, Var};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;

#[derive(Default)]
struct Declarations {
    aliases: BTreeMap<String, (Rc<TypeDeclaration>, bool)>,
    locals: BTreeMap<String, Expression>,
    bound: BTreeSet<String>,
}

pub struct Resolver<'a> {
    root: &'a Path,
    map: &'a Node,
    modules: BTreeMap<PathBuf, Declarations>,
    pub dependencies: BTreeMap<PathBuf, String>,
}

impl<'a> Resolver<'a> {
    pub fn new(root: &'a Path, map: &'a Node) -> Self {
        Self {
            root,
            map,
            modules: BTreeMap::new(),
            dependencies: BTreeMap::new(),
        }
    }

    pub fn add(&mut self, path: &Path, ast: &Ast) -> Result<()> {
        let mut module = Declarations::default();
        let mut ambiguous = BTreeSet::new();
        for stmt in ast.nodes().stmts() {
            let alias = match stmt {
                Stmt::TypeDeclaration(alias) => Some((alias, false)),
                Stmt::ExportedTypeDeclaration(alias) => Some((alias.type_declaration(), true)),
                Stmt::LocalAssignment(local) => {
                    let mut values = local.expressions().iter();
                    for name in local.names().iter() {
                        let name = name.token().to_string();
                        if !module.bound.insert(name.clone()) {
                            ambiguous.insert(name.clone());
                        }
                        if let Some(expr) = values.next() {
                            module.locals.insert(name, expr.clone());
                        }
                    }
                    None
                }
                Stmt::Assignment(assignment) => {
                    for variable in assignment.variables().iter() {
                        if let Var::Name(name) = variable {
                            ambiguous.insert(name.token().to_string());
                        }
                    }
                    None
                }
                _ => None,
            };
            if let Some((alias, exported)) = alias {
                let name = alias.type_name().token().to_string();
                if Leaf::parse(&name).is_some() || is_class(&name) || name == "Enum" {
                    bail!(
                        "{}: alias {name} shadows a built-in payload type",
                        path.display()
                    );
                }
                if module
                    .aliases
                    .insert(name.clone(), (Rc::new(alias.clone()), exported))
                    .is_some()
                {
                    bail!("{}: duplicate type alias {name}", path.display());
                }
            }
        }
        for name in ambiguous {
            module.locals.remove(&name);
        }
        self.modules.insert(path.to_path_buf(), module);
        Ok(())
    }

    // Load reachable imports before borrowing their syntax nodes for resolution.
    fn prepare(
        &mut self,
        path: &Path,
        ty: &TypeInfo,
    ) -> Result<BTreeMap<(PathBuf, String), PathBuf>> {
        let mut imports = BTreeMap::new();
        let mut aliases = Vec::new();
        self.scan_imports(path, ty, &mut imports, &mut aliases)?;
        let mut visited = BTreeSet::new();
        while let Some((path, name, exported)) = aliases.pop() {
            let Some((alias, public)) = self.modules[&path].aliases.get(&name).cloned() else {
                continue; // Resolution reports unknown aliases with their context.
            };
            if alias.generics().is_none()
                && (!exported || public)
                && visited.insert((path.clone(), name.clone()))
            {
                self.scan_imports(&path, alias.type_definition(), &mut imports, &mut aliases)
                    .with_context(|| format!("in alias {name} ({})", path.display()))?;
            }
        }
        Ok(imports)
    }

    fn scan_imports(
        &mut self,
        path: &Path,
        ty: &TypeInfo,
        imports: &mut BTreeMap<(PathBuf, String), PathBuf>,
        aliases: &mut Vec<(PathBuf, String, bool)>,
    ) -> Result<()> {
        let mut pending = vec![ty];
        while let Some(ty) = pending.pop() {
            match ty {
                TypeInfo::Basic(token) => {
                    let name = token.token().to_string();
                    if name != "nil" && Leaf::parse(&name).is_none() && !is_class(&name) {
                        aliases.push((path.to_path_buf(), name, false));
                    }
                }
                TypeInfo::Module {
                    module, type_info, ..
                } => {
                    let IndexedTypeInfo::Basic(name) = &**type_info else {
                        continue;
                    };
                    let module = module.token().to_string();
                    if module != "Enum" {
                        let key = (path.to_path_buf(), module.clone());
                        if !imports.contains_key(&key) {
                            imports.insert(key.clone(), self.import(path, &module)?);
                        }
                        aliases.push((imports[&key].clone(), name.token().to_string(), true));
                    }
                }
                TypeInfo::Optional { base, .. } => pending.push(base),
                TypeInfo::Array { type_info, .. } => pending.push(type_info),
                TypeInfo::Table { fields, .. } => {
                    for field in fields.iter().collect::<Vec<_>>().into_iter().rev() {
                        if let TypeFieldKey::IndexSignature { inner, .. } = field.key() {
                            pending.push(inner);
                        }
                        pending.push(field.value());
                    }
                }
                TypeInfo::Union(union) => {
                    pending.extend(union.types().iter().collect::<Vec<_>>().into_iter().rev())
                }
                TypeInfo::Tuple { types, .. } if types.len() == 1 => pending.extend(types.iter()),
                _ => {}
            }
        }
        Ok(())
    }

    pub fn payload(&mut self, path: &Path, ty: &TypeInfo) -> Result<Type> {
        let imports = self.prepare(path, ty)?;
        enum Work<'a> {
            Visit(&'a Path, &'a TypeInfo),
            Alias(&'a Path, String, bool),
            LeaveAlias,
            Optional,
            Array,
            Table(Vec<&'a full_moon::ast::luau::TypeField>, usize),
            Union(usize),
        }
        let mut pending = vec![Work::Visit(path, ty)];
        let mut values = Vec::new();
        let mut active = BTreeSet::new();
        let mut context = Vec::new();
        let result = (|| {
            while let Some(work) = pending.pop() {
                match work {
                    Work::Visit(path, ty) => match ty {
                        TypeInfo::Basic(token) => {
                            let name = token.token().to_string();
                            if name == "nil" {
                                values.push(Type::Nil);
                            } else if let Some(leaf) = Leaf::parse(&name) {
                                values.push(Type::Leaf(leaf));
                            } else if is_class(&name) {
                                values.push(Type::Instance(name));
                            } else {
                                pending.push(Work::Alias(path, name, false));
                            }
                        }
                        TypeInfo::String(token) => {
                            values.push(Type::StringLiteral(token.token().to_string()))
                        }
                        TypeInfo::Boolean(token) => {
                            values.push(Type::BooleanLiteral(token.token().to_string() == "true"))
                        }
                        TypeInfo::Optional { base, .. } => {
                            pending.push(Work::Optional);
                            pending.push(Work::Visit(path, base));
                        }
                        TypeInfo::Array {
                            type_info, access, ..
                        } => {
                            if access.is_some() {
                                bail!("read/write modifiers are not supported in payload types");
                            }
                            pending.push(Work::Array);
                            pending.push(Work::Visit(path, type_info));
                        }
                        TypeInfo::Table { fields, .. } => {
                            pending.push(Work::Table(fields.iter().collect(), values.len()));
                            for field in fields.iter().collect::<Vec<_>>().into_iter().rev() {
                                if field.access().is_some() {
                                    bail!(
                                        "read/write modifiers are not supported in payload types"
                                    );
                                }
                                if let TypeFieldKey::IndexSignature { inner, .. } = field.key() {
                                    pending.push(Work::Visit(path, inner));
                                }
                                pending.push(Work::Visit(path, field.value()));
                            }
                        }
                        TypeInfo::Union(union) => {
                            pending.push(Work::Union(values.len()));
                            pending.extend(
                                union
                                    .types()
                                    .iter()
                                    .collect::<Vec<_>>()
                                    .into_iter()
                                    .rev()
                                    .map(|ty| Work::Visit(path, ty)),
                            );
                        }
                        TypeInfo::Tuple { types, .. } if types.len() == 1 => {
                            pending.push(Work::Visit(path, types.iter().next().unwrap()));
                        }
                        TypeInfo::Module {
                            module, type_info, ..
                        } => {
                            let IndexedTypeInfo::Basic(name) = &**type_info else {
                                bail!("generic payload aliases are not supported");
                            };
                            let name = name.token().to_string();
                            let module = module.token().to_string();
                            if module == "Enum" {
                                if !is_enum(&name) {
                                    bail!("unknown Roblox enum Enum.{name}");
                                }
                                values.push(Type::Enum(name));
                            } else {
                                let imported = &imports[&(path.to_path_buf(), module)];
                                pending.push(Work::Alias(imported, name, true));
                            }
                        }
                        TypeInfo::Generic { .. } | TypeInfo::GenericPack { .. } => {
                            bail!("generic payload aliases are not supported")
                        }
                        TypeInfo::Intersection(_) => bail!(
                            "intersection payload types are not supported; declare one record or a union"
                        ),
                        TypeInfo::Typeof { .. } => bail!(
                            "computed payload types (typeof) are not supported; write an explicit type"
                        ),
                        TypeInfo::Callback { .. } => {
                            bail!("functions cannot be sent through Roblox remotes")
                        }
                        _ => bail!(
                            "unsupported payload type; use explicit values, records, arrays, dictionaries, or unions"
                        ),
                    },
                    Work::Alias(path, name, exported) => {
                        let (alias, public) = self.modules.get(path).and_then(|m| m.aliases.get(&name))
                            .with_context(|| format!("unsupported payload type {name}; use a supported built-in or explicit type alias"))?;
                        if exported && !public {
                            bail!("{}: type {name} must be exported", path.display());
                        }
                        if alias.generics().is_some() {
                            bail!("generic payload aliases are not supported: {name}");
                        }
                        let key = (path.to_path_buf(), name.clone());
                        if !active.insert(key.clone()) {
                            bail!("recursive payload alias {name} in {}", path.display());
                        }
                        context.push(key);
                        pending.push(Work::LeaveAlias);
                        pending.push(Work::Visit(path, alias.type_definition()));
                    }
                    Work::LeaveAlias => {
                        active.remove(&context.pop().unwrap());
                    }
                    Work::Optional => {
                        let value = values.pop().unwrap();
                        values.push(value.optional());
                    }
                    Work::Array => {
                        let value = values.pop().unwrap();
                        values.push(Self::array(value)?);
                    }
                    Work::Union(start) => {
                        let branches = values.split_off(start);
                        values.push(Type::Union(branches));
                    }
                    Work::Table(fields, start) => {
                        let mut resolved = values.split_off(start).into_iter();
                        let mut names = BTreeSet::new();
                        let mut record = Vec::new();
                        let mut indexed = None;
                        for field in &fields {
                            let value = resolved.next().unwrap();
                            let name = match field.key() {
                                TypeFieldKey::Name(name) => name.token().to_string(),
                                TypeFieldKey::IndexSignature { .. } => {
                                    match &resolved.next().unwrap() {
                                        Type::StringLiteral(name) => literal_name(name)?,
                                        Type::Leaf(key @ (Leaf::String | Leaf::Number)) => {
                                            if fields.len() != 1 {
                                                bail!(
                                                    "use a record or a dictionary/array, not mixed table fields and indexers"
                                                );
                                            }
                                            indexed = Some(if *key == Leaf::String {
                                                Type::Dictionary(Box::new(value))
                                            } else {
                                                Self::array(value)?
                                            });
                                            continue;
                                        }
                                        _ => bail!(
                                            "table indexers must be string or number; Instance keys cannot cross remotes faithfully"
                                        ),
                                    }
                                }
                                _ => bail!(
                                    "table keys must be named fields, string literals, or a string/number indexer"
                                ),
                            };
                            if !names.insert(name.clone()) {
                                bail!("duplicate record field {name}");
                            }
                            record.push(Field { name, ty: value });
                        }
                        record.sort_by(|a, b| a.name.cmp(&b.name));
                        values.push(indexed.unwrap_or(Type::Record(record)));
                    }
                }
            }
            Ok(values.pop().unwrap())
        })();
        result.map_err(|error: anyhow::Error| {
            // Keep alias context as text; thousands of nested error objects
            // would recurse again when the error is dropped.
            if context.is_empty() {
                return error;
            }
            let context = context
                .iter()
                .map(|(path, name)| format!("in alias {name} ({}): ", path.display()))
                .collect::<String>();
            anyhow!("{context}{error:#}")
        })
    }

    fn array(value: Type) -> Result<Type> {
        if value.accepts_nil() {
            bail!("array elements cannot be optional; use {{T}}? for an optional array");
        }
        Ok(Type::Array(Box::new(value)))
    }

    fn import(&mut self, path: &Path, name: &str) -> Result<PathBuf> {
        if self.modules[path].bound.contains("require") {
            bail!("static type imports cannot shadow require");
        }
        let expr = self
            .modules
            .get(path)
            .and_then(|m| m.locals.get(name))
            .with_context(|| {
                format!("{name} must be a top-level local initialized with a static require")
            })?;
        let Expression::FunctionCall(call) = expr else {
            bail!("{name} must be a static require");
        };
        let suffixes: Vec<_> = call.suffixes().collect();
        let [Suffix::Call(Call::AnonymousCall(args))] = suffixes.as_slice() else {
            bail!("{name} must be a static require");
        };
        if !matches!(call.prefix(), Prefix::Name(n) if n.token().to_string() == "require") {
            bail!("{name} must be a static require");
        }
        let target = match one_arg(args)? {
            Expression::String(token) => {
                // Roblox string requires traverse Instances, not filesystem paths.
                let value = literal_name(&token.token().to_string())?;
                let (mut location, rest) = if let Some(rest) = value.strip_prefix("@game/") {
                    (vec![], rest)
                } else if let Some(rest) = value.strip_prefix("@self/") {
                    (self.map.source_location(self.root, path)?, rest)
                } else if value.starts_with("./") || value.starts_with("../") {
                    let mut location = self.map.source_location(self.root, path)?;
                    location.pop().context("require path goes above game")?;
                    (location, value.as_str())
                } else {
                    bail!("string requires must start with ./, ../, @self/, or @game/");
                };
                for part in rest.split('/') {
                    match part {
                        "." => {}
                        ".." => {
                            location.pop().context("require path goes above game")?;
                        }
                        "" => bail!("empty component in require path"),
                        name => location.push(name.to_string()),
                    }
                }
                self.map.module_at(self.root, &location)?
            }
            expr => {
                let location = self.instance_path(path, expr)?;
                self.map.module_at(self.root, &location)?
            }
        };
        let target = fs::canonicalize(&target)
            .with_context(|| format!("could not locate type module {}", target.display()))?;
        let root = fs::canonicalize(self.root)?;
        let relative = target
            .strip_prefix(&root)
            .context("type imports must stay inside the project root")?;
        if relative
            .components()
            .any(|part| matches!(part.as_os_str().to_str(), Some(".git" | ".rogrid")))
        {
            bail!("type imports cannot use .git or generated .rogrid files");
        }
        if !self.modules.contains_key(&target) {
            let source = fs::read_to_string(&target)
                .with_context(|| format!("could not read {}", target.display()))?;
            let ast = super::parse::syntax(&target, &source)?;
            self.add(&target, &ast)?;
            self.dependencies.insert(target.clone(), source);
        }
        Ok(target)
    }

    fn instance_path(&self, path: &Path, expr: &Expression) -> Result<Vec<String>> {
        let mut current = expr;
        let mut seen = BTreeSet::new();
        let mut suffixes = Vec::new();
        let mut location = loop {
            let (prefix, parts): (Prefix, Vec<&Suffix>) = match current {
                Expression::Var(Var::Name(name)) => (Prefix::Name(name.clone()), vec![]),
                Expression::Var(Var::Expression(var)) => {
                    (var.prefix().clone(), var.suffixes().collect())
                }
                Expression::FunctionCall(call) => {
                    (call.prefix().clone(), call.suffixes().collect())
                }
                _ => bail!("require path must use game, script, or a static local path"),
            };
            let Prefix::Name(name) = prefix else {
                bail!("computed require paths are not supported");
            };
            let name = name.token().to_string();
            if matches!(name.as_str(), "game" | "script")
                && self.modules[path].bound.contains(&name)
            {
                bail!("static type imports cannot shadow {name}");
            }
            // Follow locals to the root, then apply their path operations outward.
            suffixes.extend(parts.into_iter().rev());
            match name.as_str() {
                "game" => break vec![],
                "script" => break self.map.source_location(self.root, path)?,
                _ => {
                    if !seen.insert(name.clone()) {
                        bail!("cyclic local in require path: {name}");
                    }
                    current = self
                        .modules
                        .get(path)
                        .and_then(|m| m.locals.get(&name))
                        .context("require path uses a non-static local")?;
                }
            }
        };
        for suffix in suffixes.into_iter().rev() {
            match suffix {
                Suffix::Index(Index::Dot { name, .. }) => {
                    let name = name.token().to_string();
                    if name == "Parent" {
                        location.pop().context("require path goes above game")?;
                    } else {
                        location.push(name);
                    }
                }
                Suffix::Index(Index::Brackets {
                    expression: Expression::String(name),
                    ..
                }) => location.push(literal_name(&name.token().to_string())?),
                Suffix::Call(Call::MethodCall(call)) => {
                    let method = call.name().token().to_string();
                    if !matches!(
                        method.as_str(),
                        "WaitForChild" | "FindFirstChild" | "GetService"
                    ) {
                        bail!("unsupported method in static require path: {method}");
                    }
                    if method == "GetService" && !location.is_empty() {
                        bail!("GetService must be called on game");
                    }
                    let arg = if method == "WaitForChild" {
                        let FunctionArgs::Parentheses { arguments, .. } = call.args() else {
                            bail!("use WaitForChild with parentheses");
                        };
                        if arguments.is_empty() || arguments.len() > 2 {
                            bail!("WaitForChild needs a name and optional timeout");
                        }
                        arguments.iter().next().unwrap()
                    } else {
                        one_arg(call.args())?
                    };
                    let Expression::String(name) = arg else {
                        bail!("require path names must be string literals");
                    };
                    location.push(literal_name(&name.token().to_string())?);
                }
                _ => bail!("computed require paths are not supported"),
            }
        }
        Ok(location)
    }
}

fn one_arg(args: &FunctionArgs) -> Result<&Expression> {
    let FunctionArgs::Parentheses { arguments, .. } = args else {
        bail!("static imports require parentheses and one argument");
    };
    if arguments.len() != 1 {
        bail!("static imports require exactly one argument");
    }
    Ok(arguments.iter().next().unwrap())
}

// Names need their value for lookup. Singleton payload strings preserve the
// original token instead, including arbitrary non-UTF-8 byte escapes.
fn literal_name(token: &str) -> Result<String> {
    if token.starts_with('[') {
        bail!("use quoted strings for field names and require paths");
    }
    let mut chars = token[1..token.len() - 1].chars().peekable();
    let mut bytes = Vec::new();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            bytes.extend(ch.to_string().bytes());
            continue;
        }
        let ch = chars.next().context("incomplete string escape")?;
        match ch {
            'a' => bytes.push(7),
            'b' => bytes.push(8),
            'f' => bytes.push(12),
            'n' | '\n' => bytes.push(10),
            'r' => bytes.push(13),
            't' => bytes.push(9),
            'v' => bytes.push(11),
            '\\' | '\'' | '"' => bytes.push(ch as u8),
            'z' => {
                while chars.peek().is_some_and(|c| c.is_whitespace()) {
                    chars.next();
                }
            }
            'x' => {
                let digits: String = chars.by_ref().take(2).collect();
                bytes.push(u8::from_str_radix(&digits, 16).context("invalid hex escape")?);
            }
            'u' => {
                if chars.next() != Some('{') {
                    bail!("invalid unicode escape");
                }
                let digits: String = chars.by_ref().take_while(|c| *c != '}').collect();
                let ch = char::from_u32(u32::from_str_radix(&digits, 16)?)
                    .context("invalid unicode escape")?;
                bytes.extend(ch.to_string().bytes());
            }
            '0'..='9' => {
                let mut digits = ch.to_string();
                for _ in 0..2 {
                    if chars.peek().is_some_and(|c| c.is_ascii_digit()) {
                        digits.push(chars.next().unwrap());
                    }
                }
                bytes.push(digits.parse::<u8>().context("invalid decimal escape")?);
            }
            _ => bail!("unsupported escape in field name or require path"),
        }
    }
    String::from_utf8(bytes).context("field names and require paths must be UTF-8")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn imported(source: &str, shared: &str, other: &str) -> Result<(Type, usize)> {
        let dir = tempfile::tempdir()?;
        let root = dir.path();
        let main = root.join("Event.luau");
        fs::write(&main, source)?;
        // The mapped name deliberately differs from the filename.
        fs::write(root.join("shared-source.luau"), shared)?;
        fs::write(root.join("Other.luau"), other)?;
        let map: Node =
            serde_json::from_value(json!({"name":"test","className":"DataModel","children":[
                {"name":"ReplicatedStorage","className":"ReplicatedStorage","children":[
                    {"name":"Shared","className":"ModuleScript","filePaths":["shared-source.luau"]},
                    {"name":"Other","className":"ModuleScript","filePaths":["Other.luau"]},
                    {"name":"Event","className":"ModuleScript","filePaths":["Event.luau"]}
                ]}
            ]}))?;
        let ast = super::super::parse::syntax(&main, source)?;
        let mut resolver = Resolver::new(root, &map);
        resolver.add(&main, &ast)?;
        let alias = resolver.modules[&main].aliases["Payload"].0.clone();
        let result = resolver.payload(&main, alias.type_definition())?;
        Ok((result, resolver.dependencies.len()))
    }

    #[test]
    fn resolves_relative_game_script_and_local_require_paths() {
        for require in [
            "require('./Shared')",
            "require('@game/ReplicatedStorage/Shared')",
            "require('@self/../Shared')",
            "require(script.Parent.Shared)",
            "require(script.Parent['Shared'])",
            "require(game.ReplicatedStorage.Shared)",
            "require(game:GetService('ReplicatedStorage'):WaitForChild('Shared', 10))",
            "require(Storage:FindFirstChild('Shared'))",
        ] {
            let source = format!(
                "local Storage = game:GetService('ReplicatedStorage')\nlocal Types = {require}\ntype Payload = Types.Item"
            );
            let (ty, deps) =
                imported(&source, "export type Item = {id: string}\nreturn {}", "").unwrap();
            assert_eq!(ty.to_string(), "{ id: string }");
            assert_eq!(deps, 1);
        }
    }

    fn local_path_chain(root: &str, last: usize) -> String {
        let mut source = format!("local p0 = {root}\n");
        for i in 1..=last {
            source.push_str(&format!("local p{i} = p{}\n", i - 1));
        }
        source
    }

    #[test]
    fn long_local_require_paths_resolve_and_can_be_reused() {
        for root in ["game:GetService('ReplicatedStorage')", "script.Parent"] {
            for last in [31, 32, 33, 128] {
                let mut source = local_path_chain(root, last);
                source.push_str(&format!(
                    "local T = require(p{last}.Shared)\nlocal U = require(p{last}['Other'])\ntype Payload = {{first: T.Item, second: U.Id}}"
                ));
                let (ty, dependencies) = imported(
                    &source,
                    "export type Item = string",
                    "export type Id = number",
                )
                .unwrap();
                assert_eq!(ty.to_string(), "{ first: string, second: number }");
                assert_eq!(dependencies, 2);
            }
        }
    }

    #[test]
    fn local_require_paths_apply_operations_from_root_to_target() {
        let source = "local root = game
local storage = root:GetService('ReplicatedStorage')
local folder = storage['Shared'].Parent
local other = folder:WaitForChild('Other', 10)
local parent = other.Parent
local T = require(parent:FindFirstChild('Shared'))
type Payload = T.Item";
        let (ty, dependencies) = imported(
            source,
            "export type Item = string",
            "export type Item = number",
        )
        .unwrap();
        assert_eq!(ty.to_string(), "string");
        assert_eq!(dependencies, 1);
    }

    #[test]
    fn long_local_require_paths_preserve_invalid_path_errors() {
        for (root, expected) in [
            ("p128", "cyclic local in require path"),
            ("dynamic()", "require path uses a non-static local"),
            ("game.Parent", "require path goes above game"),
            (
                "game.ReplicatedStorage[child]",
                "computed require paths are not supported",
            ),
            (
                "game.ReplicatedStorage:GetService('ReplicatedStorage')",
                "GetService must be called on game",
            ),
        ] {
            let source = local_path_chain(root, 128)
                + "local T = require(p128.Shared)\ntype Payload = T.Item";
            let error = imported(&source, "export type Item = number", "").unwrap_err();
            assert!(format!("{error:#}").contains(expected), "{root}: {error:#}");
        }
        let source = local_path_chain("script.Parent", 128)
            + "p64 = script.Parent\nlocal T = require(p128.Shared)\ntype Payload = T.Item";
        let error = imported(&source, "export type Item = number", "").unwrap_err();
        assert!(format!("{error:#}").contains("require path uses a non-static local"));
    }

    #[test]
    fn follows_transitive_aliases_and_records_every_dependency() {
        let (ty, deps) = imported(
            "local T = require('./Shared')\ntype Payload = T.Item",
            "local T = require('./Other')\nexport type Item = T.Id",
            "export type Id = number",
        )
        .unwrap();
        assert_eq!(ty.to_string(), "number");
        assert_eq!(deps, 2);
    }

    #[test]
    fn long_imported_alias_chains_reuse_types_without_false_cycles() {
        let mut shared = "local T = require('./Other')\ntype A0 = T.Id\n".to_string();
        for i in 1..=512 {
            shared.push_str(&format!("type A{i} = A{}\n", i - 1));
        }
        shared.push_str("export type Item = {first: A512, second: A512}\n");
        let (ty, dependencies) = imported(
            "local T = require('./Shared')\ntype Payload = T.Item",
            &shared,
            "export type Id = string",
        )
        .unwrap();
        assert_eq!(ty.to_string(), "{ first: string, second: string }");
        assert_eq!(dependencies, 2);
    }

    #[test]
    fn resolution_recovers_after_a_deep_cycle_or_invalid_type() {
        let path = Path::new("Test.luau");
        let map: Node =
            serde_json::from_value(json!({"name": "test", "className": "DataModel"})).unwrap();
        let mut source = "type Good = string\ntype A0 = A256\n".to_string();
        for i in 1..=256 {
            source.push_str(&format!("type A{i} = A{}\n", i - 1));
        }
        source.push_str("type Bad = {Good?}\n");
        let ast = super::super::parse::syntax(path, &source).unwrap();
        let mut resolver = Resolver::new(Path::new("."), &map);
        resolver.add(path, &ast).unwrap();
        for name in ["A256", "Bad", "Good"] {
            let alias = resolver.modules[path].aliases[name].0.clone();
            let result = resolver.payload(path, alias.type_definition());
            if name == "Good" {
                assert_eq!(result.unwrap().to_string(), "string");
            } else {
                assert!(result.is_err());
            }
        }
    }

    #[test]
    fn imports_reject_private_generic_cyclic_missing_and_dynamic_types() {
        for (main, shared, other, message) in [
            (
                "local T = require('./Shared')\nlocal T\ntype Payload = T.Item",
                "export type Item=number",
                "",
                "static require",
            ),
            (
                "local T = require('./Shared')\nT = require('./Other')\ntype Payload = T.Item",
                "export type Item=number",
                "",
                "static require",
            ),
            (
                "local game = {}\nlocal T = require(game.Shared)\ntype Payload = T.Item",
                "",
                "",
                "cannot shadow game",
            ),
            (
                "local require = something\nlocal T = require('./Shared')\ntype Payload = T.Item",
                "",
                "",
                "cannot shadow require",
            ),
            (
                "local T = require('./Shared.luau')\ntype Payload = T.Item",
                "export type Item=number",
                "",
                "exactly one ModuleScript",
            ),
            (
                "local T = require('./Shared')\ntype Payload = T.Item",
                "type Item = string",
                "",
                "must be exported",
            ),
            (
                "local T = require('./Shared')\ntype Payload = T.Item",
                "export type Item<T> = string",
                "",
                "generic payload aliases",
            ),
            (
                "local T = require('./Shared')\ntype Payload = T.Item",
                "local T = require('./Other')\nexport type Item = T.Item",
                "local T = require('./Shared')\nexport type Item = T.Item",
                "recursive payload alias",
            ),
            (
                "local T = require('./Missing')\ntype Payload = T.Item",
                "",
                "",
                "exactly one ModuleScript",
            ),
            (
                "local T = require(findModule())\ntype Payload = T.Item",
                "",
                "",
                "non-static local",
            ),
            (
                "local T = require(123)\ntype Payload = T.Item",
                "",
                "",
                "require path must use",
            ),
            (
                "local T = require(game:GetService('ReplicatedStorage').Missing)\ntype Payload = T.Item",
                "",
                "",
                "exactly one ModuleScript",
            ),
        ] {
            let error = imported(main, shared, other).expect_err("must reject");
            assert!(format!("{error:#}").contains(message), "{main}: {error:#}");
        }
    }

    #[test]
    fn field_names_decode_luau_escapes_and_detect_equivalent_keys() {
        assert_eq!(literal_name(r#""a\x62\099\u{1f600}""#).unwrap(), "abc😀");
        assert_eq!(literal_name(r#"'a\z  b\n'"#).unwrap(), "ab\n");
        assert!(literal_name(r#""\255""#).is_err());
    }
}
