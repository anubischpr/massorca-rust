//! Bootstrap and uninstall against a scripted remote shell: order, payloads, redaction and errors (decision 0044).

use std::cell::RefCell;
use std::fs;
use std::path::PathBuf;

use massorca_core_remote::bootstrap::{
    BootstrapError, BootstrapPlan, BootstrapReport, DEFAULT_LISTEN, DEFAULT_OFFER_TTL_SECS,
    DEFAULT_REMOTE_DIR, bootstrap, uninstall, validate_listen,
};
use massorca_core_remote::ssh::{RemoteOutput, RemoteShell, SshError};
use tempfile::TempDir;

const EM_X86_64: u16 = 0x3E;
const EM_AARCH64: u16 = 0xB7;
const OFFER: &str = "mop1-TEST-OFFER-0001";

/// Records every command with its stdin and answers from a script.
struct FakeShell {
    calls: RefCell<Vec<(String, Option<Vec<u8>>)>>,
    answer: Box<dyn Fn(&str) -> RemoteOutput>,
}

impl FakeShell {
    fn new(answer: impl Fn(&str) -> RemoteOutput + 'static) -> Self {
        Self {
            calls: RefCell::new(Vec::new()),
            answer: Box::new(answer),
        }
    }

    fn commands(&self) -> Vec<String> {
        self.calls
            .borrow()
            .iter()
            .map(|(command, _)| command.clone())
            .collect()
    }

    fn stdin(&self, index: usize) -> Option<Vec<u8>> {
        self.calls.borrow()[index].1.clone()
    }
}

impl RemoteShell for FakeShell {
    fn run(&self, command: &str, stdin: Option<&[u8]>) -> Result<RemoteOutput, SshError> {
        self.calls
            .borrow_mut()
            .push((command.to_owned(), stdin.map(<[u8]>::to_vec)));
        Ok((self.answer)(command))
    }
}

/// A host whose transport fails before any command runs.
struct UnreachableHost;

impl RemoteShell for UnreachableHost {
    fn run(&self, _command: &str, _stdin: Option<&[u8]>) -> Result<RemoteOutput, SshError> {
        Err(SshError::Failed {
            status: 255,
            stderr: "Host key verification failed.".to_owned(),
        })
    }
}

fn ok(stdout: &str) -> RemoteOutput {
    RemoteOutput {
        status: 0,
        stdout: stdout.as_bytes().to_vec(),
        stderr: Vec::new(),
    }
}

fn fail(status: i32, stderr: &str) -> RemoteOutput {
    RemoteOutput {
        status,
        stdout: Vec::new(),
        stderr: stderr.as_bytes().to_vec(),
    }
}

/// A healthy x86_64 host whose service answers at once, in the `massorca --json` envelope.
fn healthy(command: &str) -> RemoteOutput {
    if command == "uname -sm" {
        ok("Linux x86_64\n")
    } else if command.contains("pair offer") {
        ok(&format!(
            r#"{{"ok":true,"result":{{"offer":"{OFFER}","expires_at_ms":1}}}}"#
        ))
    } else if command.contains("service status") {
        ok(r#"{"ok":true,"result":{"version":"0.9.0-test"}}"#)
    } else {
        ok("")
    }
}

/// An ELF header for `machine`, then a payload that must arrive unchanged.
fn elf(machine: u16) -> Vec<u8> {
    let mut bytes = vec![0_u8; 64];
    bytes[..4].copy_from_slice(b"\x7fELF");
    bytes[4] = 2; // 64-bit
    bytes[5] = 1; // little-endian
    bytes[6] = 1; // ELF version
    bytes[18..20].copy_from_slice(&machine.to_le_bytes());
    bytes.extend_from_slice(b"fake-massorca-payload");
    bytes
}

fn binary_in(dir: &TempDir, bytes: &[u8]) -> PathBuf {
    let path = dir.path().join("massorca");
    fs::write(&path, bytes).expect("write the fake binary");
    path
}

fn plan(binary: PathBuf) -> BootstrapPlan {
    BootstrapPlan {
        binary,
        listen: vec!["tailscale".to_owned()],
        remote_dir: DEFAULT_REMOTE_DIR.to_owned(),
        device_name: "laptop".to_owned(),
        offer_ttl_secs: DEFAULT_OFFER_TTL_SECS,
    }
}

/// Runs bootstrap and returns its result with every progress line it printed.
fn run(
    shell: &dyn RemoteShell,
    plan: &BootstrapPlan,
) -> (Result<BootstrapReport, BootstrapError>, Vec<String>) {
    let mut lines = Vec::new();
    let result = bootstrap(shell, plan, &mut |line: &str| lines.push(line.to_owned()));
    (result, lines)
}

#[test]
fn defaults_match_the_decision() {
    assert_eq!(DEFAULT_REMOTE_DIR, "~/.massorca");
    assert_eq!(DEFAULT_OFFER_TTL_SECS, 120);
    assert_eq!(DEFAULT_LISTEN, "tailscale");
}

#[test]
fn the_ssh_host_plan_takes_the_default_directory_offer_lifetime_and_listen() {
    let plan = BootstrapPlan::for_ssh_host(
        PathBuf::from("massorca"),
        vec![DEFAULT_LISTEN.to_owned()],
        "laptop".to_owned(),
    );
    assert_eq!(plan.binary, PathBuf::from("massorca"));
    assert_eq!(plan.listen, vec!["tailscale".to_owned()]);
    assert_eq!(plan.remote_dir, DEFAULT_REMOTE_DIR);
    assert_eq!(plan.device_name, "laptop");
    assert_eq!(plan.offer_ttl_secs, DEFAULT_OFFER_TTL_SECS);
}

#[test]
fn happy_path_runs_each_step_in_order_and_keeps_the_offer_out_of_view() {
    let dir = TempDir::new().expect("temp dir");
    let bytes = elf(EM_X86_64);
    let shell = FakeShell::new(healthy);
    let (result, progress) = run(&shell, &plan(binary_in(&dir, &bytes)));
    let report = result.expect("bootstrap succeeds");

    let commands = shell.commands();
    assert_eq!(commands.len(), 6, "{commands:#?}");
    assert_eq!(commands[0], "uname -sm");
    assert!(commands[1].starts_with(
        "mkdir -p \"$HOME/.massorca/bin\" \"$HOME/.massorca/home\" \"$HOME/.config/systemd/user\""
    ));
    assert!(
        commands[1].contains(
            "mv -f \"$HOME/.massorca/bin/massorca.new\" \"$HOME/.massorca/bin/massorca\""
        )
    );
    assert_eq!(
        commands[2],
        "cat > \"$HOME/.config/systemd/user/massorca.service\""
    );
    assert_eq!(
        commands[3],
        "systemctl --user daemon-reload && systemctl --user enable massorca.service && systemctl --user restart massorca.service"
    );
    assert!(commands[4].contains("--no-autostart --json service status"));
    assert!(commands[5].ends_with("--no-autostart --json pair offer --admin --ttl 120"));

    assert_eq!(
        shell.stdin(1),
        Some(bytes),
        "the binary is uploaded byte for byte"
    );
    let unit = String::from_utf8(shell.stdin(2).expect("the unit is sent on stdin"))
        .expect("the unit is text");
    assert!(unit.contains(
        "ExecStart=%h/.massorca/bin/massorca service run --listen tailscale --log-stderr\n"
    ));

    assert_eq!(report.remote_os, "Linux");
    assert_eq!(report.remote_arch, "x86_64");
    assert_eq!(report.remote_binary, "~/.massorca/bin/massorca");
    assert_eq!(report.unit_path, "~/.config/systemd/user/massorca.service");
    assert_eq!(report.server_version, "0.9.0-test");
    assert_eq!(report.offer.expose(), OFFER);

    let debug = format!("{report:?}");
    assert!(debug.contains("<redacted>"), "{debug}");
    assert!(
        !debug.contains(OFFER),
        "the offer must not reach Debug output"
    );
    assert!(!progress.is_empty());
    assert!(
        progress.iter().all(|line| !line.contains(OFFER)),
        "{progress:?}"
    );
}

#[test]
fn unsupported_systems_stop_before_anything_changes() {
    let dir = TempDir::new().expect("temp dir");
    let binary = binary_in(&dir, &elf(EM_X86_64));
    for uname in ["Darwin arm64", "Linux i686", "FreeBSD amd64"] {
        let shell = FakeShell::new(move |command: &str| {
            if command == "uname -sm" {
                ok(&format!("{uname}\n"))
            } else {
                healthy(command)
            }
        });
        let (result, _) = run(&shell, &plan(binary.clone()));
        assert!(
            matches!(result, Err(BootstrapError::UnsupportedSystem(_))),
            "{uname}: {result:?}"
        );
        assert_eq!(shell.commands(), ["uname -sm"], "{uname}");
    }
}

#[test]
fn a_binary_for_another_machine_or_not_an_elf_is_refused_before_upload() {
    let mut big_endian = elf(EM_X86_64);
    big_endian[5] = 2;
    let mut thirty_two_bit = elf(EM_X86_64);
    thirty_two_bit[4] = 1;
    let cases = [
        ("aarch64 binary on an x86_64 host", elf(EM_AARCH64)),
        ("32-bit ELF", thirty_two_bit),
        ("big-endian ELF", big_endian),
        ("shell script", b"#!/bin/sh\necho hello\n".to_vec()),
    ];
    for (case, bytes) in cases {
        let dir = TempDir::new().expect("temp dir");
        let shell = FakeShell::new(healthy);
        let (result, _) = run(&shell, &plan(binary_in(&dir, &bytes)));
        assert!(
            matches!(result, Err(BootstrapError::BinaryMismatch(_))),
            "{case}: {result:?}"
        );
        assert_eq!(
            shell.commands(),
            ["uname -sm"],
            "{case}: nothing may change on the host"
        );
    }
}

#[test]
fn a_missing_binary_fails_locally_before_the_host_is_contacted() {
    let dir = TempDir::new().expect("temp dir");
    let shell = FakeShell::new(healthy);
    let (result, _) = run(&shell, &plan(dir.path().join("absent")));
    assert!(matches!(result, Err(BootstrapError::Io(_))), "{result:?}");
    assert!(shell.commands().is_empty());
}

#[test]
fn remote_directory_must_be_a_plain_path_under_home() {
    let dir = TempDir::new().expect("temp dir");
    let binary = binary_in(&dir, &elf(EM_X86_64));
    let rejected = [
        "~",
        "~/",
        "/",
        "/opt/massorca",
        "~/../x",
        "~/a/../b",
        "~/.",
        "~/a b",
        "~/\"q\"",
        "$HOME/x",
        "~/a//b",
        "~/a/",
    ];
    for remote_dir in rejected {
        let mut bad = plan(binary.clone());
        bad.remote_dir = remote_dir.to_owned();
        let shell = FakeShell::new(healthy);
        let (result, _) = run(&shell, &bad);
        assert!(
            matches!(result, Err(BootstrapError::InvalidPlan(_))),
            "{remote_dir}: {result:?}"
        );
        assert!(
            shell.commands().is_empty(),
            "{remote_dir}: nothing may be sent"
        );
    }
}

#[test]
fn a_custom_remote_directory_is_used_in_every_path() {
    let dir = TempDir::new().expect("temp dir");
    let mut custom = plan(binary_in(&dir, &elf(EM_X86_64)));
    custom.remote_dir = "~/apps/massorca-2".to_owned();
    let shell = FakeShell::new(healthy);
    let (result, _) = run(&shell, &custom);
    let report = result.expect("bootstrap succeeds");

    let commands = shell.commands();
    assert!(commands[1].contains("\"$HOME/apps/massorca-2/bin/massorca.new\""));
    assert!(commands[4].contains("MASSORCA_HOME=\"$HOME/apps/massorca-2/home\""));
    assert!(commands[5].contains("\"$HOME/apps/massorca-2/bin/massorca\""));
    assert_eq!(report.remote_binary, "~/apps/massorca-2/bin/massorca");
    let unit = String::from_utf8(shell.stdin(2).expect("the unit is sent")).expect("text");
    assert!(unit.contains("MASSORCA_HOME=%h/apps/massorca-2/home"));
}

#[test]
fn listen_addresses_use_only_safe_characters_and_stay_short() {
    let longest = "a".repeat(64);
    let too_long = "a".repeat(65);
    for good in [
        "tailscale",
        "tailscale:7347",
        "localhost:7347",
        "100.64.0.1:7347",
        "[fd7a:115c:a1e0::1]:7347",
        longest.as_str(),
    ] {
        assert!(validate_listen(good).is_ok(), "{good} should be accepted");
    }
    for bad in ["a b", "$(id)", "x;y", "", too_long.as_str()] {
        assert!(
            matches!(validate_listen(bad), Err(BootstrapError::InvalidPlan(_))),
            "{bad:?} should be refused"
        );
    }
}

#[test]
fn a_bad_or_missing_listen_list_is_refused_before_the_host_is_contacted() {
    let dir = TempDir::new().expect("temp dir");
    let binary = binary_in(&dir, &elf(EM_X86_64));
    for listen in [Vec::new(), vec!["0.0.0.0:7347; rm -rf ~".to_owned()]] {
        let mut bad = plan(binary.clone());
        bad.listen = listen;
        let shell = FakeShell::new(healthy);
        let (result, _) = run(&shell, &bad);
        assert!(
            matches!(result, Err(BootstrapError::InvalidPlan(_))),
            "{result:?}"
        );
        assert!(shell.commands().is_empty());
    }
}

#[test]
fn a_service_that_never_answers_is_a_remote_error_and_no_offer_is_requested() {
    let dir = TempDir::new().expect("temp dir");
    let shell = FakeShell::new(|command: &str| {
        if command.contains("service status") {
            fail(1, "the service did not answer within 20 seconds")
        } else {
            healthy(command)
        }
    });
    let (result, _) = run(&shell, &plan(binary_in(&dir, &elf(EM_X86_64))));
    match result {
        Err(BootstrapError::Remote(message)) => {
            assert!(message.contains("did not answer"), "{message}");
        }
        other => panic!("expected a remote error, got {other:?}"),
    }
    assert!(
        shell
            .commands()
            .iter()
            .all(|command| !command.contains("pair offer"))
    );
}

#[test]
fn an_offer_reply_without_an_offer_is_refused_without_echoing_the_reply() {
    let dir = TempDir::new().expect("temp dir");
    let shell = FakeShell::new(|command: &str| {
        if command.contains("pair offer") {
            ok(r#"{"unexpected":"SHOULD-NOT-APPEAR"}"#)
        } else {
            healthy(command)
        }
    });
    let (result, _) = run(&shell, &plan(binary_in(&dir, &elf(EM_X86_64))));
    match result {
        Err(BootstrapError::Remote(message)) => {
            assert!(!message.contains("SHOULD-NOT-APPEAR"), "{message}");
        }
        other => panic!("expected a remote error, got {other:?}"),
    }
}

#[test]
fn a_failed_offer_does_not_leak_its_error_output() {
    let dir = TempDir::new().expect("temp dir");
    let shell = FakeShell::new(|command: &str| {
        if command.contains("pair offer") {
            fail(1, &format!("debug output {OFFER}"))
        } else {
            healthy(command)
        }
    });
    let (result, _) = run(&shell, &plan(binary_in(&dir, &elf(EM_X86_64))));
    match result {
        Err(BootstrapError::Remote(message)) => {
            assert!(!message.contains(OFFER), "{message}");
        }
        other => panic!("expected a remote error, got {other:?}"),
    }
}

#[test]
fn transport_failures_surface_as_ssh_errors() {
    let dir = TempDir::new().expect("temp dir");
    let (result, _) = run(&UnreachableHost, &plan(binary_in(&dir, &elf(EM_X86_64))));
    assert!(
        matches!(
            result,
            Err(BootstrapError::Ssh(SshError::Failed { status: 255, .. }))
        ),
        "{result:?}"
    );
}

#[test]
fn uninstall_stops_removes_the_unit_and_the_directory_in_order() {
    let shell = FakeShell::new(|_: &str| ok(""));
    uninstall(&shell, DEFAULT_REMOTE_DIR).expect("uninstall succeeds");
    assert_eq!(
        shell.commands(),
        [
            "systemctl --user disable --now massorca.service",
            "rm -f \"$HOME/.config/systemd/user/massorca.service\"",
            "systemctl --user daemon-reload",
            "rm -rf \"$HOME/.massorca\"",
        ]
    );
}

#[test]
fn uninstall_treats_a_unit_that_was_never_loaded_as_already_removed() {
    let shell = FakeShell::new(|command: &str| {
        if command.starts_with("systemctl --user disable") {
            fail(
                1,
                "Failed to disable unit: Unit file massorca.service does not exist.",
            )
        } else {
            ok("")
        }
    });
    uninstall(&shell, DEFAULT_REMOTE_DIR).expect("uninstall succeeds");
    assert_eq!(shell.commands().len(), 4);
}

#[test]
fn uninstall_keeps_everything_when_the_service_cannot_be_stopped() {
    let shell = FakeShell::new(|command: &str| {
        if command.starts_with("systemctl --user disable") {
            fail(1, "Failed to connect to bus: No medium found")
        } else {
            ok("")
        }
    });
    let result = uninstall(&shell, DEFAULT_REMOTE_DIR);
    assert!(
        matches!(result, Err(BootstrapError::Remote(_))),
        "{result:?}"
    );
    assert_eq!(shell.commands().len(), 1);
}

#[test]
fn uninstall_keeps_everything_when_the_user_bus_is_missing() {
    // A missing bus socket is a real failure, not an absent unit: the service may still run.
    let shell = FakeShell::new(|command: &str| {
        if command.starts_with("systemctl --user disable") {
            fail(1, "Failed to connect to bus: No such file or directory")
        } else {
            ok("")
        }
    });
    let result = uninstall(&shell, DEFAULT_REMOTE_DIR);
    assert!(
        matches!(result, Err(BootstrapError::Remote(_))),
        "{result:?}"
    );
    assert_eq!(shell.commands().len(), 1);
}

#[test]
fn uninstall_never_removes_home_or_the_root() {
    for remote_dir in ["~", "~/", "/", "~/..", "~/."] {
        let shell = FakeShell::new(|_: &str| ok(""));
        let result = uninstall(&shell, remote_dir);
        assert!(
            matches!(result, Err(BootstrapError::InvalidPlan(_))),
            "{remote_dir}: {result:?}"
        );
        assert!(shell.commands().is_empty(), "{remote_dir}");
    }
}
