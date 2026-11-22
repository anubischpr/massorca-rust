//! Platform resources for the desktop binary.

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        windows();
    }
}

/// Embeds `resources/windows/massorca.rc`, whose icon id 1 GPUI uses for the window class.
fn windows() {
    let dir = std::path::Path::new("resources/windows");
    println!(
        "cargo:rerun-if-changed={}",
        dir.join("massorca.rc").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        dir.join("massorca.ico").display()
    );
    embed_resource::compile(
        dir.join("massorca.rc"),
        embed_resource::ParamsIncludeDirs([dir]),
    )
    .manifest_required()
    .unwrap();
}
