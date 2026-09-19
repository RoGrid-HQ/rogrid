//! Resolve static Luau payload types without running game code.
use super::sourcemap::Node;
use super::types::{Field, Leaf, NetworkPayloadType as Type, is_class, is_enum};
use anyhow::{Context, Result, bail};
use full_moon::ast::luau::{IndexedTypeInfo, TypeDeclaration, TypeFieldKey, TypeInfo};
use full_moon::ast::{Ast, Call, Expression, FunctionArgs, Index, Prefix, Stmt, Suffix, Var};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Default)]
struct Declarations {
    aliases: BTreeMap<String, (TypeDeclaration, bool)>,
    locals: BTreeMap<String, Expression>,
    bound: BTreeSet<String>,
}

pub struct Resolver<'a> {
    root: &'a Path,
    map: &'a Node,
    modules: BTreeMap<PathBuf, Declarations>,
    active: BTreeSet<(PathBuf, String)>,
    pub dependencies: BTreeMap<PathBuf, String>,
}

impl<'a> Resolver<'a> {
    pub fn new(root: &'a Path, map: &'a Node) -> Self {
        Self {
            root,
            map,
            modules: BTreeMap::new(),
            active: BTreeSet::new(),
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
                    .insert(name.clone(), (alias.clone(), exported))
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

    pub fn payload(&mut self, path: &Path, ty: &TypeInfo, remaining: &mut usize) -> Result<Type> {
        self.resolve(path, ty, 0, remaining)
    }

    fn resolve(
        &mut self,
        path: &Path,
        ty: &TypeInfo,
        depth: usize,
        remaining: &mut usize,
    ) -> Result<Type> {
        if depth > 32 {
            bail!("payload types exceed 32 levels of nesting/alias expansion");
        }
        if *remaining == 0 {
            bail!("payload schema exceeds 4096 expanded nodes per event");
        }
        *remaining -= 1;
        Ok(match ty {
            TypeInfo::Basic(token) => {
                let name = token.token().to_string();
                if name == "nil" {
                    Type::Nil
                } else if let Some(leaf) = Leaf::parse(&name) {
                    Type::Leaf(leaf)
                } else if is_class(&name) {
                    Type::Instance(name)
                } else {
                    return self.alias(path, &name, false, depth + 1, remaining);
                }
            }
            // Preserve Luau's literal token, including arbitrary byte escapes.
            TypeInfo::String(token) => Type::StringLiteral(token.token().to_string()),
            TypeInfo::Boolean(token) => Type::BooleanLiteral(token.token().to_string() == "true"),
            TypeInfo::Optional { base, .. } => {
                self.resolve(path, base, depth + 1, remaining)?.optional()
            }
            TypeInfo::Array {
                type_info, access, ..
            } => {
                if access.is_some() {
                    bail!("read/write modifiers are not supported in payload types");
                }
                Self::array(self.resolve(path, type_info, depth + 1, remaining)?)?
            }
            TypeInfo::Table { fields, .. } => {
                let mut names = BTreeSet::new();
                let mut record = Vec::new();
                let mut indexed = None;
                for field in fields.iter() {
                    if field.access().is_some() {
                        bail!("read/write modifiers are not supported in payload types");
                    }
                    let value = self.resolve(path, field.value(), depth + 1, remaining)?;
                    let name = match field.key() {
                        TypeFieldKey::Name(name) => name.token().to_string(),
                        TypeFieldKey::IndexSignature { inner, .. } => {
                            match self.resolve(path, inner, depth + 1, remaining)? {
                                Type::StringLiteral(name) => literal_name(&name)?,
                                Type::Leaf(key @ (Leaf::String | Leaf::Number)) => {
                                    if fields.len() != 1 {
                                        bail!(
                                            "use a record or a dictionary/array, not mixed table fields and indexers"
                                        );
                                    }
                                    indexed = Some(if key == Leaf::String {
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
                indexed.unwrap_or(Type::Record(record))
            }
            TypeInfo::Union(union) => Type::Union(
                union
                    .types()
                    .iter()
                    .map(|ty| self.resolve(path, ty, depth + 1, remaining))
                    .collect::<Result<_>>()?,
            ),
            TypeInfo::Tuple { types, .. } if types.len() == 1 => {
                self.resolve(path, types.iter().next().unwrap(), depth + 1, remaining)?
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
                    Type::Enum(name)
                } else {
                    let imported = self.import(path, &module)?;
                    self.alias(&imported, &name, true, depth + 1, remaining)?
                }
            }
            TypeInfo::Generic { .. } | TypeInfo::GenericPack { .. } => {
                bail!("generic payload aliases are not supported")
            }
            TypeInfo::Intersection(_) => {
                bail!("intersection payload types are not supported; declare one record or a union")
            }
            TypeInfo::Typeof { .. } => {
                bail!("computed payload types (typeof) are not supported; write an explicit type")
            }
            TypeInfo::Callback { .. } => bail!("functions cannot be sent through Roblox remotes"),
            _ => bail!(
                "unsupported payload type; use explicit values, records, arrays, dictionaries, or unions"
            ),
        })
    }

    fn array(value: Type) -> Result<Type> {
        if value.accepts_nil() {
            bail!("array elements cannot be optional; use {{T}}? for an optional array");
        }
        Ok(Type::Array(Box::new(value)))
    }

    fn alias(
        &mut self,
        path: &Path,
        name: &str,
        exported: bool,
        depth: usize,
        remaining: &mut usize,
    ) -> Result<Type> {
        let (alias, public) = self.modules.get(path).and_then(|m| m.aliases.get(name)).cloned()
            .with_context(|| format!("unsupported payload type {name}; use a supported built-in or explicit type alias"))?;
        if exported && !public {
            bail!("{}: type {name} must be exported", path.display());
        }
        if alias.generics().is_some() {
            bail!("generic payload aliases are not supported: {name}");
        }
        let key = (path.to_path_buf(), name.to_string());
        if !self.active.insert(key.clone()) {
            bail!("recursive payload alias {name} in {}", path.display());
        }
        let result = self
            .resolve(path, alias.type_definition(), depth, remaining)
            .with_context(|| format!("in alias {name} ({})", path.display()));
        self.active.remove(&key);
        result
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
                let location = self.instance_path(path, expr, &mut BTreeSet::new(), 0)?;
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

    fn instance_path(
        &self,
        path: &Path,
        expr: &Expression,
        active: &mut BTreeSet<String>,
        depth: usize,
    ) -> Result<Vec<String>> {
        if depth > 32 {
            bail!("static require path is too deeply nested");
        }
        let (prefix, suffixes): (Prefix, Vec<&Suffix>) = match expr {
            Expression::Var(Var::Name(name)) => (Prefix::Name(name.clone()), vec![]),
            Expression::Var(Var::Expression(var)) => {
                (var.prefix().clone(), var.suffixes().collect())
            }
            Expression::FunctionCall(call) => (call.prefix().clone(), call.suffixes().collect()),
            _ => bail!("require path must use game, script, or a static local path"),
        };
        let Prefix::Name(name) = prefix else {
            bail!("computed require paths are not supported");
        };
        let name = name.token().to_string();
        if matches!(name.as_str(), "game" | "script") && self.modules[path].bound.contains(&name) {
            bail!("static type imports cannot shadow {name}");
        }
        let mut location = match name.as_str() {
            "game" => vec![],
            "script" => self.map.source_location(self.root, path)?,
            _ => {
                if !active.insert(name.clone()) {
                    bail!("cyclic local in require path: {name}");
                }
                let value = self
                    .modules
                    .get(path)
                    .and_then(|m| m.locals.get(&name))
                    .context("require path uses a non-static local")?;
                let result = self.instance_path(path, value, active, depth + 1)?;
                active.remove(&name);
                result
            }
        };
        for suffix in suffixes {
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
        let result = resolver.alias(&main, "Payload", false, 0, &mut 4096)?;
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
