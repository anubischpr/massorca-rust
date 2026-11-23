//! Listening TCP sockets and the processes that hold them (decision 0081, D2 and D4). Native calls only; each platform's
//! reader is in its own module, and `unsafe` stays in the Windows and macOS ones.

use std::collections::BTreeSet;
use std::net::IpAddr;

use crate::{ProcessInfo, sys};

/// One TCP socket in the LISTEN state, and the process that holds it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ListeningSocket {
    pub pid: u32,
    /// The address the socket is bound to: a wildcard (`0.0.0.0`, `::`) or one interface's address.
    pub address: IpAddr,
    pub port: u16,
}

/// The listening sockets of the processes asked about, and the processes whose sockets could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listening {
    /// The sockets of the asked processes, one entry per socket: a process listening on IPv4 and on IPv6 has two.
    pub sockets: Vec<ListeningSocket>,
    /// The asked processes whose sockets could not be read: they exited, or access was denied. Not in `sockets`.
    pub unreadable: BTreeSet<u32>,
}

/// The TCP sockets in the LISTEN state that the processes `pids` hold, read now. `table` is the process table the pids
/// came from; a platform that reads process by process checks that each one is still the process listed. `None` when
/// the socket table cannot be read on this platform, so nothing is known about any port.
#[must_use]
pub fn listening_tcp(table: &[ProcessInfo], pids: &[u32]) -> Option<Listening> {
    let asked: BTreeSet<u32> = pids.iter().copied().collect();
    sys::listening_sockets(table, &asked)
}
