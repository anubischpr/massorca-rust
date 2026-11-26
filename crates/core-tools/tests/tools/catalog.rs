//! The tools the service asks about on its own host.

use massorca_core_tools::catalog::host_tool_specs;
use massorca_protocol::methods::service::ToolKind;

#[test]
fn the_catalog_lists_forge_clis_and_shells_but_not_git_or_wsl() {
    let specs = host_tool_specs();
    let ids: Vec<&str> = specs.iter().map(|spec| spec.id).collect();

    assert!(ids.contains(&"gh"), "{ids:?}");
    assert!(ids.contains(&"glab"), "{ids:?}");
    assert!(ids.contains(&"pwsh"), "{ids:?}");
    assert_eq!(ids.contains(&"bash"), !cfg!(windows), "{ids:?}");
    assert!(
        !ids.contains(&"git"),
        "git is reported by core-git: {ids:?}"
    );
    assert!(
        ids.iter().all(|id| !id.contains("wsl")),
        "WSL is never probed: {ids:?}"
    );
}

#[test]
fn only_forge_clis_are_asked_for_a_version() {
    for spec in host_tool_specs() {
        match spec.kind {
            ToolKind::Forge => assert!(spec.version_args.is_some(), "{}", spec.id),
            ToolKind::Shell => assert_eq!(spec.version_args, None, "{}", spec.id),
            other => panic!("unexpected kind {other:?} for {}", spec.id),
        }
    }
}
