//! The name this computer has on the servers it pairs with (decision 0041).

/// The device name when this computer's host name cannot be read.
const FALLBACK_DEVICE_NAME: &str = "massorca";

/// This computer's host name, for the name it has on a server: COMPUTERNAME, HOSTNAME, then /etc/hostname.
pub fn default_device_name() -> String {
    let candidates = ["COMPUTERNAME", "HOSTNAME"]
        .into_iter()
        .filter_map(|key| std::env::var(key).ok())
        .chain(std::fs::read_to_string("/etc/hostname").ok());
    candidates
        .map(|name| name.trim().to_owned())
        .find(|name| !name.is_empty())
        .unwrap_or_else(|| FALLBACK_DEVICE_NAME.to_owned())
}

#[cfg(test)]
mod tests {
    use super::default_device_name;

    #[test]
    fn a_device_name_is_never_empty() {
        assert!(!default_device_name().trim().is_empty());
    }
}
