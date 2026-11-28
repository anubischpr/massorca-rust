//! `massorca-fake-ssh`: a stand-in for OpenSSH's stdio forward, `ssh -W host:port <target>`, for the tests of dialing a
//! host through SSH (decision 0133). It is an example, not a shipped binary, and it never contacts the target.
//!
//! - It appends one JSON line `{"pid":..,"args":[..]}` to the file `FAKE_SSH_LOG` names.
//! - A target starting with `denied@` prints an authentication failure and exits 255, as ssh does.
//! - Otherwise it connects to the `-W` address on this computer and pipes its stdin and stdout through that connection.
//! - It then waits up to a minute instead of exiting, so a test sees whether the client killed it.

use std::env;
use std::fs::OpenOptions;
use std::io::{self, Read, Write};
use std::net::{Shutdown, TcpStream};
use std::process::ExitCode;
use std::thread;
use std::time::Duration;

/// OpenSSH exits with 255 when its own connection or authentication fails.
const SSH_OWN_FAILURE: u8 = 255;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    log(&args);
    let forward = args
        .iter()
        .position(|arg| arg == "-W")
        .and_then(|at| args.get(at + 1));
    let Some(forward) = forward else {
        say("fake ssh: no -W forward");
        return ExitCode::from(SSH_OWN_FAILURE);
    };
    if args
        .last()
        .is_some_and(|target| target.starts_with("denied@"))
    {
        say("denied@bastion: Permission denied (publickey).");
        return ExitCode::from(SSH_OWN_FAILURE);
    }
    let Ok(stream) = TcpStream::connect(forward) else {
        say("channel 0: open failed: connect failed: Connection refused");
        return ExitCode::from(SSH_OWN_FAILURE);
    };
    let Ok(mut upstream) = stream.try_clone() else {
        return ExitCode::from(SSH_OWN_FAILURE);
    };
    let down = thread::spawn(move || pipe_to_stdout(stream));
    let _ = io::copy(&mut io::stdin().lock(), &mut upstream);
    let _ = upstream.shutdown(Shutdown::Write);
    let _ = down.join();
    thread::sleep(Duration::from_secs(60));
    ExitCode::SUCCESS
}

/// Copies the connection to stdout, flushing each chunk: std's stdout buffers until a newline.
fn pipe_to_stdout(mut stream: TcpStream) {
    let mut stdout = io::stdout().lock();
    let mut chunk = [0_u8; 16 * 1024];
    while let Ok(read @ 1..) = stream.read(&mut chunk) {
        if stdout
            .write_all(&chunk[..read])
            .and_then(|()| stdout.flush())
            .is_err()
        {
            return;
        }
    }
}

fn say(text: &str) {
    let _ = writeln!(io::stderr().lock(), "{text}");
}

fn log(args: &[String]) {
    let Some(path) = env::var_os("FAKE_SSH_LOG") else {
        return;
    };
    let line = serde_json::json!({ "pid": std::process::id(), "args": args });
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "{line}");
    }
}
