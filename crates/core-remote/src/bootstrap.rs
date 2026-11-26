//! Installs, upgrades and removes the MassOrca service on a Linux host over SSH (decisions 0044, 0045).

use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use thiserror::Error;

use crate::process::excerpt;
use crate::ssh::{RemoteOutput, RemoteShell, SshError, SystemSsh};

/// Install directory on the host unless the plan says otherwise.
pub const DEFAULT_REMOTE_DIR: &str = "~/.massorca";
/// Validity of the pairing offer made during bootstrap (decision 0044).
pub const DEFAULT_OFFER_TTL_SECS: u32 = 120;
/// The listen address of an installed service when none is given: this node's Tailscale address (decision 0043).
pub const DEFAULT_LISTEN: &str = "tailscale";

/// The systemd user unit that runs the service on an SSH host; its logs are read with `journalctl --user -u` (REM-25).
pub const UNIT_NAME: &str = "massorca.service";

/// The unit file as the host's shell expands it.
fn unit_file() -> String {
    format!("$HOME/.config/systemd/user/{UNIT_NAME}")
}

/// The unit file as it is shown to people.
fn unit_path_display() -> String {
    format!("~/.config/systemd/user/{UNIT_NAME}")
}

fn enable_unit() -> String {
    format!(
        "systemctl --user daemon-reload && systemctl --user enable {UNIT_NAME} && systemctl --user restart {UNIT_NAME}"
    )
}
/// Readiness checks of the service, half a second apart: 20 seconds in all.
const READY_POLLS: u32 = 40;

/// The ELF header bytes checked here: magic and class (0..5), e_machine (18..20).
const ELF_HEADER_LEN: usize = 20;
const ELF_CLASS_64: u8 = 2;
const ELF_DATA_LSB: u8 = 1;
const EM_X86_64: u16 = 0x3E;
const EM_AARCH64: u16 = 0xB7;

/// What to install and how the service listens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootstrapPlan {
    /// The Linux `massorca` binary on this machine.
    pub binary: PathBuf,
    /// Listen addresses for `service run --listen`, such as `tailscale`.
    pub listen: Vec<String>,
    /// Install directory on the host, `~/<path>`.
    pub remote_dir: String,
    /// Name the pairing device gets on the host, given when the caller accepts the offer.
    pub device_name: String,
    /// Validity of the pairing offer, in seconds.
    pub offer_ttl_secs: u32,
}

impl BootstrapPlan {
    /// The plan `host add-ssh` and the desktop share: the default install directory and offer lifetime.
    pub fn for_ssh_host(binary: PathBuf, listen: Vec<String>, device_name: String) -> Self {
        Self {
            binary,
            listen,
            remote_dir: DEFAULT_REMOTE_DIR.to_owned(),
            device_name,
            offer_ttl_secs: DEFAULT_OFFER_TTL_SECS,
        }
    }
}

/// What bootstrap found on the host and installed there.
#[derive(Debug)]
pub struct BootstrapReport {
    pub remote_os: String,
    pub remote_arch: String,
    /// The installed binary, as `~/<path>`.
    pub remote_binary: String,
    /// The installed unit file, as `~/<path>`.
    pub unit_path: String,
    pub offer: SecretOffer,
    /// The service's reported version; empty when it reports none.
    pub server_version: String,
}

/// A pairing offer. It is a secret: `Debug` prints `<redacted>` and there is no `Display`.
pub struct SecretOffer(String);

impl SecretOffer {
    /// The offer text, for the pairing step only. Never log or print it.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for SecretOffer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("<redacted>")
    }
}

/// Why bootstrap or uninstall stopped.
#[derive(Debug, Error)]
pub enum BootstrapError {
    #[error(transparent)]
    Ssh(#[from] SshError),
    #[error("unsupported system: {0}")]
    UnsupportedSystem(String),
    #[error("the binary does not fit the host: {0}")]
    BinaryMismatch(String),
    /// A remote step failed; the message names the step and the host's error output.
    #[error("{0}")]
    Remote(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("invalid plan: {0}")]
    InvalidPlan(String),
}

/// Inputs of the systemd user unit. Paths use systemd's `%h` (the user's home).
#[derive(Debug, Clone, Copy)]
pub struct UnitSpec<'a> {
    pub exe: &'a str,
    pub home: &'a str,
    pub listen: &'a [String],
}

/// The systemd user unit of decision 0045.
pub fn systemd_unit(spec: &UnitSpec<'_>) -> String {
    let mut listen = String::new();
    for address in spec.listen {
        listen.push_str(" --listen ");
        listen.push_str(address);
    }
    format!(
        "[Unit]\n\
         Description=MassOrca service\n\
         After=network-online.target\n\
         \n\
         [Service]\n\
         Environment=MASSORCA_HOME={home}\n\
         ExecStart={exe} service run{listen} --log-stderr\n\
         Restart=on-failure\n\
         RestartSec=5\n\
         \n\
         [Install]\n\
         WantedBy=default.target\n",
        home = spec.home,
        exe = spec.exe,
    )
}

/// Checks that `path` is a 64-bit little-endian ELF executable for the host's `uname -m`.
pub fn check_linux_binary(path: &Path, arch: &str) -> Result<(), BootstrapError> {
    let mut header = Vec::with_capacity(ELF_HEADER_LEN);
    File::open(path)?
        .take(ELF_HEADER_LEN as u64)
        .read_to_end(&mut header)?;
    check_elf(&header, arch)
}

/// Accepts only characters that are safe in the unit file and the remote shell, up to 64 long.
pub fn validate_listen(value: &str) -> Result<(), BootstrapError> {
    let safe = value
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || b".:[]_-".contains(&byte));
    if safe && (1..=64).contains(&value.len()) {
        Ok(())
    } else {
        Err(BootstrapError::InvalidPlan(
            "a listen address is 1 to 64 letters, digits or any of . : [ ] _ -".to_owned(),
        ))
    }
}

/// Installs or upgrades the service: checks the host and binary, uploads the binary, writes and
/// starts the unit, waits for the service, then asks it for a pairing offer (decision 0044).
pub fn bootstrap(
    shell: &dyn RemoteShell,
    plan: &BootstrapPlan,
    progress: &mut dyn FnMut(&str),
) -> Result<BootstrapReport, BootstrapError> {
    let rest = remote_rest(&plan.remote_dir)?;
    check_listen_list(&plan.listen)?;
    let binary = std::fs::read(&plan.binary)?;

    progress("Checking the remote system");
    let RemoteMachine {
        os: remote_os,
        arch: remote_arch,
    } = probe_machine(shell)?;
    check_elf(&binary, &remote_arch)?;

    progress("Uploading the binary");
    checked(
        shell.run(&upload_command(rest), Some(&binary))?,
        "uploading the binary",
    )?;

    progress("Writing the systemd user unit");
    let unit = systemd_unit(&UnitSpec {
        exe: &format!("%h/{rest}/bin/massorca"),
        home: &format!("%h/{rest}/home"),
        listen: &plan.listen,
    });
    checked(
        shell.run(&format!("cat > \"{}\"", unit_file()), Some(unit.as_bytes()))?,
        "writing the unit file",
    )?;
    checked(shell.run(&enable_unit(), None)?, "starting the service")?;

    progress("Waiting for the service");
    let ready = checked(
        shell.run(&wait_command(rest), None)?,
        "waiting for the service",
    )?;

    progress("Creating a pairing offer");
    let offer_command = service_command(
        rest,
        &format!("--json pair offer --admin --ttl {}", plan.offer_ttl_secs),
    );
    let offer = offer_from(&shell.run(&offer_command, None)?)?;

    Ok(BootstrapReport {
        remote_os,
        remote_arch,
        remote_binary: format!("~/{rest}/bin/massorca"),
        unit_path: unit_path_display(),
        offer,
        server_version: version_of(&ready.stdout),
    })
}

/// Installs or upgrades the service on the SSH host `target` (`[user@]host`) with `ssh` options such as
/// `-p 2222`. It blocks while ssh runs, so a UI calls it on a thread of its own.
pub fn install_over_ssh(
    target: &str,
    ssh_args: Vec<String>,
    plan: &BootstrapPlan,
    progress: &mut dyn FnMut(&str),
) -> Result<BootstrapReport, BootstrapError> {
    let shell = SystemSsh::new(target, ssh_args)?;
    bootstrap(&shell, plan, progress)
}

/// The machine an SSH host reports for `uname -sm`, once it is one MassOrca runs on (decision 0044).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoteMachine {
    /// The host's system, `Linux`.
    pub os: String,
    /// The host's machine, `x86_64` or `aarch64`.
    pub arch: String,
}

/// Checks the host with `uname -sm` and names its machine. Installs nothing, so a UI can test a host with it.
pub fn probe_machine(shell: &dyn RemoteShell) -> Result<RemoteMachine, BootstrapError> {
    let uname = checked(shell.run("uname -sm", None)?, "checking the remote system")?;
    let (os, arch) = parse_uname(&uname.stdout)?;
    Ok(RemoteMachine { os, arch })
}

/// Probes the SSH host `target` (`[user@]host`) with `ssh_args`, as `probe_machine` does. It blocks while ssh runs.
pub fn probe_over_ssh(
    target: &str,
    ssh_args: Vec<String>,
) -> Result<RemoteMachine, BootstrapError> {
    let shell = SystemSsh::new(target, ssh_args)?;
    probe_machine(&shell)
}

/// Stops and disables the service, then removes its unit and the install directory (decision 0045).
pub fn uninstall(shell: &dyn RemoteShell, remote_dir: &str) -> Result<(), BootstrapError> {
    let rest = remote_rest(remote_dir)?;
    let stop = shell.run(&format!("systemctl --user disable --now {UNIT_NAME}"), None)?;
    if stop.status != 0 && !is_not_loaded(&stop.stderr) {
        return Err(failed("stopping the service", &stop));
    }
    checked(
        shell.run(&format!("rm -f \"{}\"", unit_file()), None)?,
        "removing the unit file",
    )?;
    checked(
        shell.run("systemctl --user daemon-reload", None)?,
        "reloading systemd",
    )?;
    checked(
        shell.run(&format!("rm -rf \"$HOME/{rest}\""), None)?,
        "removing the install directory",
    )?;
    Ok(())
}

fn check_listen_list(listen: &[String]) -> Result<(), BootstrapError> {
    if listen.is_empty() {
        return Err(BootstrapError::InvalidPlan(
            "at least one listen address is needed".to_owned(),
        ));
    }
    listen
        .iter()
        .try_for_each(|address| validate_listen(address))
}

/// The part after `~/` of a remote directory. Only plain relative paths pass, so the remote
/// shell never meets a quote, a space, or a `..` that could leave the directory.
fn remote_rest(remote_dir: &str) -> Result<&str, BootstrapError> {
    let rest = remote_dir
        .strip_prefix("~/")
        .ok_or_else(invalid_remote_dir)?;
    let plain = rest.split('/').all(|part| {
        !part.is_empty()
            && part != "."
            && part != ".."
            && part
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
    });
    if plain {
        Ok(rest)
    } else {
        Err(invalid_remote_dir())
    }
}

fn invalid_remote_dir() -> BootstrapError {
    BootstrapError::InvalidPlan(
        "the remote directory must be ~/ followed by letters, digits, . _ - and /, with no . or .. parts"
            .to_owned(),
    )
}

/// The host must be Linux on one of the two supported machines; returns its name and machine.
fn parse_uname(stdout: &[u8]) -> Result<(String, String), BootstrapError> {
    let text = String::from_utf8_lossy(stdout);
    let mut words = text.split_whitespace();
    match (words.next(), words.next()) {
        (Some("Linux"), Some(arch @ ("x86_64" | "aarch64"))) => {
            Ok(("Linux".to_owned(), arch.to_owned()))
        }
        _ => Err(BootstrapError::UnsupportedSystem(format!(
            "the host reports `{}`; MassOrca needs Linux on x86_64 or aarch64",
            excerpt(stdout)
        ))),
    }
}

/// Checks the ELF header of `bytes` against the host's machine type.
fn check_elf(bytes: &[u8], arch: &str) -> Result<(), BootstrapError> {
    let machine = match arch {
        "x86_64" => EM_X86_64,
        "aarch64" => EM_AARCH64,
        other => {
            return Err(BootstrapError::UnsupportedSystem(format!(
                "architecture {other} is not supported"
            )));
        }
    };
    if bytes.len() < ELF_HEADER_LEN || !bytes.starts_with(b"\x7fELF") {
        return Err(BootstrapError::BinaryMismatch(
            "the file is not an ELF executable".to_owned(),
        ));
    }
    if bytes[4] != ELF_CLASS_64 {
        return Err(BootstrapError::BinaryMismatch(
            "the ELF file is not 64-bit".to_owned(),
        ));
    }
    if bytes[5] != ELF_DATA_LSB {
        return Err(BootstrapError::BinaryMismatch(
            "the ELF file is not little-endian".to_owned(),
        ));
    }
    let found = u16::from_le_bytes([bytes[18], bytes[19]]);
    if found != machine {
        return Err(BootstrapError::BinaryMismatch(format!(
            "the binary is built for {}, the remote host is {arch}",
            machine_name(found)
        )));
    }
    Ok(())
}

fn machine_name(machine: u16) -> String {
    match machine {
        EM_X86_64 => "x86_64".to_owned(),
        EM_AARCH64 => "aarch64".to_owned(),
        other => format!("machine type 0x{other:x}"),
    }
}

/// Makes the install directories, then writes the binary as `.new` and renames it into place.
fn upload_command(rest: &str) -> String {
    format!(
        "mkdir -p \"$HOME/{rest}/bin\" \"$HOME/{rest}/home\" \"$HOME/.config/systemd/user\" \
         && chmod 700 \"$HOME/{rest}\" \
         && cat > \"$HOME/{rest}/bin/massorca.new\" \
         && chmod 700 \"$HOME/{rest}/bin/massorca.new\" \
         && mv -f \"$HOME/{rest}/bin/massorca.new\" \"$HOME/{rest}/bin/massorca\""
    )
}

/// Polls the service until it answers, then prints its last status reply.
fn wait_command(rest: &str) -> String {
    let probe = service_command(rest, "--json service status");
    format!(
        "i=0; while ! out=$({probe} 2>/dev/null); do i=$((i+1)); \
         if [ \"$i\" -ge {READY_POLLS} ]; then echo 'the service did not answer within 20 seconds' >&2; exit 1; fi; \
         sleep 0.5; done; printf '%s' \"$out\""
    )
}

/// The installed `massorca`, with its home in the install directory and no autostart.
fn service_command(rest: &str, arguments: &str) -> String {
    format!(
        "MASSORCA_HOME=\"$HOME/{rest}/home\" \"$HOME/{rest}/bin/massorca\" --no-autostart {arguments}"
    )
}

#[derive(Deserialize)]
struct OfferReply {
    offer: Option<String>,
}

#[derive(Deserialize)]
struct StatusReply {
    version: Option<String>,
}

/// `massorca --json` wraps its result as `{"ok": true, "result": ...}`; a bare object is read as is.
#[derive(Deserialize)]
#[serde(untagged)]
enum Reply<T> {
    Envelope { result: T },
    Bare(T),
}

impl<T> Reply<T> {
    fn into_inner(self) -> T {
        match self {
            Self::Envelope { result } | Self::Bare(result) => result,
        }
    }
}

/// Reads the offer from a `pair offer` reply. The reply holds the secret, so no part of it,
/// stderr included, goes into an error.
fn offer_from(reply: &RemoteOutput) -> Result<SecretOffer, BootstrapError> {
    if reply.status != 0 {
        return Err(BootstrapError::Remote(format!(
            "the pairing offer failed (exit status {})",
            reply.status
        )));
    }
    let parsed: OfferReply = serde_json::from_slice::<Reply<OfferReply>>(&reply.stdout)
        .map(Reply::into_inner)
        .map_err(|_| BootstrapError::Remote("the pairing offer reply is not JSON".to_owned()))?;
    match parsed.offer {
        Some(offer)
            if !offer.is_empty() && !offer.chars().any(|c| c.is_whitespace() || c.is_control()) =>
        {
            Ok(SecretOffer(offer))
        }
        _ => Err(BootstrapError::Remote(
            "the pairing offer reply has no offer".to_owned(),
        )),
    }
}

fn version_of(stdout: &[u8]) -> String {
    serde_json::from_slice::<Reply<StatusReply>>(stdout)
        .ok()
        .and_then(|reply| reply.into_inner().version)
        .unwrap_or_default()
}

/// systemd's answer for a unit that is not there, which uninstall treats as already removed.
/// Any other failure (no user bus, permissions) keeps the files, since the service may run.
fn is_not_loaded(stderr: &[u8]) -> bool {
    let text = String::from_utf8_lossy(stderr).to_lowercase();
    text.contains(UNIT_NAME)
        && (text.contains("not loaded") || text.contains("does not exist"))
}

/// A non-zero exit becomes a `Remote` error that names the step and shows the error output.
fn checked(output: RemoteOutput, step: &str) -> Result<RemoteOutput, BootstrapError> {
    if output.status == 0 {
        Ok(output)
    } else {
        Err(failed(step, &output))
    }
}

fn failed(step: &str, output: &RemoteOutput) -> BootstrapError {
    BootstrapError::Remote(format!(
        "{step} failed (exit status {}): {}",
        output.status,
        excerpt(&output.stderr)
    ))
}
