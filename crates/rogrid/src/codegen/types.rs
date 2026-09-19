//! Network payload types only. Other framework features are not restricted by this model.
use std::collections::HashMap;
use std::fmt::{self, Write};

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
        let mut pending = vec![self];
        while let Some(ty) = pending.pop() {
            match ty {
                Self::Nil | Self::Optional(_) => return true,
                Self::Union(types) => pending.extend(types),
                _ => {}
            }
        }
        false
    }

    pub fn descriptor(&self) -> String {
        let mut nodes = vec![self];
        let mut next = 0;
        while next < nodes.len() {
            match nodes[next] {
                Self::Optional(inner) | Self::Array(inner) | Self::Dictionary(inner) => {
                    nodes.push(inner)
                }
                Self::Record(fields) => nodes.extend(fields.iter().map(|field| &field.ty)),
                Self::Union(types) => nodes.extend(types),
                _ => {}
            }
            next += 1;
        }
        let mut result = String::new();
        let mut references = HashMap::new();
        if nodes.len() == 1 {
            self.render(&mut result, true, &references).unwrap();
            return result;
        }
        // Build one layer per statement to avoid Luau's register limit on deeply
        // nested table literals. The temporary list is discarded after startup.
        result.push_str("(function(): any\nlocal shapes: {any} = {}\n");
        let count = nodes.len();
        for (index, ty) in nodes.into_iter().rev().enumerate() {
            let id = index + 1;
            write!(result, "shapes[{id}] = ").unwrap();
            ty.render(&mut result, true, &references).unwrap();
            result.push('\n');
            // Addresses identify borrowed nodes only; no raw pointer is dereferenced.
            references.insert(ty as *const Self, id);
        }
        write!(result, "return shapes[{count}]\nend)()").unwrap();
        result
    }

    // Both public annotations and runtime descriptors walk the same type tree
    // with a work list; neither adds a Rust call for each nesting level.
    fn render(
        &self,
        out: &mut impl fmt::Write,
        descriptor: bool,
        references: &HashMap<*const Self, usize>,
    ) -> fmt::Result {
        enum Part<'a> {
            Type(&'a NetworkPayloadType),
            Field(&'a Field),
            Text(&'static str),
        }
        let mut pending = vec![Part::Type(self)];
        while let Some(part) = pending.pop() {
            match part {
                Part::Text(text) => out.write_str(text)?,
                Part::Field(field) => {
                    if descriptor {
                        write!(out, "[{}] = ", quote(&field.name))?;
                    } else if super::identifier(&field.name) {
                        write!(out, "{}: ", field.name)?;
                    } else {
                        write!(out, "[{}]: ", quote(&field.name))?;
                    }
                }
                Part::Type(ty) => {
                    if let Some(id) = references.get(&(ty as *const Self)) {
                        write!(out, "shapes[{id}]")?;
                        continue;
                    }
                    match ty {
                        Self::Leaf(leaf) => out.write_str(&if descriptor {
                            quote(leaf.name())
                        } else {
                            leaf.name().to_string()
                        })?,
                        Self::Instance(name) => {
                            if descriptor {
                                write!(out, "{{ instance = {} }}", quote(name))?;
                            } else {
                                out.write_str(name)?;
                            }
                        }
                        Self::Enum(name) => {
                            if descriptor {
                                write!(out, "{{ enum = {} }}", quote(name))?;
                            } else {
                                write!(out, "Enum.{name}")?;
                            }
                        }
                        Self::StringLiteral(value) => {
                            if descriptor {
                                write!(out, "{{ literal = {value} }}")?;
                            } else {
                                out.write_str(value)?;
                            }
                        }
                        Self::BooleanLiteral(value) => {
                            if descriptor {
                                write!(out, "{{ literal = {value} }}")?;
                            } else {
                                write!(out, "{value}")?;
                            }
                        }
                        Self::Nil => out.write_str(if descriptor { "\"nil\"" } else { "nil" })?,
                        Self::Optional(inner) | Self::Array(inner) | Self::Dictionary(inner) => {
                            let (open, close) = match (ty, descriptor) {
                                (Self::Optional(_), true) => ("{ optional = ", " }"),
                                (Self::Array(_), true) => ("{ array = ", " }"),
                                (Self::Dictionary(_), true) => ("{ dictionary = ", " }"),
                                (Self::Optional(_), false) => ("(", ")?"),
                                (Self::Array(_), false) => ("{ ", " }"),
                                _ => ("{ [string]: ", " }"),
                            };
                            out.write_str(open)?;
                            pending.push(Part::Text(close));
                            pending.push(Part::Type(inner));
                        }
                        Self::Record(fields) => {
                            out.write_str(if descriptor { "{ record = { " } else { "{ " })?;
                            pending.push(Part::Text(if descriptor { " } }" } else { " }" }));
                            for (index, field) in fields.iter().enumerate().rev() {
                                pending.push(Part::Type(&field.ty));
                                pending.push(Part::Field(field));
                                if index > 0 {
                                    pending.push(Part::Text(", "));
                                }
                            }
                        }
                        Self::Union(types) => {
                            // The cast keeps Luau from inferring the first branch's exact shape.
                            out.write_str(if descriptor { "{ union = ({ " } else { "(" })?;
                            pending.push(Part::Text(if descriptor {
                                " } :: { any }) }"
                            } else {
                                ")"
                            }));
                            for (index, ty) in types.iter().enumerate().rev() {
                                pending.push(Part::Type(ty));
                                if index > 0 {
                                    pending.push(Part::Text(if descriptor { ", " } else { " | " }));
                                }
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

impl fmt::Display for NetworkPayloadType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.render(f, false, &HashMap::new())
    }
}

// Free deep type trees iteratively as well, including partially resolved types
// on an error path. Otherwise dropping Box/Vec children reintroduces recursion.
impl Drop for NetworkPayloadType {
    fn drop(&mut self) {
        fn children(ty: &mut NetworkPayloadType, pending: &mut Vec<NetworkPayloadType>) {
            match ty {
                NetworkPayloadType::Optional(inner)
                | NetworkPayloadType::Array(inner)
                | NetworkPayloadType::Dictionary(inner) => {
                    pending.push(std::mem::replace(inner.as_mut(), NetworkPayloadType::Nil));
                }
                NetworkPayloadType::Record(fields) => {
                    pending.extend(std::mem::take(fields).into_iter().map(|field| field.ty));
                }
                NetworkPayloadType::Union(types) => pending.append(types),
                _ => {}
            }
        }
        let mut pending = Vec::new();
        children(self, &mut pending);
        while let Some(mut ty) = pending.pop() {
            children(&mut ty, &mut pending);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deep_type_rendering_nil_checks_and_drop_use_a_work_list() {
        std::thread::Builder::new()
            .stack_size(256 * 1024)
            .spawn(|| {
                let mut ty = NetworkPayloadType::Nil;
                for _ in 0..10_000 {
                    ty =
                        NetworkPayloadType::Union(vec![NetworkPayloadType::Leaf(Leaf::Number), ty]);
                }
                assert!(ty.accepts_nil());
                assert_eq!(ty.to_string().matches("number").count(), 10_000);
                assert_eq!(ty.descriptor().matches("union =").count(), 10_000);
                drop(ty);
                let mut ty = NetworkPayloadType::Leaf(Leaf::String);
                for i in 0..10_000 {
                    ty = match i % 4 {
                        0 => NetworkPayloadType::Array(Box::new(ty)),
                        1 => NetworkPayloadType::Dictionary(Box::new(ty)),
                        2 => NetworkPayloadType::Optional(Box::new(ty)),
                        _ => NetworkPayloadType::Record(vec![Field {
                            name: "child".into(),
                            ty,
                        }]),
                    };
                }
                assert!(!ty.accepts_nil());
                assert!(ty.to_string().contains("string"));
                assert!(ty.descriptor().contains("\"string\""));
                drop(ty);
            })
            .unwrap()
            .join()
            .unwrap();
    }
}
