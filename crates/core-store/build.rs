//! Writes the one-file-per-step table into OUT_DIR from `migrations/`; the crate includes it (decision 0119).

#[path = "src/migration_folder.rs"]
mod migration_folder;

use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    let manifest_dir =
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR"));
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("cargo sets OUT_DIR"));
    let folder = manifest_dir.join("migrations");
    println!("cargo:rerun-if-changed=migrations");
    let steps = migration_folder::scan(&folder).unwrap_or_else(|error| panic!("{error}"));
    for step in &steps {
        println!("cargo:rerun-if-changed=migrations/{}", step.file_name);
    }
    fs::write(
        out_dir.join("file_migrations.rs"),
        migration_folder::render(&steps),
    )
    .expect("cannot write the generated migration table");
}
