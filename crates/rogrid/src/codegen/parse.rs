use std::collections::BTreeSet;
use std::path::Path;

use anyhow::{Result, anyhow, bail};
use full_moon::ast::luau::TypeInfo;
use full_moon::ast::{
    Call, Expression, Field, FunctionArgs, Index, LastStmt, Parameter, Prefix, Suffix,
};
use full_moon::node::Node;

use super::{Argument, Event, Module, Side};

/// A deliberately small declaration grammar; handler bodies remain ordinary Luau.
pub fn module(
    path: &Path,
    source: &str,
    side: Side,
    name: String,
    location: Vec<String>,
) -> Result<Module> {
    let ast = full_moon::parse(source).map_err(|errors| {
        anyhow!(
            errors
                .iter()
                .map(|error| {
                    let position = error.range().0;
                    format!(
                        "{}:{}:{}: {}",
                        path.display(),
                        position.line(),
                        position.character(),
                        error.error_message()
                    )
                })
                .collect::<Vec<_>>()
                .join("\n")
        )
    })?;
    let fail = |node: &dyn Node, message: &str| {
        let position = node.start_position().unwrap_or_default();
        anyhow!(
            "{}:{}:{}: {message}",
            path.display(),
            position.line(),
            position.character()
        )
    };
    let Some(LastStmt::Return(returned)) = ast.nodes().last_stmt() else {
        bail!(
            "{}:1:1: event modules must end with `return {{ name = RoGrid.event(function(...) ... end) }}`",
            path.display()
        );
    };
    let values: Vec<_> = returned.returns().iter().collect();
    let [Expression::TableConstructor(table)] = values.as_slice() else {
        return Err(fail(
            returned,
            "return one literal table of named RoGrid.event declarations",
        ));
    };
    let mut names = BTreeSet::new();
    let mut events = Vec::new();
    for field in table.fields().iter() {
        let Field::NameKey { key, value, .. } = field else {
            return Err(fail(
                field,
                "use a named field: name = RoGrid.event(function(...) ... end)",
            ));
        };
        let event_name = key.token().to_string();
        if !names.insert(event_name.clone()) {
            return Err(fail(key, "duplicate event name"));
        }
        let Expression::FunctionCall(call) = value else {
            return Err(fail(value, "expected RoGrid.event(function(...) ... end)"));
        };
        let suffixes: Vec<_> = call.suffixes().collect();
        let [
            Suffix::Index(Index::Dot { name: member, .. }),
            Suffix::Call(Call::AnonymousCall(FunctionArgs::Parentheses { arguments, .. })),
        ] = suffixes.as_slice()
        else {
            return Err(fail(call, "expected RoGrid.event(function(...) ... end)"));
        };
        if !matches!(call.prefix(), Prefix::Name(token) if token.token().to_string() == "RoGrid")
            || member.token().to_string() != "event"
        {
            return Err(fail(
                call,
                "only RoGrid.event declarations are supported in event modules",
            ));
        }
        let args: Vec<_> = arguments.iter().collect();
        let [Expression::Function(function)] = args.as_slice() else {
            return Err(fail(call, "RoGrid.event takes one inline function"));
        };
        let body = function.body();
        if body.generics().is_some() {
            return Err(fail(body, "generic event handlers are not supported"));
        }
        if let Some(result) = body.return_type()
            && !matches!(result.type_info(), TypeInfo::Tuple { types, .. } if types.is_empty())
        {
            return Err(fail(
                result,
                "events do not return a reply; omit the return annotation or use ()",
            ));
        }
        let mut params = Vec::new();
        let mut param_names = BTreeSet::new();
        let mut annotations = body.type_specifiers();
        for (index, parameter) in body.parameters().iter().enumerate() {
            let Parameter::Name(token) = parameter else {
                return Err(fail(
                    parameter,
                    "variadic event arguments are not supported",
                ));
            };
            let param_name = token.token().to_string();
            if !param_names.insert(param_name.clone()) {
                return Err(fail(parameter, "duplicate parameter name"));
            }
            if param_name == "self" || param_name.starts_with("_rogrid") {
                return Err(fail(
                    parameter,
                    "parameter names `self` and `_rogrid...` are reserved",
                ));
            }
            let Some(Some(annotation)) = annotations.next() else {
                return Err(fail(
                    parameter,
                    "every event parameter needs an explicit type",
                ));
            };
            let TypeInfo::Basic(ty) = annotation.type_info() else {
                return Err(fail(
                    annotation,
                    "supported payload types are string, boolean, and number",
                ));
            };
            let ty = ty.token().to_string();
            if side == Side::Server && index == 0 {
                if ty != "Player" {
                    return Err(fail(
                        annotation,
                        "the first server parameter must be the sending Player",
                    ));
                }
                continue;
            }
            let ty = match ty.as_str() {
                "string" => "string",
                "boolean" => "boolean",
                "number" => "number",
                _ => {
                    return Err(fail(
                        annotation,
                        "supported payload types are string, boolean, and number (no aliases yet)",
                    ));
                }
            };
            params.push(Argument {
                name: param_name,
                ty,
            });
        }
        if side == Side::Server && body.parameters().is_empty() {
            return Err(fail(
                body,
                "server handlers require a first parameter typed Player",
            ));
        }
        if params.len() > 16 {
            return Err(fail(body, "events support at most 16 payload arguments"));
        }
        events.push(Event {
            name: event_name,
            args: params,
        });
    }
    if events.is_empty() {
        return Err(fail(table, "event modules must declare at least one event"));
    }
    events.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(Module {
        side,
        name,
        events,
        location,
    })
}
