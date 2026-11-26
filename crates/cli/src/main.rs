//! The `massorca` binary: a thin shell over the `massorca_cli` library.

use std::process::ExitCode;

/// The stack the command line runs on. Building the whole clap command tree (as `agent-context` does) takes more than
/// the 1 MiB a Windows main thread gets in a debug build, so the work runs on a thread with room to spare.
const STACK_BYTES: usize = 8 * 1024 * 1024;

fn main() -> ExitCode {
    let args: Vec<_> = std::env::args_os().collect();
    match std::thread::Builder::new()
        .name("massorca".to_owned())
        .stack_size(STACK_BYTES)
        .spawn(move || massorca_cli::run(args))
    {
        Ok(handle) => handle.join().unwrap_or(ExitCode::FAILURE),
        // No thread could be made: run here, as before.
        Err(_) => massorca_cli::run(std::env::args_os()),
    }
}
