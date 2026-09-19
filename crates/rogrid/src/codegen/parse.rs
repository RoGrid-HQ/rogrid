use std::collections::BTreeSet;
use std::path::Path;

use anyhow::{Result, anyhow, bail};
use full_moon::ast::luau::TypeInfo;
use full_moon::ast::{
    Call, Expression, Field, FunctionArgs, Index, LastStmt, Parameter, Prefix, Suffix,
};
use full_moon::node::Node;

use crate::config;

use super::resolve::Resolver;
use super::{Argument, Event, Module, Side};

/// A deliberately small declaration grammar; handler bodies remain ordinary Luau.
pub fn module(
    path: &Path,
    source: &str,
    side: Side,
    name: String,
    location: Vec<String>,
    resolver: &mut Resolver<'_>,
) -> Result<Module> {
    let ast = syntax(path, source)?;
    declarations(path, &ast, side, name, location, resolver)
}

// full_moon uses recursive descent. A bounded, larger parser stack also handles
// ordinary nested handler code on Windows, whose default thread stack is small.
pub(super) fn syntax(path: &Path, source: &str) -> Result<full_moon::ast::Ast> {
    let parsed = std::thread::scope(|scope| {
        std::thread::Builder::new()
            .stack_size(config::PARSER_STACK_BYTES)
            .spawn_scoped(scope, || full_moon::parse(source))?
            .join()
            .map_err(|_| anyhow!("Luau parser panicked"))
    })?;
    parsed.map_err(|errors| {
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
    })
}

fn declarations(
    path: &Path,
    ast: &full_moon::ast::Ast,
    side: Side,
    name: String,
    location: Vec<String>,
    resolver: &mut Resolver<'_>,
) -> Result<Module> {
    resolver.add(path, ast)?;
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
            if side == Side::Server && index == 0 {
                if !matches!(annotation.type_info(), TypeInfo::Basic(ty) if ty.token().to_string() == "Player")
                {
                    return Err(fail(
                        annotation,
                        "the first server parameter must be the sending Player",
                    ));
                }
                continue;
            }
            let ty = resolver
                .payload(path, annotation.type_info())
                .map_err(|error| fail(annotation, &format!("{error:#}")))?;
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

#[cfg(test)]
mod tests {
    use super::super::{
        emit, sourcemap,
        types::{Leaf, NetworkPayloadType},
    };
    use super::*;

    fn parse(source: &str, side: Side) -> Result<Module> {
        let map: sourcemap::Node =
            serde_json::from_str(r#"{"name":"test","className":"DataModel"}"#).unwrap();
        module(
            Path::new("Test.luau"),
            source,
            side,
            "Test".into(),
            vec!["ServerScriptService".into(), "Test".into()],
            &mut Resolver::new(Path::new("."), &map),
        )
    }

    fn payload(ty: &str, prelude: &str) -> Result<Module> {
        parse(
            &format!(
                "{prelude}\nreturn {{ send = RoGrid.event(function(player: Player, value: {ty}) end) }}"
            ),
            Side::Server,
        )
    }

    #[test]
    fn all_native_names_render_as_valid_callers_and_descriptors() {
        for leaf in Leaf::ALL {
            let module = payload(leaf.name(), "").unwrap();
            assert_eq!(module.events[0].args[0].ty, NetworkPayloadType::Leaf(*leaf));
            for (path, source) in emit::files(&[module], "test") {
                assert!(full_moon::parse(&source).is_ok(), "{path}: {source}");
            }
        }
        for ty in [
            "Instance",
            "Model",
            "BasePart",
            "TextLabel",
            "Enum.Material",
            "Enum.Font",
        ] {
            assert!(payload(ty, "").is_ok(), "{ty}");
        }
    }

    #[test]
    fn containers_optionals_literals_and_aliases() {
        for (ty, prelude, rendered) in [
            ("string?", "", "(string)?"),
            ("Maybe", "type Maybe = string?", "(string)?"),
            ("Maybe?", "type Maybe = string?", "(string)?"),
            ("{number}", "", "{ number }"),
            ("{[number]: number}", "", "{ number }"),
            ("{[string]: Vector3}", "", "{ [string]: Vector3 }"),
            (
                "{[Key]: Vector3}",
                "type Key = string",
                "{ [string]: Vector3 }",
            ),
            ("{[Key]: Vector3}", "type Key = number", "{ Vector3 }"),
            ("{a: string, b: number?}", "", "{ a: string, b: (number)? }"),
            ("false | true", "", "(false | true)"),
            ("number | nil", "", "(number | nil)"),
            (
                "{kind: 'equip', id: string} | {kind: 'clear'}",
                "",
                "({ id: string, kind: 'equip' } | { kind: 'clear' })",
            ),
            (
                "Item",
                "type Id = string\nexport type Item = {id: Id}",
                "{ id: string }",
            ),
            (
                "{['hello-world']: string}",
                "",
                "{ [\"hello-world\"]: string }",
            ),
            (r#""a\255\u{1f600}""#, "", r#""a\255\u{1f600}""#),
        ] {
            let module = payload(ty, prelude).unwrap_or_else(|error| panic!("{ty}: {error:#}"));
            assert_eq!(module.events[0].args[0].ty.to_string(), rendered, "{ty}");
            for (path, source) in emit::files(&[module], "test") {
                full_moon::parse(&source)
                    .unwrap_or_else(|errors| panic!("{path}: {errors:?}\n{source}"));
            }
        }
    }

    #[test]
    fn unsupported_types_have_actionable_errors() {
        for (ty, prelude, message) in [
            ("any", "", "unsupported payload type any"),
            ("unknown", "", "unsupported payload type unknown"),
            ("table", "", "unsupported payload type table"),
            ("TweenInfo", "", "unsupported payload type TweenInfo"),
            ("EditableMesh", "", "unsupported payload type EditableMesh"),
            ("NotAClass", "", "unsupported payload type NotAClass"),
            ("Enum.NotAnEnum", "", "unknown Roblox enum"),
            ("{number?}", "", "array elements cannot be optional"),
            (
                "{Maybe}",
                "type Maybe = number | nil",
                "array elements cannot be optional",
            ),
            ("{a: number, a: string}", "", "duplicate record field a"),
            ("{a: number, ['a']: string}", "", "duplicate record field a"),
            (
                "{[Instance]: string}",
                "",
                "indexers must be string or number",
            ),
            ("{[string]: string, id: string}", "", "mixed table fields"),
            ("A", "type A = {next: A?}", "recursive payload alias"),
            ("A", "type A = B\ntype B = A", "recursive payload alias"),
            ("A<number>", "type A<T> = {T}", "generic payload aliases"),
            ("A", "type A<T> = number", "generic payload aliases"),
            ("string", "type Vector3 = string", "shadows a built-in"),
            (
                "A",
                "type A = string\ntype A = number",
                "duplicate type alias",
            ),
            ("string & number", "", "intersection payload types"),
            ("typeof(1)", "", "computed payload types"),
            ("() -> ()", "", "functions cannot be sent"),
            ("Types.Item", "local Types = getTypes()", "static require"),
        ] {
            let error = payload(ty, prelude)
                .err()
                .unwrap_or_else(|| panic!("accepted {ty}"));
            assert!(format!("{error:#}").contains(message), "{ty}: {error:#}");
        }
    }

    #[test]
    fn event_rules_and_sender_are_preserved() {
        for (source, message) in [
            ("return {e=RoGrid.event(function() end)}", "first parameter"),
            (
                "return {e=RoGrid.event(function(p: number) end)}",
                "sending Player",
            ),
            (
                "return {e=RoGrid.event(function(p: Player, x) end)}",
                "explicit type",
            ),
            (
                "return {e=RoGrid.event(function(p: Player, ...: number) end)}",
                "variadic",
            ),
            (
                "return {e=RoGrid.event(function(p: Player, self: number) end)}",
                "reserved",
            ),
            (
                "return {e=RoGrid.event(function(p: Player, _rogridFoo: number) end)}",
                "reserved",
            ),
            (
                "return {e=RoGrid.event(function(p: Player): number return 1 end)}",
                "do not return a reply",
            ),
            (
                "return {e=RoGrid.event(function<T>(p: Player) end)}",
                "generic event handlers",
            ),
        ] {
            let error = parse(source, Side::Server).err().expect("must reject");
            assert!(error.to_string().contains(message), "{error:#}");
        }
        let source = "return {e=RoGrid.event(function(player: Player) end)}";
        assert_eq!(parse(source, Side::Server).unwrap().events[0].args.len(), 0);
        assert_eq!(parse(source, Side::Client).unwrap().events[0].args.len(), 1);
    }

    #[test]
    fn event_arguments_above_the_former_cap_preserve_names_types_and_order() {
        for side in [Side::Server, Side::Client] {
            for n in [16, 17, 64] {
                let args = (0..n)
                    .map(|i| format!("p{i}: number"))
                    .collect::<Vec<_>>()
                    .join(", ");
                let sender = if side == Side::Server {
                    "player: Player, "
                } else {
                    ""
                };
                let source = format!("return {{e=RoGrid.event(function({sender}{args}) end)}}");
                let module = parse(&source, side).unwrap();
                assert_eq!(module.events[0].args.len(), n);
                for (i, arg) in module.events[0].args.iter().enumerate() {
                    assert_eq!(arg.name, format!("p{i}"));
                    assert_eq!(arg.ty, NetworkPayloadType::Leaf(Leaf::Number));
                }
                for (path, source) in emit::files(&[module], "test") {
                    assert!(full_moon::parse(&source).is_ok(), "{path}: {source}");
                }
            }
        }
    }

    #[test]
    fn invalid_event_declarations_report_the_documented_rule() {
        for (source, expected) in [
            ("local x = 1", "event modules must end"),
            (
                "local events = {}; return events",
                "return one literal table",
            ),
            ("return {}, {}", "return one literal table"),
            ("return {}", "at least one event"),
            (
                "return { RoGrid.event(function() end) }",
                "use a named field",
            ),
            (
                "return { ['e'] = RoGrid.event(function() end) }",
                "use a named field",
            ),
            ("return { e = function() end }", "expected RoGrid.event"),
            (
                "return { e = Other.event(function() end) }",
                "only RoGrid.event",
            ),
            (
                "return { e = RoGrid.other(function() end) }",
                "only RoGrid.event",
            ),
            (
                "return { e = RoGrid:event(function() end) }",
                "expected RoGrid.event",
            ),
            (
                "local f = function() end; return { e = RoGrid.event(f) }",
                "one inline function",
            ),
            (
                "return { e = RoGrid.event(function() end, true) }",
                "one inline function",
            ),
            (
                "return { e = RoGrid.event(function() end), e = RoGrid.event(function() end) }",
                "duplicate event name",
            ),
            (
                "return { e = RoGrid.event(function(x: number, x: number) end) }",
                "duplicate parameter name",
            ),
        ] {
            let error = parse(source, Side::Client).err().expect(source);
            assert!(
                format!("{error:#}").contains(expected),
                "{source}: {error:#}"
            );
        }
    }

    #[test]
    fn syntax_errors_identify_the_source_line_and_column() {
        let error = syntax(Path::new("Combat.luau"), "local x = 1\nreturn {\n")
            .err()
            .unwrap();
        let message = format!("{error:#}");
        assert!(message.contains("Combat.luau:"), "{message}");
        let position = message.split("Combat.luau:").nth(1).unwrap();
        let mut parts = position.split(':');
        assert!(parts.next().unwrap().parse::<usize>().unwrap() >= 2);
        assert!(parts.next().unwrap().parse::<usize>().unwrap() >= 1);
    }

    #[test]
    fn multiple_events_are_sorted_and_allow_explicit_empty_returns() {
        let module = parse("return { z = RoGrid.event(function(): () end), a = RoGrid.event(function(value: number) end) }", Side::Client).unwrap();
        assert_eq!(
            module
                .events
                .iter()
                .map(|event| event.name.as_str())
                .collect::<Vec<_>>(),
            ["a", "z"]
        );
        assert!(module.events[1].args.is_empty());
    }

    #[test]
    fn large_alias_expansions_preserve_every_leaf() {
        let mut prelude = "type A0 = number\n".to_string();
        for i in 1..=13 {
            prelude.push_str(&format!(
                "type A{i} = {{left: A{}, right: A{}}}\n",
                i - 1,
                i - 1
            ));
        }
        let module = payload("A13", &prelude).unwrap();
        let ty = &module.events[0].args[0].ty;
        assert_eq!(ty.to_string().matches("number").count(), 8192);
        assert_eq!(ty.descriptor().matches("\"number\"").count(), 8192);
        for (path, source) in emit::files(&[module], "test") {
            syntax(Path::new(&path), &source).unwrap();
        }
    }

    #[test]
    fn large_records_and_combined_arguments_have_no_schema_budget() {
        for (fields, arguments) in [(4095, 1), (4096, 1), (4097, 1), (2048, 2)] {
            let fields_source = (0..fields)
                .map(|i| format!("f{i}: number"))
                .collect::<Vec<_>>()
                .join(", ");
            for side in [Side::Server, Side::Client] {
                let params = (0..arguments)
                    .map(|i| format!("p{i}: Item"))
                    .collect::<Vec<_>>()
                    .join(", ");
                let sender = if side == Side::Server {
                    "player: Player, "
                } else {
                    ""
                };
                let source = format!(
                    "type Item = {{{fields_source}}}\nreturn {{send = RoGrid.event(function({sender}{params}) end)}}"
                );
                let module = parse(&source, side).unwrap();
                assert_eq!(module.events[0].args.len(), arguments);
                for arg in &module.events[0].args {
                    let NetworkPayloadType::Record(record) = &arg.ty else {
                        panic!("expected record")
                    };
                    assert_eq!(record.len(), fields);
                    assert!(
                        record
                            .iter()
                            .all(|field| field.ty == NetworkPayloadType::Leaf(Leaf::Number))
                    );
                }
            }
        }
        let fields = (0..4097)
            .map(|i| format!("f{i}: number, "))
            .collect::<String>();
        for (last, prelude, expected) in [
            ("any", "", "unsupported payload type any"),
            (
                "Loop",
                "type Loop = {next: Loop}\n",
                "recursive payload alias Loop",
            ),
        ] {
            let error = payload(&format!("{{{fields}last: {last}}}"), prelude)
                .err()
                .unwrap();
            assert!(format!("{error:#}").contains(expected));
        }
    }

    #[test]
    fn deep_inline_types_generate_annotations_descriptors_and_reports() {
        for depth in [33, 128] {
            let ty = format!("{}number{}", "{ child: ".repeat(depth), " }".repeat(depth));
            let module = payload(&ty, "").unwrap();
            assert_eq!(module.events[0].args[0].ty.to_string(), ty);
            assert_eq!(
                module.events[0].args[0]
                    .ty
                    .descriptor()
                    .matches("record =")
                    .count(),
                depth
            );
            let modules = [module];
            assert!(
                super::super::report::inventory(&modules)
                    .values()
                    .any(|signature| signature == &format!("value: {ty}"))
            );
            let generated = emit::files(&modules, "test");
            assert!(generated["shared/Server.luau"].contains(&format!("value: {ty}")));
            assert!(
                generated["server/Start.luau"]
                    .contains(&modules[0].events[0].args[0].ty.descriptor())
            );
            for (path, source) in generated {
                syntax(Path::new(&path), &source).unwrap();
            }
        }
    }

    #[test]
    fn alias_chains_beyond_the_former_size_cap_resolve() {
        let mut prelude = "type A0 = number\n".to_string();
        for i in 1..=5000 {
            prelude.push_str(&format!("type A{i} = A{}\n", i - 1));
        }
        for name in ["A4094", "A4095", "A5000"] {
            let module = payload(name, &prelude).unwrap();
            assert_eq!(module.events[0].args[0].ty.to_string(), "number");
        }
        let cyclic = prelude.replace("type A0 = number", "type A0 = A5000");
        let error = payload("A5000", &cyclic).err().unwrap();
        assert!(format!("{error:#}").contains("recursive payload alias A5000"));
    }

    #[test]
    fn nested_aliases_keep_nil_checks_and_cycle_detection() {
        let mut arrays = "type A0 = number\n".to_string();
        let mut unions = "type A0 = nil\n".to_string();
        for i in 1..=1000 {
            arrays.push_str(&format!("type A{i} = {{A{}}}\n", i - 1));
            unions.push_str(&format!("type A{i} = A{} | number\n", i - 1));
        }
        let module = payload("A1000", &arrays).unwrap();
        let ty = &module.events[0].args[0].ty;
        assert_eq!(ty.to_string().matches('{').count(), 1000);
        assert_eq!(ty.descriptor().matches("array =").count(), 1000);
        assert!(!ty.accepts_nil());
        assert!(
            payload("{A1000}", &unions)
                .err()
                .unwrap()
                .to_string()
                .contains("array elements cannot be optional")
        );
        let cyclic = arrays.replace("type A0 = number", "type A0 = A1000");
        assert!(
            format!("{:#}", payload("A1000", &cyclic).err().unwrap())
                .contains("recursive payload alias")
        );
    }

    #[test]
    fn generated_contract_has_concrete_types_and_named_descriptors() {
        let module = payload(
            "Item?",
            "type Item = {id: string, flags: {boolean}, kind: 'a' | 'b'}",
        )
        .unwrap();
        let files = emit::files(&[module], "test");
        assert!(
            files["shared/Server.luau"]
                .contains("value: ({ flags: { boolean }, id: string, kind: ('a' | 'b') })?")
        );
        assert!(files["server/Start.luau"].contains(
            r#"{ name = "value", shape = (function(): any
local shapes: {any} = {}
shapes[1] = { literal = 'b' }
shapes[2] = { literal = 'a' }
shapes[3] = "boolean"
shapes[4] = { union = ({ shapes[2], shapes[1] } :: { any }) }
shapes[5] = "string"
shapes[6] = { array = shapes[3] }
shapes[7] = { record = { ["flags"] = shapes[6], ["id"] = shapes[5], ["kind"] = shapes[4] } }
shapes[8] = { optional = shapes[7] }
return shapes[8]
end)() },"#
        ));
        assert!(!files["server/Start.luau"].contains("function(..."));
        assert!(files["server/Start.luau"].contains("_protocol == 3"));
    }

    #[test]
    #[ignore = "requires luau-lsp on PATH and ROGRID_ROBLOX_DEFINITIONS pointing to Roblox definitions"]
    fn payloads_pass_luau_typechecking() {
        use std::{env, fs, process::Command};
        let definitions =
            env::var_os("ROGRID_ROBLOX_DEFINITIONS").expect("set ROGRID_ROBLOX_DEFINITIONS");
        let mut types: Vec<_> = Leaf::ALL.iter().map(|leaf| leaf.name()).collect();
        types.extend([
            "Instance",
            "BasePart",
            "TextLabel",
            "Enum.Material",
            "nil",
            "false | true",
            "{kind: 'equip', id: string} | {kind: 'clear'}",
            "{value: string | {id: number}, position: Vector3?, flags: {[string]: boolean}}?",
            "{Vector3}",
            "{[string]: {id: string, value: number?}}",
            "{['hello-world']: string}",
        ]);
        let deep = format!("{}number{}", "{ child: ".repeat(128), " }".repeat(128));
        types.push(&deep);
        let wide = format!(
            "{{ {} }}",
            (1..=5000)
                .map(|i| format!("f{i}: number"))
                .collect::<Vec<_>>()
                .join(", ")
        );
        types.push(&wide);
        let mut descriptors = Vec::new();
        let mut callers = Vec::new();
        for (i, source) in types.iter().enumerate() {
            let module = payload(source, "").unwrap();
            let ty = &module.events[0].args[0].ty;
            descriptors.push(ty.descriptor());
            callers.push(format!("f{i} = function(_value: {ty}): () end"));
        }
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("payloads.luau");
        fs::write(&file, format!("--!strict\nlocal shapes: {{any}} = {{ {} }}\nreturn {{ callers = {{ {} }}, shapes = shapes }}\n", descriptors.join(",\n"), callers.join(",\n"))).unwrap();
        let output = Command::new("luau-lsp")
            .args(["analyze", "--platform=roblox", "--definitions"])
            .arg(definitions)
            .arg(file)
            .output()
            .expect("run luau-lsp");
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
