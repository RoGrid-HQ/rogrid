//! The types RoGrid can send over the network and check at runtime.

/// Roblox datatypes, checked with `typeof`. Any other capitalised name is treated
/// as an Instance class and checked with `IsA`.
pub const DATATYPES: &[&str] = &[
    "BrickColor",
    "CFrame",
    "Color3",
    "ColorSequence",
    "DateTime",
    "Font",
    "NumberRange",
    "NumberSequence",
    "Ray",
    "Rect",
    "Region3",
    "TweenInfo",
    "UDim",
    "UDim2",
    "Vector2",
    "Vector2int16",
    "Vector3",
    "Vector3int16",
];

#[derive(Debug, Clone, PartialEq)]
pub enum Ty {
    String,
    Number,
    Boolean,
    Buffer,
    /// The explicit opt-out: not checked, and the handler has to narrow it.
    Unknown,
    StringLiteral(String),
    BooleanLiteral(bool),
    Optional(Box<Ty>),
    Array(Box<Ty>),
    /// A map with string keys. Roblox cannot send other key types.
    Map(Box<Ty>),
    Table(Vec<(String, Ty)>),
    Union(Vec<Ty>),
    Datatype(String),
    /// A member of the named Enum, such as `Enum.Material`.
    EnumItem(String),
    Instance(String),
    /// A named type from the same file, already prefixed with the function's name.
    Alias(String),
}

impl Ty {
    /// The type as Luau source, for generated aliases and for error messages.
    pub fn to_luau(&self) -> String {
        match self {
            Ty::String => "string".into(),
            Ty::Number => "number".into(),
            Ty::Boolean => "boolean".into(),
            Ty::Buffer => "buffer".into(),
            Ty::Unknown => "unknown".into(),
            Ty::StringLiteral(text) => lua_string(text),
            Ty::BooleanLiteral(value) => value.to_string(),
            Ty::Optional(inner) => match **inner {
                Ty::Union(_) => format!("({})?", inner.to_luau()),
                _ => format!("{}?", inner.to_luau()),
            },
            Ty::Array(inner) => format!("{{ {} }}", inner.to_luau()),
            Ty::Map(value) => format!("{{ [string]: {} }}", value.to_luau()),
            Ty::Table(fields) if fields.is_empty() => "{}".into(),
            Ty::Table(fields) => {
                let fields: Vec<String> = fields
                    .iter()
                    .map(|(name, ty)| format!("{name}: {}", ty.to_luau()))
                    .collect();
                format!("{{ {} }}", fields.join(", "))
            }
            Ty::Union(members) => members
                .iter()
                .map(Ty::to_luau)
                .collect::<Vec<_>>()
                .join(" | "),
            Ty::Datatype(name) | Ty::Instance(name) | Ty::Alias(name) => name.clone(),
            Ty::EnumItem(name) => format!("Enum.{name}"),
        }
    }
}

/// A Luau string literal holding `text`.
pub fn lua_string(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            _ => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prints_luau_types() {
        let ty = Ty::Table(vec![
            ("itemId".into(), Ty::String),
            ("quantity".into(), Ty::Optional(Box::new(Ty::Number))),
            ("tags".into(), Ty::Array(Box::new(Ty::String))),
        ]);
        assert_eq!(
            ty.to_luau(),
            "{ itemId: string, quantity: number?, tags: { string } }"
        );
    }

    #[test]
    fn parenthesises_an_optional_union() {
        let union = Ty::Union(vec![Ty::StringLiteral("a".into()), Ty::Number]);
        assert_eq!(Ty::Optional(Box::new(union)).to_luau(), "(\"a\" | number)?");
    }
}
