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
    /// The target the fields make, or the sentence that says what to fix. A chosen key is the `-i` option.
    pub fn from_fields(
        address: &str,
        port: &str,
        user: &str,
        identity: Option<&Path>,
    ) -> Result<Self, String> {
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
        let mut ssh_args = if port.is_empty() {
            Vec::new()
        } else {
            match port.parse::<u16>() {
                Ok(number) if number >= 1 => vec!["-p".to_owned(), number.to_string()],
                _ => return Err("The port is a number from 1 to 65535.".to_owned()),
            }
        };
        if let Some(identity) = identity {
            ssh_args.extend(["-i".to_owned(), identity.to_string_lossy().into_owned()]);
        }
        let target = if user.is_empty() {
            address.to_owned()
        } else {
            format!("{user}@{address}")
        };
        Ok(Self { target, ssh_args })
    }
}

/// The label of the key menu's first row: the ssh config's own choice, which passes no key.
pub const SSH_CONFIG_LABEL: &str = "Default (ssh config)";

/// The key an SSH add passes to ssh with `-i`: a key file in the user's `.ssh` folder, or the ssh config's choice.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum KeyChoice {
    #[default]
    SshConfig,
    File(PathBuf),
}

impl KeyChoice {
    /// The key file to pass with `-i`, when the choice names one.
    pub fn identity(&self) -> Option<&Path> {
        match self {
            Self::SshConfig => None,
            Self::File(path) => Some(path),
        }
    }
}

/// The key files among the names of an SSH folder, by name only: an `id_*` name without an extension, or a name
/// with a `.pub` partner. Public keys, known hosts and config files are not keys.
pub fn key_names(entries: &[String]) -> Vec<String> {
    let mut keys: Vec<String> = entries
        .iter()
        .filter(|name| {
            let has_public_half = entries.contains(&format!("{name}.pub"));
            !name.ends_with(".pub")
                && ((name.starts_with("id_") && !name.contains('.')) || has_public_half)
        })
        .cloned()
        .collect();
    keys.sort();
    keys
}

/// The key files in an SSH folder, sorted. A folder that cannot be read has none, and no folder is a key.
pub fn list_key_files(ssh_dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(ssh_dir) else {
        return Vec::new();
    };
    let files: Vec<String> = entries
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_file())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    key_names(&files)
        .into_iter()
        .map(|name| ssh_dir.join(name))
        .collect()
}

/// The key menu's rows: the ssh config's choice, then each key file as `~/.ssh/<name>`.
pub fn key_labels(keys: &[PathBuf]) -> Vec<String> {
    std::iter::once(SSH_CONFIG_LABEL.to_owned())
        .chain(keys.iter().map(|key| format!("~/.ssh/{}", file_name(key))))
        .collect()
}

/// The choice of menu row `index`: row 0 is the ssh config's choice, and row N is the Nth key file.
pub fn key_choice_at(keys: &[PathBuf], index: Option<usize>) -> KeyChoice {
    index
        .and_then(|index| index.checked_sub(1))
        .and_then(|at| keys.get(at))
        .map_or(KeyChoice::SshConfig, |key| KeyChoice::File(key.clone()))
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
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
            SshTarget::from_fields("100.64.0.10", "2222", "deploy", None),
            Ok(SshTarget {
                target: "deploy@100.64.0.10".to_owned(),
                ssh_args: vec!["-p".to_owned(), "2222".to_owned()],
            })
        );
        assert_eq!(
            SshTarget::from_fields("vps.example.ts.net", "", "", None),
            Ok(SshTarget {
                target: "vps.example.ts.net".to_owned(),
                ssh_args: Vec::new(),
            })
        );
    }

    #[test]
    fn the_address_is_required_and_cannot_carry_options_or_a_user() {
        assert_eq!(
            SshTarget::from_fields("  ", "", "", None),
            Err("Type the address of the host.".to_owned())
        );
        assert!(SshTarget::from_fields("-oProxyCommand=run", "", "", None).is_err());
        let user_in_address =
            SshTarget::from_fields("deploy@vps", "", "", None).expect_err("refused");
        assert!(user_in_address.contains("User"), "{user_in_address}");
    }

    #[test]
    fn the_port_is_a_number_from_1_to_65535() {
        assert!(SshTarget::from_fields("vps", "0", "", None).is_err());
        assert!(SshTarget::from_fields("vps", "70000", "", None).is_err());
        assert!(SshTarget::from_fields("vps", "abc", "", None).is_err());
        assert!(SshTarget::from_fields("vps", "65535", "", None).is_ok());
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

    #[test]
    fn a_chosen_key_is_passed_to_ssh_as_the_identity_option_after_the_port() {
        let key = Path::new("C:/Users/me/.ssh/work_key");
        assert_eq!(
            SshTarget::from_fields("100.64.0.10", "2222", "deploy", Some(key)),
            Ok(SshTarget {
                target: "deploy@100.64.0.10".to_owned(),
                ssh_args: vec![
                    "-p".to_owned(),
                    "2222".to_owned(),
                    "-i".to_owned(),
                    "C:/Users/me/.ssh/work_key".to_owned(),
                ],
            })
        );
    }

    #[test]
    fn key_names_lists_the_private_keys_by_name_and_skips_the_rest() {
        let entries: Vec<String> = [
            "authorized_keys",
            "config",
            "id_ed25519",
            "id_ed25519.pub",
            "id_old.bak",
            "known_hosts",
            "known_hosts.old",
            "notes.txt",
            "old_key",
            "work_key",
            "work_key.pub",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect();
        assert_eq!(
            key_names(&entries),
            vec!["id_ed25519".to_owned(), "work_key".to_owned()]
        );
    }

    #[test]
    fn list_key_files_reads_the_names_in_the_folder_and_no_folder_is_a_key() {
        let dir = tempfile::tempdir().expect("a temporary folder");
        for name in [
            "id_ed25519",
            "id_ed25519.pub",
            "work_key",
            "work_key.pub",
            "config",
        ] {
            std::fs::write(dir.path().join(name), b"").expect("a file");
        }
        std::fs::create_dir(dir.path().join("id_folder")).expect("a folder");
        assert_eq!(
            list_key_files(dir.path()),
            vec![dir.path().join("id_ed25519"), dir.path().join("work_key")]
        );
        assert!(
            list_key_files(&dir.path().join("missing")).is_empty(),
            "a folder that cannot be read has no keys"
        );
    }

    #[test]
    fn the_menu_offers_the_ssh_config_first_then_each_key_by_its_name() {
        let keys = vec![
            PathBuf::from("C:/Users/me/.ssh/id_ed25519"),
            PathBuf::from("C:/Users/me/.ssh/work_key"),
        ];
        assert_eq!(
            key_labels(&keys),
            vec![
                "Default (ssh config)",
                "~/.ssh/id_ed25519",
                "~/.ssh/work_key"
            ]
        );
        assert_eq!(key_labels(&[]), vec!["Default (ssh config)"]);
    }

    #[test]
    fn a_menu_row_is_the_ssh_config_or_the_key_it_names() {
        let keys = vec![
            PathBuf::from("C:/Users/me/.ssh/id_ed25519"),
            PathBuf::from("C:/Users/me/.ssh/work_key"),
        ];
        assert_eq!(key_choice_at(&keys, Some(0)), KeyChoice::SshConfig);
        assert_eq!(
            key_choice_at(&keys, Some(2)),
            KeyChoice::File(PathBuf::from("C:/Users/me/.ssh/work_key"))
        );
        assert_eq!(key_choice_at(&keys, None), KeyChoice::SshConfig);
        assert_eq!(
            key_choice_at(&keys, Some(9)),
            KeyChoice::SshConfig,
            "a row past the keys is no key"
        );
        assert_eq!(KeyChoice::SshConfig.identity(), None);
        assert_eq!(
            key_choice_at(&keys, Some(1)).identity(),
            Some(Path::new("C:/Users/me/.ssh/id_ed25519"))
        );
    }
}
