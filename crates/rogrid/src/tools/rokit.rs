use super::{Manifest, ToolManager};

pub const TOOL_MANAGER: ToolManager = ToolManager {
    name: "rokit",
    binary: Some("rokit"),
    homepage: "https://github.com/rojo-rbx/rokit",
    manifest: Some(Manifest {
        file: "rokit.toml",
        template: "[tools]\n{{pins}}\n",
    }),
    pin_line: r#"{{alias}} = "{{spec}}""#,
    install: &["rokit trust {{pin_ids}}", "rokit install"],
    installs_tools: true,
    only_with: None,
};
