//! The tools the service looks for on its own host, besides git (which core-git reports).

use massorca_protocol::methods::service::ToolKind;

use crate::probe::ToolSpec;

/// The forge CLIs and shells the service probes. WSL is never probed: no WSL command runs from the service.
/// On Windows `bash` is left out: the `bash.exe` on the `PATH` is usually WSL's launcher, not a shell a terminal runs.
pub fn host_tool_specs() -> Vec<ToolSpec> {
    let mut specs = vec![
        ToolSpec {
            id: "gh",
            kind: ToolKind::Forge,
            program: "gh",
            version_args: Some(&["--version"]),
        },
        ToolSpec {
            id: "glab",
            kind: ToolKind::Forge,
            program: "glab",
            version_args: Some(&["--version"]),
        },
        shell("pwsh"),
    ];
    if cfg!(windows) {
        specs.push(shell("powershell"));
    } else {
        specs.extend(["bash", "zsh", "sh"].map(shell));
    }
    specs
}

/// A shell is found on the `PATH` and never started, so no version is read.
fn shell(name: &'static str) -> ToolSpec {
    ToolSpec {
        id: name,
        kind: ToolKind::Shell,
        program: name,
        version_args: None,
    }
}
