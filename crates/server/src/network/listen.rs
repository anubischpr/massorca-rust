//! What `--listen` accepts (decision 0045): `tailscale[:port]`, `localhost:<port>` or `<ip>:<port>`.

use std::fmt;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::str::FromStr;

use massorca_net::DEFAULT_PORT;

/// Where one network listener binds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ListenSpec {
    /// This node's Tailscale IPv4 address, read when the listener starts.
    Tailscale { port: u16 },
    /// A fixed address.
    Address(SocketAddr),
}

/// The network part of a service: where it listens. Empty means local only.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NetworkConfig {
    pub listen: Vec<ListenSpec>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ListenSpecError {
    #[error("{0:?} is not a listen address; use tailscale[:PORT], localhost:PORT or IP:PORT")]
    Invalid(String),
    #[error(
        "{0} listens on every interface, including public ones; pass --allow-all-interfaces to mean it"
    )]
    AllInterfaces(String),
}

impl ListenSpec {
    /// Parses one `--listen` value. An unspecified address (`0.0.0.0`, `[::]`) needs `allow_all_interfaces`.
    pub fn parse(value: &str, allow_all_interfaces: bool) -> Result<Self, ListenSpecError> {
        let invalid = || ListenSpecError::Invalid(value.to_owned());
        if value == "tailscale" {
            return Ok(Self::Tailscale { port: DEFAULT_PORT });
        }
        if let Some(port) = value.strip_prefix("tailscale:") {
            return Ok(Self::Tailscale {
                port: parse_port(port).ok_or_else(invalid)?,
            });
        }
        if let Some(port) = value.strip_prefix("localhost:") {
            let port = parse_port(port).ok_or_else(invalid)?;
            return Ok(Self::Address(SocketAddr::new(
                IpAddr::V4(Ipv4Addr::LOCALHOST),
                port,
            )));
        }
        let address = SocketAddr::from_str(value).map_err(|_| invalid())?;
        if address.port() == 0 {
            return Err(invalid());
        }
        if address.ip().is_unspecified() && !allow_all_interfaces {
            return Err(ListenSpecError::AllInterfaces(value.to_owned()));
        }
        Ok(Self::Address(address))
    }
}

impl fmt::Display for ListenSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Tailscale { port } => write!(f, "tailscale:{port}"),
            Self::Address(address) => write!(f, "{address}"),
        }
    }
}

fn parse_port(text: &str) -> Option<u16> {
    text.parse::<u16>().ok().filter(|port| *port != 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_forms_of_decision_0045_parse() {
        assert_eq!(
            ListenSpec::parse("tailscale", false),
            Ok(ListenSpec::Tailscale { port: 7347 })
        );
        assert_eq!(
            ListenSpec::parse("tailscale:9000", false),
            Ok(ListenSpec::Tailscale { port: 9000 })
        );
        assert_eq!(
            ListenSpec::parse("localhost:9000", false)
                .unwrap()
                .to_string(),
            "127.0.0.1:9000"
        );
        assert_eq!(
            ListenSpec::parse("[::1]:9000", false).unwrap().to_string(),
            "[::1]:9000"
        );
    }

    #[test]
    fn every_interface_needs_a_flag_and_bad_values_are_refused() {
        assert!(matches!(
            ListenSpec::parse("0.0.0.0:9000", false),
            Err(ListenSpecError::AllInterfaces(_))
        ));
        assert!(ListenSpec::parse("0.0.0.0:9000", true).is_ok());
        assert!(matches!(
            ListenSpec::parse("[::]:9000", false),
            Err(ListenSpecError::AllInterfaces(_))
        ));
        for bad in [
            "",
            "tailscale:",
            "tailscale:0",
            "localhost",
            "1.2.3.4",
            "host:80",
            "1.2.3.4:0",
        ] {
            assert!(ListenSpec::parse(bad, false).is_err(), "{bad}");
        }
    }
}
