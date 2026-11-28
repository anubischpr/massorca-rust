//! Dialing a paired host through OpenSSH's stdio forward, `ssh -W` (decision 0133). The ssh child only carries bytes;
//! the TLS handshake over its stdin and stdout checks the pinned server key as a direct connection does. The child is
//! killed when the connection ends.

use std::io;
use std::path::Path;
use std::pin::Pin as StdPin;
use std::process::{Child, Stdio};
use std::task::{Context, Poll};
use std::time::Duration;

use massorca_core_remote::ssh::{find_system_ssh, stdio_forward_arguments};
use massorca_net::{ClientWsOver, NetError, Pin, TlsIdentity};
use tokio::io::{AsyncRead, AsyncReadExt as _, AsyncWrite, ReadBuf};
use tokio::process::{ChildStderr, ChildStdin, ChildStdout};
use tokio::task::JoinHandle;

use crate::host_storage::SshRoute;

/// ssh's own `ConnectTimeout` is 15 s per hop, so a route through one jump host still fits.
const DIAL_TIMEOUT: Duration = Duration::from_secs(40);
/// How long a failed dial waits for what ssh printed before it exited.
const STDERR_WAIT: Duration = Duration::from_secs(2);
/// The end of ssh's error output kept for the message.
const STDERR_KEPT: usize = 600;

/// The client end of a connection dialed through ssh.
pub(crate) type SshWs = ClientWsOver<SshTunnel>;

/// Why a dial through ssh failed.
#[derive(Debug)]
pub(crate) enum DialFailure {
    /// The server presented another key than the pinned one. Final, as on a direct connection.
    Mismatch(NetError),
    /// The route did not reach the server; the sentence names the route and what ssh said.
    Unreachable(String),
}

/// Dials the host through `route` and runs TLS with `pin` and the WebSocket upgrade over it.
pub(crate) async fn dial(
    route: &SshRoute,
    addresses: &[String],
    pin: Pin,
    identity: &TlsIdentity,
) -> Result<SshWs, DialFailure> {
    let unreachable = |reason: &str| DialFailure::Unreachable(failure_sentence(route, reason));
    let forward = route
        .forward
        .as_deref()
        .or_else(|| addresses.first().map(String::as_str))
        .ok_or_else(|| unreachable("the host has no address for ssh to forward to"))?;
    let args = stdio_forward_arguments(&route.ssh_target, &route.ssh_args, forward)
        .map_err(|error| unreachable(&error.to_string()))?;
    let program = find_system_ssh().map_err(|error| unreachable(&error.to_string()))?;
    let (tunnel, stderr) = SshTunnel::spawn(&program, &args)
        .map_err(|error| unreachable(&format!("ssh did not start: {error}")))?;
    let attempt = tokio::time::timeout(
        DIAL_TIMEOUT,
        massorca_net::connect_stream(tunnel, forward, pin, identity),
    );
    match attempt.await {
        Ok(Ok(ws)) => Ok(ws),
        Ok(Err(mismatch @ NetError::ServerPinMismatch { .. })) => {
            Err(DialFailure::Mismatch(mismatch))
        }
        // The tunnel was dropped with the failed handshake, so ssh is stopped and its error output ends.
        Ok(Err(error)) => Err(unreachable(&stderr.reason_or(error.to_string()).await)),
        Err(_) => Err(unreachable(&format!(
            "ssh did not reach the server within {} s",
            DIAL_TIMEOUT.as_secs()
        ))),
    }
}

/// The message of a route that failed: names the route, never a key or a password.
pub(crate) fn failure_sentence(route: &SshRoute, reason: &str) -> String {
    format!("through the SSH route `{}`: {reason}", route.describe())
}

/// The stdin and stdout of a running `ssh -W` as one byte stream. Dropping it kills ssh and waits for it.
#[derive(Debug)]
pub(crate) struct SshTunnel {
    child: Child,
    /// Kill-on-close, so what ssh starts for `-J` or a ProxyCommand ends with it.
    #[cfg(windows)]
    _job: Option<win32job::Job>,
    stdin: ChildStdin,
    stdout: ChildStdout,
}

impl SshTunnel {
    fn spawn(program: &Path, args: &[String]) -> io::Result<(Self, StderrTail)> {
        let mut command = massorca_process::command(program);
        command
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        let (mut child, job) = massorca_core_remote::windows_job::spawn_in_job(&mut command, 0)?;
        #[cfg(not(windows))]
        let mut child = command.spawn()?;
        let pipes = (child.stdin.take(), child.stdout.take(), child.stderr.take());
        let (Some(stdin), Some(stdout), Some(stderr)) = pipes else {
            stop(&mut child);
            return Err(io::Error::other("ssh started without its pipes"));
        };
        let opened = (|| {
            Ok::<_, io::Error>((
                ChildStdin::from_std(stdin)?,
                ChildStdout::from_std(stdout)?,
                ChildStderr::from_std(stderr)?,
            ))
        })();
        let (stdin, stdout, stderr) = match opened {
            Ok(pipes) => pipes,
            Err(error) => {
                stop(&mut child);
                return Err(error);
            }
        };
        let tunnel = Self {
            child,
            #[cfg(windows)]
            _job: job,
            stdin,
            stdout,
        };
        Ok((tunnel, StderrTail::read(stderr)))
    }
}

impl Drop for SshTunnel {
    fn drop(&mut self) {
        stop(&mut self.child);
    }
}

/// Kills the child and reaps it; one that already exited is only reaped.
fn stop(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

impl AsyncRead for SshTunnel {
    fn poll_read(
        mut self: StdPin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        StdPin::new(&mut self.stdout).poll_read(cx, buf)
    }
}

impl AsyncWrite for SshTunnel {
    fn poll_write(
        mut self: StdPin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        StdPin::new(&mut self.stdin).poll_write(cx, buf)
    }

    fn poll_flush(mut self: StdPin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        StdPin::new(&mut self.stdin).poll_flush(cx)
    }

    fn poll_shutdown(mut self: StdPin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        StdPin::new(&mut self.stdin).poll_shutdown(cx)
    }
}

/// The end of what ssh prints on stderr, read until it closes.
#[derive(Debug)]
struct StderrTail(JoinHandle<Vec<u8>>);

impl StderrTail {
    fn read(mut stderr: ChildStderr) -> Self {
        Self(tokio::spawn(async move {
            let mut kept = Vec::new();
            let mut chunk = [0_u8; 1024];
            while let Ok(read @ 1..) = stderr.read(&mut chunk).await {
                kept.extend_from_slice(&chunk[..read]);
                let excess = kept.len().saturating_sub(STDERR_KEPT);
                kept.drain(..excess);
            }
            kept
        }))
    }

    /// What ssh said, or `fallback` when it said nothing in time.
    async fn reason_or(self, fallback: String) -> String {
        let said = match tokio::time::timeout(STDERR_WAIT, self.0).await {
            Ok(Ok(bytes)) => printable(&bytes),
            _ => String::new(),
        };
        if said.is_empty() { fallback } else { said }
    }
}

/// ssh's output as one line of printable text, for a message. A remote banner or a ProxyCommand may print terminal
/// control sequences, so every control character becomes a space; what remains is plain text.
fn printable(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes)
        .chars()
        .map(|c| {
            if c.is_control() || is_bidi_control(c) {
                ' '
            } else {
                c
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// The bidirectional embeddings, overrides and isolates, which reorder the text a terminal shows.
fn is_bidi_control(c: char) -> bool {
    matches!(c, '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_ssh_printed_is_quoted_without_control_characters() {
        let quoted =
            printable(b"\x1b[31mPermission denied\x07 (publickey).\r\n\x1b]0;title\x07\x08 end\t");
        assert!(!quoted.chars().any(char::is_control), "{quoted:?}");
        assert!(quoted.contains("Permission denied"), "{quoted:?}");
        assert!(quoted.ends_with("end"), "{quoted:?}");
        assert_eq!(printable("a\u{202E}b".as_bytes()), "a b");
    }

    fn route() -> SshRoute {
        SshRoute {
            ssh_target: "deploy@vps.example.ts.net".to_owned(),
            ssh_args: vec!["-J".to_owned(), "bastion".to_owned()],
            forward: None,
        }
    }

    #[test]
    fn a_failure_names_the_route_as_typed() {
        assert_eq!(
            failure_sentence(&route(), "Permission denied (publickey)."),
            "through the SSH route `ssh -J bastion deploy@vps.example.ts.net`: Permission denied (publickey)."
        );
    }

    #[tokio::test]
    async fn an_ssh_program_that_cannot_start_is_an_io_error() {
        let missing = std::env::temp_dir().join("massorca-no-such-ssh-for-this-test");
        assert!(SshTunnel::spawn(&missing, &["vps".to_owned()]).is_err());
    }

    #[tokio::test]
    async fn a_route_with_no_forward_and_no_address_is_unreachable_before_ssh_runs() {
        let identity = TlsIdentity::generate("laptop").expect("a device identity");
        let pin: Pin = "sha256:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"
            .parse()
            .expect("a pin");
        match dial(&route(), &[], pin, &identity).await {
            Err(DialFailure::Unreachable(sentence)) => {
                assert!(
                    sentence.contains("ssh -J bastion deploy@vps.example.ts.net"),
                    "{sentence}"
                );
            }
            Err(DialFailure::Mismatch(error)) => panic!("expected unreachable, got {error}"),
            Ok(_) => panic!("expected unreachable, got a connection"),
        }
    }
}
