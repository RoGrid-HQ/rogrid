//! Reads one request file and works out its input and output types.

use std::collections::{HashMap, HashSet};

use full_moon::ast::luau::{IndexedTypeInfo, TypeDeclaration, TypeField, TypeFieldKey, TypeInfo};
use full_moon::ast::punctuated::Punctuated;
use full_moon::ast::{
    AnonymousFunction, Block, Call, Expression, FunctionArgs, Index, LastStmt, Parameter, Prefix,
    Stmt, Suffix,
};
use full_moon::node::Node;
use full_moon::tokenizer::{TokenReference, TokenType};

use super::types::{DATATYPES, Ty};

pub struct Function {
    /// Path below the functions folder without the extension, such as `shop/buyItem`.
    pub name: String,
    pub input: Option<Ty>,
    pub output: Option<Ty>,
    /// Named types the signature uses, prefixed so they are unique across files.
    pub aliases: Vec<(String, Ty)>,
}

/// Something wrong in a request file, tied to the line it is on.
#[derive(Debug)]
pub struct Problem {
    pub line: usize,
    pub message: String,
}

pub fn parse(name: &str, source: &str) -> Result<Function, Problem> {
    let ast = full_moon::parse(source).map_err(|errors| Problem {
        line: errors[0].range().0.line(),
        message: format!("syntax error: {}", errors[0].error_message()),
    })?;
    let block = ast.nodes();

    let (input, output) = signature(handler(block)?)?;
    let mut resolver = Resolver::new(name, block);
    let input = input.map(|info| resolver.convert(info)).transpose()?;
    let output = output.map(|info| resolver.convert(info)).transpose()?;

    Ok(Function {
        name: name.to_string(),
        input,
        output,
        aliases: resolver.aliases,
    })
}

/// The inline function passed to `RoGrid.request`, which the module must return.
fn handler(block: &Block) -> Result<&AnonymousFunction, Problem> {
    const EXPECTED: &str = "the module must end with `return RoGrid.request(...)`";

    let Some(LastStmt::Return(returned)) = block.last_stmt() else {
        return Err(Problem {
            line: 1,
            message: EXPECTED.into(),
        });
    };
    let mut values = returned.returns().iter();
    let (Some(Expression::FunctionCall(call)), None) = (values.next(), values.next()) else {
        return Err(problem(returned.token(), EXPECTED));
    };

    let suffixes: Vec<&Suffix> = call.suffixes().collect();
    let (
        Prefix::Name(_),
        [
            Suffix::Index(Index::Dot { name, .. }),
            Suffix::Call(Call::AnonymousCall(FunctionArgs::Parentheses { arguments, .. })),
        ],
    ) = (call.prefix(), suffixes.as_slice())
    else {
        return Err(problem(returned.token(), EXPECTED));
    };

    let kind = text(name);
    if kind != "request" {
        let message = format!("`RoGrid.{kind}` is not supported yet. Only `RoGrid.request` is");
        return Err(problem(name, message));
    }

    match arguments.iter().last() {
        Some(Expression::Function(function)) => Ok(function),
        _ => Err(problem(
            name,
            "pass the handler as an inline function, so its types can be read",
        )),
    }
}

/// The handler's input and output annotations. Either can be absent.
fn signature(
    function: &AnonymousFunction,
) -> Result<(Option<&TypeInfo>, Option<&TypeInfo>), Problem> {
    let body = function.body();
    let mut parameters = body.parameters().iter().zip(body.type_specifiers());

    match parameters.next() {
        Some((Parameter::Name(_), Some(specifier)))
            if is_named(specifier.type_info(), "Player") => {}
        _ => {
            return Err(problem(
                function.function_token(),
                "the handler's first parameter must be `player: Player`",
            ));
        }
    }

    let input = match parameters.next() {
        None => None,
        Some((Parameter::Name(_), Some(specifier))) => Some(specifier.type_info()),
        Some((Parameter::Name(name), None)) => {
            return Err(problem(
                name,
                "annotate the input, for example `input: Input`",
            ));
        }
        Some(_) => {
            return Err(problem(
                function.function_token(),
                "a request takes one input table, not a variable number of arguments",
            ));
        }
    };

    if parameters.next().is_some() {
        return Err(problem(
            function.function_token(),
            "a request takes one input table, not several parameters",
        ));
    }

    let output = match body.return_type().map(|specifier| specifier.type_info()) {
        Some(TypeInfo::Tuple { types, .. }) if types.is_empty() => None,
        Some(info @ TypeInfo::Tuple { types, .. }) if types.len() > 1 => {
            return Err(problem(info, "a request returns one value"));
        }
        other => other,
    };

    Ok((input, output))
}

/// Turns Luau type syntax into `Ty`, following type aliases declared in the same file.
struct Resolver<'a> {
    prefix: String,
    declarations: HashMap<String, &'a TypeDeclaration>,
    aliases: Vec<(String, Ty)>,
    seen: HashSet<String>,
}

impl<'a> Resolver<'a> {
    fn new(function_name: &str, block: &'a Block) -> Self {
        let mut declarations = HashMap::new();
        for stmt in block.stmts() {
            let declaration = match stmt {
                Stmt::TypeDeclaration(declaration) => declaration,
                Stmt::ExportedTypeDeclaration(exported) => exported.type_declaration(),
                _ => continue,
            };
            declarations.insert(text(declaration.type_name()), declaration);
        }

        Self {
            prefix: pascal_case(function_name),
            declarations,
            aliases: Vec::new(),
            seen: HashSet::new(),
        }
    }

    fn convert(&mut self, info: &TypeInfo) -> Result<Ty, Problem> {
        match info {
            TypeInfo::Basic(token) => self.named(token),
            TypeInfo::String(token) => string_literal(token),
            TypeInfo::Boolean(token) => Ok(Ty::BooleanLiteral(text(token) == "true")),
            TypeInfo::Optional { base, .. } => Ok(optional(self.convert(base)?)),
            TypeInfo::Array { type_info, .. } => Ok(Ty::Array(Box::new(self.convert(type_info)?))),
            TypeInfo::Table { fields, .. } => self.table(info, fields),
            TypeInfo::Union(union) => self.union(union.types()),
            TypeInfo::Tuple { types, .. } if types.len() == 1 => {
                self.convert(types.iter().next().expect("length was checked"))
            }
            TypeInfo::Module {
                module, type_info, ..
            } => match (&**type_info, text(module)) {
                (IndexedTypeInfo::Basic(item), module) if module == "Enum" => {
                    Ok(Ty::EnumItem(text(item)))
                }
                _ => Err(problem(
                    info,
                    "types from other modules are not supported yet. Declare the type in this file",
                )),
            },
            TypeInfo::Typeof { .. } => Err(problem(
                info,
                format!("`{}` cannot be checked at runtime", info.to_string().trim()),
            )),
            TypeInfo::Callback { .. } => {
                Err(problem(info, "functions cannot be sent over the network"))
            }
            TypeInfo::Generic { base, .. } => Err(problem(
                info,
                format!("generic type `{}<...>` is not supported", text(base)),
            )),
            TypeInfo::Intersection(_) => Err(problem(
                info,
                "intersections with `&` are not supported. Write the fields in one table",
            )),
            _ => Err(problem(
                info,
                format!("`{}` cannot be checked at runtime", info.to_string().trim()),
            )),
        }
    }

    fn named(&mut self, token: &TokenReference) -> Result<Ty, Problem> {
        let name = text(token);
        match name.as_str() {
            "string" => Ok(Ty::String),
            "number" => Ok(Ty::Number),
            "boolean" => Ok(Ty::Boolean),
            "buffer" => Ok(Ty::Buffer),
            "unknown" => Ok(Ty::Unknown),
            "any" => Err(problem(
                token,
                "`any` would switch validation off without saying so. Use `unknown` to opt out on purpose",
            )),
            "nil" => Err(problem(
                token,
                "`nil` alone is not a useful type. Use `T?` for an optional value",
            )),
            "never" | "thread" | "vector" => Err(problem(
                token,
                format!("`{name}` cannot be sent over the network"),
            )),
            _ if self.declarations.contains_key(&name) => self.alias(token, &name),
            _ if DATATYPES.contains(&name.as_str()) => Ok(Ty::Datatype(name)),
            _ if name.starts_with(char::is_uppercase) => Ok(Ty::Instance(name)),
            _ => Err(problem(token, format!("unknown type `{name}`"))),
        }
    }

    /// A reference to a type declared in this file. Its definition is converted once,
    /// and registered before that happens so recursive types terminate.
    fn alias(&mut self, token: &TokenReference, name: &str) -> Result<Ty, Problem> {
        let prefixed = format!("{}{name}", self.prefix);

        if self.seen.insert(name.to_string()) {
            let declaration = self.declarations[name];
            if declaration.generics().is_some() {
                return Err(problem(
                    token,
                    format!("generic type `{name}<...>` is not supported"),
                ));
            }

            let index = self.aliases.len();
            self.aliases.push((prefixed.clone(), Ty::Unknown));
            self.aliases[index].1 = self.convert(declaration.type_definition())?;
        }

        Ok(Ty::Alias(prefixed))
    }

    fn table(&mut self, whole: &TypeInfo, fields: &Punctuated<TypeField>) -> Result<Ty, Problem> {
        let mut named = Vec::new();
        let mut indexer = None;

        for field in fields {
            match field.key() {
                TypeFieldKey::Name(name) => named.push((text(name), self.convert(field.value())?)),
                TypeFieldKey::IndexSignature { inner, .. } => {
                    indexer = Some((inner, field.value()))
                }
                _ => return Err(problem(whole, "this kind of table field is not supported")),
            }
        }

        match indexer {
            None => Ok(Ty::Table(named)),
            Some(_) if !named.is_empty() => Err(problem(
                whole,
                "a table cannot mix named fields with `[key]: value`",
            )),
            Some((key, value)) if is_named(key, "string") => {
                Ok(Ty::Map(Box::new(self.convert(value)?)))
            }
            Some((key, _)) => Err(problem(
                key,
                "map keys must be `string`, the only key type Roblox sends reliably. For a list use `{ T }`",
            )),
        }
    }

    /// `nil` members make the whole union optional. Nested unions are flattened.
    fn union(&mut self, types: &Punctuated<TypeInfo>) -> Result<Ty, Problem> {
        let mut members = Vec::new();
        let mut has_nil = false;

        for info in types {
            if is_named(info, "nil") {
                has_nil = true;
                continue;
            }
            match self.convert(info)? {
                Ty::Union(inner) => members.extend(inner),
                Ty::Optional(inner) => {
                    has_nil = true;
                    members.push(*inner);
                }
                member => members.push(member),
            }
        }

        let ty = if members.len() == 1 {
            members.remove(0)
        } else {
            Ty::Union(members)
        };
        Ok(if has_nil { optional(ty) } else { ty })
    }
}

fn optional(ty: Ty) -> Ty {
    match ty {
        // Both already accept nil.
        Ty::Optional(_) | Ty::Unknown => ty,
        _ => Ty::Optional(Box::new(ty)),
    }
}

fn string_literal(token: &TokenReference) -> Result<Ty, Problem> {
    match token.token_type() {
        TokenType::StringLiteral { literal, .. } if !literal.contains('\\') => {
            Ok(Ty::StringLiteral(literal.to_string()))
        }
        _ => Err(problem(
            token,
            "escape sequences in a string literal type are not supported",
        )),
    }
}

fn is_named(info: &TypeInfo, name: &str) -> bool {
    matches!(info, TypeInfo::Basic(token) if text(token) == name)
}

fn text(token: &TokenReference) -> String {
    token.token().to_string()
}

fn problem(node: &impl Node, message: impl Into<String>) -> Problem {
    Problem {
        line: node.start_position().map_or(1, |position| position.line()),
        message: message.into(),
    }
}

/// `shop/buyItem` becomes `ShopBuyItem`.
fn pascal_case(name: &str) -> String {
    name.split('/')
        .map(|segment| {
            let mut chars = segment.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().chain(chars).collect::<String>(),
                None => String::new(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const BUY_ITEM: &str = r#"
local RoGrid = require("@game/ReplicatedStorage/Packages/rogrid")

type Input = {
	itemId: string,
	quantity: number?,
}

type Output =
	{ status: "ok", balance: number }
	| { status: "failed", reason: "unknown-item" | "insufficient-funds" }

return RoGrid.request(function(player: Player, input: Input): Output
	return { status = "ok", balance = 1 }
end)
"#;

    fn parse_handler(signature: &str) -> Result<Function, Problem> {
        parse(
            "test",
            &format!("return RoGrid.request(function({signature}) end)"),
        )
    }

    #[test]
    fn reads_input_and_output_through_aliases() {
        let function = parse("shop/buyItem", BUY_ITEM).unwrap();

        assert_eq!(function.input, Some(Ty::Alias("ShopBuyItemInput".into())));
        assert_eq!(function.output, Some(Ty::Alias("ShopBuyItemOutput".into())));
        assert_eq!(
            function.aliases[0],
            (
                "ShopBuyItemInput".to_string(),
                Ty::Table(vec![
                    ("itemId".into(), Ty::String),
                    ("quantity".into(), Ty::Optional(Box::new(Ty::Number))),
                ])
            )
        );
        assert_eq!(
            function.aliases[1].1.to_luau(),
            r#"{ status: "ok", balance: number } | { status: "failed", reason: "unknown-item" | "insufficient-funds" }"#
        );
    }

    #[test]
    fn input_and_output_are_optional() {
        let function = parse_handler("player: Player").unwrap();
        assert_eq!(function.input, None);
        assert_eq!(function.output, None);

        let source = "return RoGrid.request(function(player: Player): { string } return {} end)";
        let function = parse("test", source).unwrap();
        assert_eq!(function.input, None);
        assert_eq!(function.output, Some(Ty::Array(Box::new(Ty::String))));

        let source = "return RoGrid.request(function(player: Player, input: string): () end)";
        assert_eq!(parse("test", source).unwrap().output, None);
    }

    #[test]
    fn options_come_before_the_handler() {
        let source = "return RoGrid.request({ guards = { aliveOnly } }, function(player: Player, input: { item: BasePart }) end)";
        let function = parse("world/pickUp", source).unwrap();
        assert_eq!(
            function.input,
            Some(Ty::Table(vec![(
                "item".into(),
                Ty::Instance("BasePart".into())
            )]))
        );
    }

    #[test]
    fn understands_roblox_types_maps_and_nil_unions() {
        let function = parse_handler(
            "player: Player, input: { at: Vector3, material: Enum.Material, scores: { [string]: number }, note: string | nil }",
        )
        .unwrap();
        assert_eq!(
            function.input.unwrap().to_luau(),
            "{ at: Vector3, material: Enum.Material, scores: { [string]: number }, note: string? }"
        );
    }

    #[test]
    fn recursive_aliases_terminate() {
        let source = "type Node = { children: { Node } }\nreturn RoGrid.request(function(player: Player, input: Node) end)";
        let function = parse("tree", source).unwrap();
        assert_eq!(
            function.aliases[0].1.to_luau(),
            "{ children: { TreeNode } }"
        );
    }

    #[test]
    fn rejects_what_cannot_be_checked() {
        let cases = [
            ("player: Player, input: any", "`any`"),
            (
                "player: Player, input: typeof(workspace)",
                "cannot be checked",
            ),
            (
                "player: Player, input: (number) -> ()",
                "functions cannot be sent",
            ),
            (
                "player: Player, input: { [number]: string }",
                "map keys must be `string`",
            ),
            ("player: Player, input: Types.Item", "other modules"),
            ("player: Player, input", "annotate the input"),
            ("player: Player, a: string, b: string", "one input table"),
            ("input: string", "must be `player: Player`"),
        ];
        for (signature, expected) in cases {
            let message = parse_handler(signature).err().expect(signature).message;
            assert!(message.contains(expected), "{signature}: {message}");
        }
    }

    #[test]
    fn rejects_modules_that_do_not_return_a_request() {
        let message = parse("x", "return {}").err().unwrap().message;
        assert!(message.contains("RoGrid.request"));

        let message = parse("x", "return RoGrid.message(function(player: Player) end)")
            .err()
            .unwrap()
            .message;
        assert!(message.contains("not supported yet"));
    }

    #[test]
    fn reports_the_line_of_a_problem() {
        let source = "type Input = {\n\titemId: string,\n\tdiscount: any,\n}\nreturn RoGrid.request(function(player: Player, input: Input) end)";
        assert_eq!(parse("x", source).err().unwrap().line, 3);
    }
}
