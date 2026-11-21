//! The `massorca` binary: a thin shell over the `massorca_cli` library.

use std::process::ExitCode;

fn main() -> ExitCode {
    massorca_cli::run(std::env::args_os())
}
