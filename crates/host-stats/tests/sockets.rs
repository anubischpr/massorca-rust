//! The listening-socket reader against real sockets: this test process listens, and a child it starts listens to nothing
//! (decision 0081).

// The Linux reader is in the next change; until then this file runs on Windows only.
#![cfg(windows)]

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, TcpListener};
use std::process::Child;
use std::time::Duration;

use massorca_host_stats::{listening_tcp, processes};

/// Set in the child's environment: the `sleeper` test then sleeps instead of returning.
const SLEEPER_ENV: &str = "MASSORCA_HOST_STATS_SOCKETS_SLEEPER";

/// Not a test of its own: the child process the other tests read. Returns at once unless started as the child.
#[test]
fn sleeper() {
    if std::env::var_os(SLEEPER_ENV).is_some() {
        std::thread::sleep(Duration::from_secs(120));
    }
}

/// The test binary itself, running only `sleeper`, killed (by its pid) when dropped.
struct Sleeper(Child);

impl Sleeper {
    fn start() -> Self {
        let exe = std::env::current_exe().expect("the test binary");
        let child = massorca_process::command(exe)
            .args(["--exact", "sleeper", "--test-threads", "1"])
            .env(SLEEPER_ENV, "1")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("the sleeper starts");
        Self(child)
    }
}

impl Drop for Sleeper {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[cfg(windows)]
#[test]
fn a_listener_of_this_process_is_read_with_its_address_and_port() {
    let own = std::process::id();
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("a local listener");
    let port = listener.local_addr().expect("its address").port();

    let found = listening_tcp(&processes().processes, &[own]).expect("the socket table is read");
    assert!(
        found.sockets.iter().any(|socket| {
            socket.pid == own
                && socket.port == port
                && socket.address == IpAddr::V4(Ipv4Addr::LOCALHOST)
        }),
        "port {port} of pid {own} in {found:?}"
    );
}

#[cfg(windows)]
#[test]
fn an_ipv6_listener_is_read_when_the_host_has_ipv6() {
    let own = std::process::id();
    let Ok(listener) = TcpListener::bind((Ipv6Addr::LOCALHOST, 0)) else {
        return;
    };
    let port = listener.local_addr().expect("its address").port();

    let found = listening_tcp(&processes().processes, &[own]).expect("the socket table is read");
    assert!(
        found.sockets.iter().any(|socket| {
            socket.pid == own
                && socket.port == port
                && socket.address == IpAddr::V6(Ipv6Addr::LOCALHOST)
        }),
        "port {port} of pid {own} in {found:?}"
    );
}

#[cfg(windows)]
#[test]
fn a_process_that_listens_to_nothing_has_no_sockets() {
    let child = Sleeper::start();
    let found =
        listening_tcp(&processes().processes, &[child.0.id()]).expect("the socket table is read");
    assert!(found.sockets.is_empty(), "{found:?}");
    assert!(found.unreadable.is_empty(), "{found:?}");
}
