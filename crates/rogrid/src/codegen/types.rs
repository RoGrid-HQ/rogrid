//! Network payload types only. Other framework features are not restricted by this model.
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NetworkPayloadType {
    Leaf(Leaf),
    Instance(String),
    Enum(String),
    StringLiteral(String),
    BooleanLiteral(bool),
    Nil,
    Optional(Box<Self>),
    Array(Box<Self>),
    Dictionary(Box<Self>),
    Record(Vec<Field>),
    Union(Vec<Self>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Field {
    pub name: String,
    pub ty: NetworkPayloadType,
}

// Each built-in name is written once here; this supplies parsing and rendering.
macro_rules! leaves {
    ($($variant:ident => $name:literal),* $(,)?) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum Leaf { $($variant),* }
        impl Leaf {
            #[cfg(test)]
            pub const ALL: &'static [Self] = &[$(Self::$variant),*];
            pub fn parse(name: &str) -> Option<Self> {
                match name { $($name => Some(Self::$variant),)* _ => None }
            }
            pub fn name(self) -> &'static str {
                match self { $(Self::$variant => $name,)* }
            }
        }
    };
}

leaves! {
    String => "string", Number => "number", Boolean => "boolean", Buffer => "buffer",
    Vector2 => "Vector2", Vector3 => "Vector3", Vector2int16 => "Vector2int16",
    Vector3int16 => "Vector3int16", CFrame => "CFrame", Color3 => "Color3",
    BrickColor => "BrickColor", UDim => "UDim", UDim2 => "UDim2", Rect => "Rect",
    Ray => "Ray", Region3 => "Region3", Region3int16 => "Region3int16",
    NumberRange => "NumberRange", NumberSequence => "NumberSequence",
    NumberSequenceKeypoint => "NumberSequenceKeypoint", ColorSequence => "ColorSequence",
    ColorSequenceKeypoint => "ColorSequenceKeypoint", DateTime => "DateTime",
    Axes => "Axes", Faces => "Faces", PhysicalProperties => "PhysicalProperties",
    Font => "Font", EnumItem => "EnumItem",
}

pub fn is_class(name: &str) -> bool {
    include_str!("classes.txt").lines().any(|item| item == name)
}

pub fn is_enum(name: &str) -> bool {
    include_str!("enums.txt").lines().any(|item| item == name)
}

/// Quote UTF-8 text as Luau, including control bytes. JSON's \uXXXX is not Luau syntax.
pub fn quote(value: &str) -> String {
    let mut result = String::from("\"");
    for ch in value.chars() {
        match ch {
            '"' => result.push_str("\\\""),
            '\\' => result.push_str("\\\\"),
            ch if ch.is_control() => {
                use fmt::Write;
                write!(result, "\\u{{{:x}}}", ch as u32).unwrap();
            }
            ch => result.push(ch),
        }
    }
    result.push('"');
    result
}

impl NetworkPayloadType {
    pub fn optional(self) -> Self {
        if self.accepts_nil() {
            self
        } else {
            Self::Optional(Box::new(self))
        }
    }

    pub fn accepts_nil(&self) -> bool {
        match self {
            Self::Nil | Self::Optional(_) => true,
            Self::Union(types) => types.iter().any(Self::accepts_nil),
            _ => false,
        }
    }

    pub fn descriptor(&self) -> String {
        match self {
            Self::Leaf(leaf) => quote(leaf.name()),
            Self::Instance(name) => format!("{{ instance = {} }}", quote(name)),
            Self::Enum(name) => format!("{{ enum = {} }}", quote(name)),
            Self::StringLiteral(value) => format!("{{ literal = {value} }}"),
            Self::BooleanLiteral(value) => format!("{{ literal = {value} }}"),
            Self::Nil => quote("nil"),
            Self::Optional(inner) => format!("{{ optional = {} }}", inner.descriptor()),
            Self::Array(inner) => format!("{{ array = {} }}", inner.descriptor()),
            Self::Dictionary(inner) => format!("{{ dictionary = {} }}", inner.descriptor()),
            Self::Record(fields) => format!(
                "{{ record = {{ {} }} }}",
                fields
                    .iter()
                    .map(|field| format!("[{}] = {}", quote(&field.name), field.ty.descriptor()))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            // Luau otherwise infers the first branch's exact table shape for the
            // entire descriptor list. This annotation is internal, not a caller type.
            Self::Union(types) => format!(
                "{{ union = ({{ {} }} :: {{ any }}) }}",
                types
                    .iter()
                    .map(Self::descriptor)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        }
    }
}

impl fmt::Display for NetworkPayloadType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Leaf(leaf) => f.write_str(leaf.name()),
            Self::Instance(name) => f.write_str(name),
            Self::Enum(name) => write!(f, "Enum.{name}"),
            Self::StringLiteral(value) => f.write_str(value),
            Self::BooleanLiteral(value) => write!(f, "{value}"),
            Self::Nil => f.write_str("nil"),
            Self::Optional(inner) => write!(f, "({inner})?"),
            Self::Array(inner) => write!(f, "{{ {inner} }}"),
            Self::Dictionary(inner) => write!(f, "{{ [string]: {inner} }}"),
            Self::Record(fields) => {
                f.write_str("{ ")?;
                for (index, field) in fields.iter().enumerate() {
                    if index > 0 {
                        f.write_str(", ")?;
                    }
                    if super::identifier(&field.name) {
                        write!(f, "{}: {}", field.name, field.ty)?;
                    } else {
                        write!(f, "[{}]: {}", quote(&field.name), field.ty)?;
                    }
                }
                f.write_str(" }")
            }
            Self::Union(types) => {
                f.write_str("(")?;
                for (index, ty) in types.iter().enumerate() {
                    if index > 0 {
                        f.write_str(" | ")?;
                    }
                    write!(f, "{ty}")?;
                }
                f.write_str(")")
            }
        }
    }
}
