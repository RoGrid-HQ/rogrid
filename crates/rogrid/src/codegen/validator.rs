//! Turns a `Ty` into Luau code that checks a value at runtime.
//!
//! Every checker is a function `(v: any) -> string?`. It returns nil for a valid
//! value, or a description such as `.items[2].id: expected string`. The path is only
//! put together on failure, so checking a valid value allocates nothing.

use std::collections::BTreeSet;
use std::fmt::Write;

use super::types::{Ty, lua_string};

/// Small functions the checkers share. Only the ones that get used are written out.
const HELPERS: &[(&str, &str)] = &[
    (
        "isNumber",
        "-- NaN and the infinities are rejected: they are a classic way to break server maths.\n\
         local function isNumber(v: any): boolean\n\
         \treturn type(v) == \"number\" and v == v and v ~= math.huge and v ~= -math.huge\n\
         end\n",
    ),
    (
        "isDatatype",
        "-- `v == v` is false for a value with a NaN component, such as a Vector3.\n\
         local function isDatatype(v: any, name: string): boolean\n\
         \treturn typeof(v) == name and v == v\n\
         end\n",
    ),
    (
        "isEnum",
        "local function isEnum(v: any, enum: Enum): boolean\n\
         \treturn typeof(v) == \"EnumItem\" and v.EnumType == enum\n\
         end\n",
    ),
    (
        "isInstance",
        "local function isInstance(v: any, class: string): boolean\n\
         \treturn typeof(v) == \"Instance\" and v:IsA(class)\n\
         end\n",
    ),
];

#[derive(Default)]
pub struct Validators {
    /// Checkers in the order they are written out: the key in `checks`, then the body.
    functions: Vec<(String, String)>,
    helpers: BTreeSet<&'static str>,
    /// Makes local variable names unique within one checker, so nothing shadows.
    locals: usize,
}

impl Validators {
    /// Writes the checker for a named type.
    pub fn alias(&mut self, name: &str, ty: &Ty) {
        self.function(name.to_string(), ty);
    }

    /// A Luau expression for a function that checks `ty`. Named types already have
    /// one. Anything else gets its own, named after `hint`.
    pub fn checker(&mut self, ty: &Ty, hint: &str) -> String {
        if let Ty::Alias(name) = ty {
            return reference(name);
        }

        let mut key = hint.to_string();
        while self.functions.iter().any(|(existing, _)| *existing == key) {
            key.push('\'');
        }
        self.function(key.clone(), ty);
        reference(&key)
    }

    /// The helpers and the `checks` table as Luau source.
    pub fn render(&self) -> String {
        let mut out = String::new();

        for (name, source) in HELPERS {
            if self.helpers.contains(name) {
                out.push_str(source);
                out.push('\n');
            }
        }

        out.push_str("local checks: { [string]: (v: any) -> string? } = {}\n");
        for (key, body) in &self.functions {
            let _ = write!(
                out,
                "\n{} = function(v: any): string?\n{body}\treturn nil\nend\n",
                reference(key)
            );
        }

        out
    }

    fn function(&mut self, key: String, ty: &Ty) {
        // Building the body can start another checker, so the counter is kept per function.
        let outer = std::mem::take(&mut self.locals);
        let mut body = String::new();
        self.statements(ty, "v", &Path::default(), 1, &key, &mut body);
        self.locals = outer;

        self.functions.push((key, body));
    }

    fn local(&mut self, name: &str) -> String {
        self.locals += 1;
        format!("{name}{}", self.locals)
    }

    /// Statements that return a description when `expr` is not a valid `ty`.
    fn statements(
        &mut self,
        ty: &Ty,
        expr: &str,
        path: &Path,
        indent: usize,
        hint: &str,
        out: &mut String,
    ) {
        let pad = "\t".repeat(indent);

        match ty {
            Ty::Table(fields) => {
                let _ = writeln!(out, "{pad}if type({expr}) ~= \"table\" then");
                let _ = writeln!(
                    out,
                    "{pad}\treturn {}",
                    path.then_text(": expected a table")
                );
                let _ = writeln!(out, "{pad}end");

                for (name, field) in fields {
                    self.statements(
                        field,
                        &format!("{expr}.{name}"),
                        &path.field(name),
                        indent,
                        &format!("{hint}.{name}"),
                        out,
                    );
                }

                // Tables are strict: a field the type does not mention is refused.
                let key = self.local("key");
                let unknown = path.key(&key).then_text(": unknown field");
                let _ = writeln!(out, "{pad}for {key} in {expr} do");
                if fields.is_empty() {
                    let _ = writeln!(out, "{pad}\treturn {unknown}");
                } else {
                    let known: Vec<String> = fields
                        .iter()
                        .map(|(name, _)| format!("{key} ~= {}", lua_string(name)))
                        .collect();
                    let _ = writeln!(out, "{pad}\tif {} then", known.join(" and "));
                    let _ = writeln!(out, "{pad}\t\treturn {unknown}");
                    let _ = writeln!(out, "{pad}\tend");
                }
                let _ = writeln!(out, "{pad}end");
            }

            Ty::Array(inner) => {
                let index = self.local("index");
                let item = self.local("item");
                let not_array = path.then_text(": expected an array");

                let _ = writeln!(out, "{pad}if type({expr}) ~= \"table\" then");
                let _ = writeln!(out, "{pad}\treturn {not_array}");
                let _ = writeln!(out, "{pad}end");
                let _ = writeln!(out, "{pad}for {index}, {item} in {expr} do");
                let _ = writeln!(
                    out,
                    "{pad}\tif type({index}) ~= \"number\" or {index} < 1 or {index} > #{expr} or {index} % 1 ~= 0 then"
                );
                let _ = writeln!(out, "{pad}\t\treturn {not_array}");
                let _ = writeln!(out, "{pad}\tend");
                self.statements(
                    inner,
                    &item,
                    &path.index(&index),
                    indent + 1,
                    &format!("{hint}[]"),
                    out,
                );
                let _ = writeln!(out, "{pad}end");
            }

            Ty::Map(value_ty) => {
                let key = self.local("key");
                let value = self.local("value");

                let _ = writeln!(out, "{pad}if type({expr}) ~= \"table\" then");
                let _ = writeln!(out, "{pad}\treturn {}", path.then_text(": expected a map"));
                let _ = writeln!(out, "{pad}end");
                let _ = writeln!(out, "{pad}for {key}, {value} in {expr} do");
                let _ = writeln!(out, "{pad}\tif type({key}) ~= \"string\" then");
                let _ = writeln!(
                    out,
                    "{pad}\t\treturn {}",
                    path.then_text(": expected string keys")
                );
                let _ = writeln!(out, "{pad}\tend");
                self.statements(
                    value_ty,
                    &value,
                    &path.index(&key),
                    indent + 1,
                    &format!("{hint}{{}}"),
                    out,
                );
                let _ = writeln!(out, "{pad}end");
            }

            Ty::Alias(name) => {
                let problem = self.local("problem");
                let _ = writeln!(out, "{pad}local {problem} = {}({expr})", reference(name));
                let _ = writeln!(out, "{pad}if {problem} then");
                let _ = writeln!(out, "{pad}\treturn {}", path.then_variable(&problem));
                let _ = writeln!(out, "{pad}end");
            }

            // Keep the detailed message of the inner type instead of collapsing it to a condition.
            Ty::Optional(inner)
                if matches!(
                    **inner,
                    Ty::Table(_) | Ty::Array(_) | Ty::Map(_) | Ty::Alias(_)
                ) =>
            {
                let _ = writeln!(out, "{pad}if {expr} ~= nil then");
                self.statements(inner, expr, path, indent + 1, hint, out);
                let _ = writeln!(out, "{pad}end");
            }

            // Always valid, so there is nothing to check.
            Ty::Unknown => {}

            _ => {
                let condition = self.condition(ty, expr, hint);
                let expected = format!(": expected {}", ty.to_luau());
                let _ = writeln!(out, "{pad}if {} then", negate(&condition));
                let _ = writeln!(out, "{pad}\treturn {}", path.then_text(&expected));
                let _ = writeln!(out, "{pad}end");
            }
        }
    }

    /// A Luau expression that is true when `expr` is a valid `ty`.
    fn condition(&mut self, ty: &Ty, expr: &str, hint: &str) -> String {
        match ty {
            Ty::String => format!("type({expr}) == \"string\""),
            Ty::Boolean => format!("type({expr}) == \"boolean\""),
            Ty::Buffer => format!("type({expr}) == \"buffer\""),
            Ty::Unknown => "true".into(),
            Ty::Number => {
                self.helpers.insert("isNumber");
                format!("isNumber({expr})")
            }
            Ty::StringLiteral(text) => format!("{expr} == {}", lua_string(text)),
            Ty::BooleanLiteral(value) => format!("{expr} == {value}"),
            Ty::Optional(inner) => {
                format!("({expr} == nil or {})", self.condition(inner, expr, hint))
            }
            Ty::Datatype(name) => {
                self.helpers.insert("isDatatype");
                format!("isDatatype({expr}, {})", lua_string(name))
            }
            Ty::EnumItem(name) => {
                self.helpers.insert("isEnum");
                format!("isEnum({expr}, Enum.{name})")
            }
            Ty::Instance(class) => {
                self.helpers.insert("isInstance");
                format!("isInstance({expr}, {})", lua_string(class))
            }
            Ty::Union(members) => {
                let conditions: Vec<String> = members
                    .iter()
                    .enumerate()
                    .map(|(index, member)| {
                        self.condition(member, expr, &format!("{hint}|{}", index + 1))
                    })
                    .collect();
                format!("({})", conditions.join(" or "))
            }
            // Structural types need statements, so they get a checker of their own.
            Ty::Table(_) | Ty::Array(_) | Ty::Map(_) | Ty::Alias(_) => {
                format!("{}({expr}) == nil", self.checker(ty, hint))
            }
        }
    }
}

/// The opposite of a condition, written the way a person would.
fn negate(condition: &str) -> String {
    let compound = condition.contains(" or ") || condition.contains(" and ");
    if !compound && condition.contains(" == ") {
        condition.replacen(" == ", " ~= ", 1)
    } else {
        // Compound conditions are already wrapped in parentheses.
        format!("not {condition}")
    }
}

/// `checks.Name`, or `checks["a/b"]` when the key is not an identifier.
fn reference(key: &str) -> String {
    let identifier = key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        && !key.starts_with(|c: char| c.is_ascii_digit());
    if identifier && !key.is_empty() {
        format!("checks.{key}")
    } else {
        format!("checks[{}]", lua_string(key))
    }
}

/// Where in the input a value sits, as pieces of a Luau string concatenation.
#[derive(Clone, Default)]
struct Path(Vec<Piece>);

#[derive(Clone)]
enum Piece {
    Text(String),
    /// A Luau variable whose value is shown, such as an array index.
    Shown(String),
    /// A Luau variable that already holds a string.
    Raw(String),
}

impl Path {
    fn field(&self, name: &str) -> Path {
        self.with(vec![Piece::Text(format!(".{name}"))])
    }

    fn index(&self, variable: &str) -> Path {
        self.with(vec![
            Piece::Text("[".into()),
            Piece::Shown(variable.into()),
            Piece::Text("]".into()),
        ])
    }

    fn key(&self, variable: &str) -> Path {
        self.with(vec![Piece::Text(".".into()), Piece::Shown(variable.into())])
    }

    fn then_text(&self, text: &str) -> String {
        self.with(vec![Piece::Text(text.into())]).render()
    }

    fn then_variable(&self, variable: &str) -> String {
        self.with(vec![Piece::Raw(variable.into())]).render()
    }

    fn with(&self, pieces: Vec<Piece>) -> Path {
        let mut path = self.clone();
        path.0.extend(pieces);
        path
    }

    /// Joins the pieces with `..`, merging neighbouring text into one literal.
    fn render(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        let mut text = String::new();

        for piece in &self.0 {
            match piece {
                Piece::Text(more) => text.push_str(more),
                Piece::Shown(variable) | Piece::Raw(variable) => {
                    if !text.is_empty() {
                        parts.push(lua_string(&std::mem::take(&mut text)));
                    }
                    parts.push(match piece {
                        Piece::Shown(_) => format!("tostring({variable})"),
                        _ => variable.clone(),
                    });
                }
            }
        }
        if !text.is_empty() {
            parts.push(lua_string(&text));
        }

        parts.join(" .. ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check(ty: Ty) -> String {
        let mut validators = Validators::default();
        validators.checker(&ty, "input");
        validators.render()
    }

    #[test]
    fn a_table_checks_each_field_and_refuses_unknown_ones() {
        let source = check(Ty::Table(vec![
            ("itemId".into(), Ty::String),
            ("quantity".into(), Ty::Optional(Box::new(Ty::Number))),
        ]));

        assert!(source.contains(r#"if type(v.itemId) ~= "string" then"#));
        assert!(source.contains(r#"return ".itemId: expected string""#));
        assert!(source.contains("if not (v.quantity == nil or isNumber(v.quantity)) then"));
        assert!(source.contains(r#"if key1 ~= "itemId" and key1 ~= "quantity" then"#));
        assert!(source.contains(r#"return "." .. tostring(key1) .. ": unknown field""#));
    }

    #[test]
    fn unknown_is_never_checked() {
        let source = check(Ty::Table(vec![("note".into(), Ty::Unknown)]));
        assert!(!source.contains("v.note"));
        assert!(source.contains(r#"if key1 ~= "note" then"#));
    }

    #[test]
    fn only_used_helpers_are_written() {
        let source = check(Ty::Table(vec![(
            "at".into(),
            Ty::Datatype("Vector3".into()),
        )]));
        assert!(source.contains("local function isDatatype"));
        assert!(!source.contains("local function isNumber"));
    }

    #[test]
    fn an_array_reports_the_index_of_the_bad_item() {
        let source = check(Ty::Array(Box::new(Ty::String)));
        assert!(source.contains(r#"return "[" .. tostring(index1) .. "]: expected string""#));
    }

    #[test]
    fn a_named_type_passes_its_message_up_with_the_path() {
        let source = check(Ty::Table(vec![(
            "item".into(),
            Ty::Alias("ShopItem".into()),
        )]));
        assert!(source.contains("local problem1 = checks.ShopItem(v.item)"));
        assert!(source.contains(r#"return ".item" .. problem1"#));
    }

    #[test]
    fn table_members_of_a_union_get_their_own_checkers() {
        let source = check(Ty::Union(vec![
            Ty::Table(vec![("a".into(), Ty::String)]),
            Ty::StringLiteral("none".into()),
        ]));
        assert!(source.contains(r#"checks["input|1"] = function(v: any): string?"#));
        assert!(source.contains(r#"if not (checks["input|1"](v) == nil or v == "none") then"#));
    }
}
