//! The Add host dialog's SSH path (D-HO-02, decision 0075): what the fields make, what the test found, and
//! which Linux program goes to the host. A host is added only for the target its test checked. Plain data and
//! rules, free of GPUI.

use std::path::{Path, PathBuf};

/// The ssh target of an SSH add: `[user@]address`, and the ssh options the port needs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SshTarget {
    pub target: String,
    pub ssh_args: Vec<String>,
}

/// What the test of an SSH host found.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum Check {
    #[default]
    Idle,
    /// The host is being checked.
    Checking,
    /// The host answered and runs Linux on this machine.
    Passed { arch: String },
    /// The host could not be checked: the sentence, and the log of the check.
    Failed { sentence: String, log: Vec<String> },
}

impl SshTarget {
    /// The target the fields make, or the sentence that says what to fix.
    pub fn from_fields(address: &str, port: &str, user: &str) -> Result<Self, String> {
        let address = address.trim();
        let port = port.trim();
        let user = user.trim();
        if address.is_empty() {
            return Err("Type the address of the host.".to_owned());
        }
        if address.contains('@') {
            return Err("Type the user name in User, not in Address.".to_owned());
        }
        let address_ok = address
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || ".-:[]_".contains(c));
        if address.starts_with('-') || !address_ok {
            return Err("The address has characters an SSH address does not have.".to_owned());
        }
        let user_ok = user
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "._-".contains(c));
        if !user.is_empty() && (user.starts_with('-') || !user_ok) {
            return Err("The user name has characters a user name does not have.".to_owned());
        }
        let ssh_args = if port.is_empty() {
            Vec::new()
        } else {
            match port.parse::<u16>() {
                Ok(number) if number >= 1 => vec!["-p".to_owned(), number.to_string()],
                _ => return Err("The port is a number from 1 to 65535.".to_owned()),
            }
        };
        let target = if user.is_empty() {
            address.to_owned()
        } else {
            format!("{user}@{address}")
        };
        Ok(Self { target, ssh_args })
    }
}

/// The program to send to a host: the one the developer chose, else the one packaged for its machine.
pub fn program_for(chosen: Option<PathBuf>, packaged: Option<PathBuf>) -> Option<PathBuf> {
    chosen.or(packaged)
}

/// Whether "Add host" can be pressed: the host passed the test of the target typed now, a program is known,
/// and no add is under way.
pub fn can_add(
    check: &Check,
    checked: Option<&SshTarget>,
    typed: Option<&SshTarget>,
    program: Option<&Path>,
    adding: bool,
) -> bool {
    matches!(check, Check::Passed { .. })
        && checked.is_some()
        && checked == typed
        && program.is_some()
        && !adding
}

/// The line under a passed test: the host answered, and it runs Linux on `arch`.
pub fn passed_sentence(arch: &str) -> String {
    format!("Connected. The host runs Linux on {arch}.")
}

/// The line for a machine the app has no program for: plain, and it names the override.
pub fn no_program_sentence(arch: &str) -> String {
    format!(
        "This app has no MassOrca program for Linux on {arch}. Choose a program file under Advanced options."
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target(address: &str) -> SshTarget {
        SshTarget {
            target: address.to_owned(),
            ssh_args: Vec::new(),
        }
    }

    #[test]
    fn a_target_has_the_user_in_front_and_the_port_as_an_ssh_option() {
        assert_eq!(
            SshTarget::from_fields("100.64.0.10", "2222", "deploy"),
            Ok(SshTarget {
                target: "deploy@100.64.0.10".to_owned(),
                ssh_args: vec!["-p".to_owned(), "2222".to_owned()],
            })
        );
        assert_eq!(
            SshTarget::from_fields("vps.example.ts.net", "", ""),
            Ok(SshTarget {
                target: "vps.example.ts.net".to_owned(),
                ssh_args: Vec::new(),
            })
        );
    }

    #[test]
    fn the_address_is_required_and_cannot_carry_options_or_a_user() {
        assert_eq!(
            SshTarget::from_fields("  ", "", ""),
            Err("Type the address of the host.".to_owned())
        );
        assert!(SshTarget::from_fields("-oProxyCommand=run", "", "").is_err());
        let user_in_address = SshTarget::from_fields("deploy@vps", "", "").expect_err("refused");
        assert!(user_in_address.contains("User"), "{user_in_address}");
    }

    #[test]
    fn the_port_is_a_number_from_1_to_65535() {
        assert!(SshTarget::from_fields("vps", "0", "").is_err());
        assert!(SshTarget::from_fields("vps", "70000", "").is_err());
        assert!(SshTarget::from_fields("vps", "abc", "").is_err());
        assert!(SshTarget::from_fields("vps", "65535", "").is_ok());
    }

    #[test]
    fn a_host_is_added_only_for_the_target_its_test_checked() {
        let passed = Check::Passed {
            arch: "x86_64".to_owned(),
        };
        let program = Path::new("program");
        let checked = target("vps");
        assert!(can_add(
            &passed,
            Some(&checked),
            Some(&checked),
            Some(program),
            false
        ));
        assert!(!can_add(
            &passed,
            Some(&checked),
            Some(&target("nas")),
            Some(program),
            false
        ));
        assert!(!can_add(
            &passed,
            Some(&checked),
            Some(&checked),
            None,
            false
        ));
        assert!(!can_add(
            &passed,
            Some(&checked),
            Some(&checked),
            Some(program),
            true
        ));
        assert!(!can_add(
            &Check::Idle,
            None,
            Some(&checked),
            Some(program),
            false
        ));
        let failed = Check::Failed {
            sentence: "No.".to_owned(),
            log: Vec::new(),
        };
        assert!(!can_add(
            &failed,
            Some(&checked),
            Some(&checked),
            Some(program),
            false
        ));
    }

    #[test]
    fn the_developers_chosen_program_comes_before_the_packaged_one() {
        let chosen = PathBuf::from("chosen");
        let packaged = PathBuf::from("packaged");
        assert_eq!(
            program_for(Some(chosen.clone()), Some(packaged.clone())),
            Some(chosen)
        );
        assert_eq!(program_for(None, Some(packaged.clone())), Some(packaged));
        assert_eq!(program_for(None, None), None);
    }

    #[test]
    fn a_machine_without_a_program_is_said_plainly_with_the_override_named() {
        let line = no_program_sentence("x86_64");
        assert!(line.contains("x86_64"), "{line}");
        assert!(
            line.ends_with("Choose a program file under Advanced options."),
            "{line}"
        );
        assert_eq!(
            passed_sentence("aarch64"),
            "Connected. The host runs Linux on aarch64."
        );
    }
}
