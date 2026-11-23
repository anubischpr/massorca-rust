//! `/proc/net/tcp` and `/proc/net/tcp6`: the TCP sockets of the network namespace, each with its state, bound address,
//! port and inode (decision 0081, D4). Parsing is pure, so it is tested on every platform.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

/// The state `LISTEN` as the kernel prints it in these tables.
const LISTEN: &str = "0A";

/// Which table a text is: its addresses are 8 hex digits (IPv4) or 32 (IPv6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Family {
    V4,
    V6,
}

/// One listening socket of a table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Listener {
    pub(crate) address: IpAddr,
    pub(crate) port: u16,
    /// The socket's inode, which the descriptor links of its owner name as `socket:[inode]`.
    pub(crate) inode: u64,
}

/// The LISTEN sockets in a table's text, in table order. The header line and any line that does not parse are skipped,
/// so one odd line does not hide the others.
pub(crate) fn listeners(text: &str, family: Family) -> Vec<Listener> {
    text.lines()
        .skip(1)
        .filter_map(|line| parse_line(line, family))
        .collect()
}

/// One row: `sl local rem st tx:rx tr:tm retrnsmt uid timeout inode ...`, with the local address and port before the
/// colon, in hexadecimal. Only LISTEN rows are kept.
fn parse_line(line: &str, family: Family) -> Option<Listener> {
    let mut fields = line.split_ascii_whitespace();
    let _slot = fields.next()?;
    let local = fields.next()?;
    let _remote = fields.next()?;
    if fields.next()? != LISTEN {
        return None;
    }
    // Skip tx:rx, tr:tm, retrnsmt, uid and timeout; the inode is the next field.
    let inode: u64 = fields.nth(5)?.parse().ok()?;
    let (address, port) = local.split_once(':')?;
    Some(Listener {
        address: address_of(address, family)?,
        port: u16::from_str_radix(port, 16).ok()?,
        inode,
    })
}

/// The address from the kernel's hexadecimal. It prints each 32-bit word of the address as it sits in memory, so the
/// word's native bytes are the address's own octets, whatever the byte order of the host.
fn address_of(hex: &str, family: Family) -> Option<IpAddr> {
    match family {
        Family::V4 => {
            if hex.len() != 8 {
                return None;
            }
            let word = u32::from_str_radix(hex, 16).ok()?;
            Some(IpAddr::V4(Ipv4Addr::from(word.to_ne_bytes())))
        }
        Family::V6 => {
            if hex.len() != 32 {
                return None;
            }
            let mut octets = [0u8; 16];
            for (index, chunk) in hex.as_bytes().chunks(8).enumerate() {
                let word = u32::from_str_radix(std::str::from_utf8(chunk).ok()?, 16).ok()?;
                octets[index * 4..index * 4 + 4].copy_from_slice(&word.to_ne_bytes());
            }
            Some(IpAddr::V6(Ipv6Addr::from(octets)))
        }
    }
}

/// The process that owns a socket several members hold: the one that started first, because a child forked from a
/// process starts after it. A holder whose start time is unknown comes last, and ties go to the lowest pid.
pub(crate) fn owner_of(holders: &[(Option<u64>, u32)]) -> Option<u32> {
    holders
        .iter()
        .min_by_key(|&&(start, pid)| (start.unwrap_or(u64::MAX), pid))
        .map(|&(_, pid)| pid)
}

#[cfg(test)]
mod owner_tests {
    use super::owner_of;

    #[test]
    fn the_process_that_started_first_owns_a_shared_socket_even_with_a_higher_pid() {
        // Pid 7 was forked from pid 9000: once pids wrap the number says little, and the start time says who came first.
        assert_eq!(owner_of(&[(Some(900), 7), (Some(100), 9000)]), Some(9000));
    }

    #[test]
    fn equal_or_unknown_starts_fall_back_to_the_lowest_pid() {
        assert_eq!(owner_of(&[(Some(5), 12), (Some(5), 3)]), Some(3));
        assert_eq!(owner_of(&[(None, 12), (None, 3)]), Some(3));
    }

    #[test]
    fn a_known_start_comes_before_an_unknown_one() {
        assert_eq!(owner_of(&[(None, 1), (Some(9), 50)]), Some(50));
    }

    #[test]
    fn no_holder_owns_nothing() {
        assert_eq!(owner_of(&[]), None);
    }
}

#[cfg(test)]
mod tests {
    use std::net::IpAddr;

    use super::{Family, listeners};

    const TABLE_V4: &str = "  sl  local_address rem_address   st tx_queue rx_queue tr tm->when retrnsmt   uid  timeout inode
   0: 0100007F:1F90 00000000:0000 0A 00000000:00000000 00:00000000 00000000  1000        0 12345 1 0000000000000000 100 0 0 10 0
   1: 0100007F:0050 0100007F:9C40 01 00000000:00000000 00:00000000 00000000  1000        0 999 1 0000000000000000 100 0 0 10 0
   2: 00000000:01BB 00000000:0000 0A 00000000:00000000 00:00000000 00000000     0        0 4242 1 0000000000000000 100 0 0 10 0
   3: garbled line
";

    const TABLE_V6: &str = "  sl  local_address                         remote_address                        st tx_queue rx_queue tr tm->when retrnsmt   uid  timeout inode
   0: 00000000000000000000000001000000:1F90 00000000000000000000000000000000:0000 0A 00000000:00000000 00:00000000 00000000  1000        0 12345 1 0000000000000000 100 0 0 10 0
   1: 00000000000000000000000000000000:0016 00000000000000000000000000000000:0000 0A 00000000:00000000 00:00000000 00000000     0        0 777 1 0000000000000000 100 0 0 10 0
";

    #[test]
    fn listen_rows_give_their_address_port_and_inode() {
        let found = listeners(TABLE_V4, Family::V4);
        assert_eq!(found.len(), 2, "{found:?}");
        assert_eq!(found[0].address, IpAddr::from([127, 0, 0, 1]));
        assert_eq!((found[0].port, found[0].inode), (8080, 12345));
        assert_eq!(found[1].address, IpAddr::from([0, 0, 0, 0]));
        assert_eq!((found[1].port, found[1].inode), (443, 4242));
    }

    #[test]
    fn ipv6_rows_give_their_full_address() {
        let found = listeners(TABLE_V6, Family::V6);
        assert_eq!(found.len(), 2, "{found:?}");
        assert_eq!(found[0].address, IpAddr::from([0u16, 0, 0, 0, 0, 0, 0, 1]));
        assert_eq!((found[0].port, found[0].inode), (8080, 12345));
        assert_eq!(found[1].address, IpAddr::from([0u16; 8]));
        assert_eq!((found[1].port, found[1].inode), (22, 777));
    }
}
