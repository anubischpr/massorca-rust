//! `massorca terminal attach`: the local terminal shows the remote terminal and types into it, until Ctrl+] or the terminal's exit.
//!
//! Output bytes reach stdout unchanged, so the remote program draws on the local terminal. On Unix input is read as raw
//! bytes, which keeps terminal replies such as the cursor-position answer intact; crossterm's Unix reader drops those.
//! On Windows raw console reads carry no arrow keys, so key events are turned back into the bytes a terminal sends.

use std::io::{self, IsTerminal, Write};
#[cfg(windows)]
use std::sync::Arc;
#[cfg(windows)]
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(windows)]
use std::time::Duration;

use crossterm::terminal as tty;
use massorca_client::{AttachUpdate, Client, ClientError, TerminalAttachment};
use massorca_protocol::methods::terminals::{Terminal, TerminalAttachParams, TerminalResizeParams};
use tokio::sync::mpsc;

use crate::error::{CliError, exit};
use crate::terminal::exit_text;

/// The byte Ctrl+] sends; typing it detaches instead of reaching the terminal.
const DETACH_BYTE: u8 = 0x1d;

/// One thing the local terminal did that the attach must act on.
enum Input {
    Bytes(Vec<u8>),
    Resize(u16, u16),
    /// The input is over (stdin closed, or a hangup or termination signal).
    #[cfg(unix)]
    End,
}

type InputMessage = io::Result<Input>;

/// How an attach ended.
enum Ended {
    Detached,
    Exited(Terminal),
}

/// Refuses to attach unless stdin and stdout are both terminals. Checked before connecting.
pub fn require_terminal() -> Result<(), CliError> {
    if io::stdin().is_terminal() && io::stdout().is_terminal() {
        return Ok(());
    }
    Err(CliError::new(
        "usage.not_a_terminal",
        "attach needs a terminal on stdin and stdout; `terminal screen` prints a screen without one",
        exit::USAGE,
    ))
}

/// Attaches to `terminal` until the user detaches or the terminal exits. Either way the local terminal is restored first.
pub async fn attach(client: &Client, terminal: &str) -> Result<(), CliError> {
    #[cfg(windows)]
    {
        // Enables VT processing on the console output so the remote program's escape sequences draw.
        let _ = crossterm::ansi_support::supports_ansi();
    }
    let (cols, rows) = match tty::size() {
        Ok((cols, rows)) => (Some(cols), Some(rows)),
        Err(_) => (None, None),
    };
    let mut attachment = client
        .terminal_attach(&TerminalAttachParams {
            terminal: terminal.to_owned(),
            since: None,
            cols,
            rows,
            answers_queries: None,
        })
        .await?;
    write_stderr(&format!(
        "attached to terminal {terminal}; press Ctrl+] to detach\n"
    ));
    let ended = run_attached(client, &mut attachment, terminal).await?;
    match ended {
        Ended::Detached => write_stderr(&format!("detached from terminal {terminal}\n")),
        Ended::Exited(exited) => match &exited.exit {
            Some(exit) => write_stderr(&format!(
                "terminal {} exited ({})\n",
                exited.id,
                exit_text(exit)
            )),
            None => write_stderr(&format!("terminal {} exited\n", exited.id)),
        },
    }
    Ok(())
}

/// Runs the session with the local terminal in raw mode. The guard and the input source drop on every path, which restores the terminal.
async fn run_attached(
    client: &Client,
    attachment: &mut TerminalAttachment,
    terminal: &str,
) -> Result<Ended, CliError> {
    let _raw = RawMode::enable()?;
    let mut inputs = start_inputs()?;
    session(client, attachment, &mut inputs.receiver, terminal).await
}

async fn session(
    client: &Client,
    attachment: &mut TerminalAttachment,
    inputs: &mut mpsc::UnboundedReceiver<InputMessage>,
    terminal: &str,
) -> Result<Ended, CliError> {
    loop {
        tokio::select! {
            update = attachment.next() => {
                let Some(update) = update else {
                    return Err(ClientError::Disconnected.into());
                };
                match update? {
                    AttachUpdate::Snapshot { bytes, .. } | AttachUpdate::Output { bytes, .. } => {
                        write_output(&bytes)?;
                    }
                    AttachUpdate::Exited { terminal: exited } => return Ok(Ended::Exited(exited)),
                    AttachUpdate::Attached { .. } | AttachUpdate::Resized { .. } => {}
                }
            }
            input = inputs.recv() => match input {
                None => return Ok(Ended::Detached),
                #[cfg(unix)]
                Some(Ok(Input::End)) => return Ok(Ended::Detached),
                Some(Err(error)) => return Err(error.into()),
                Some(Ok(Input::Resize(cols, rows))) => {
                    client
                        .terminal_resize(&TerminalResizeParams {
                            terminal: terminal.to_owned(),
                            cols,
                            rows,
                        })
                        .await?;
                }
                Some(Ok(Input::Bytes(bytes))) => {
                    let (typed, detach) = match bytes.iter().position(|&byte| byte == DETACH_BYTE) {
                        Some(index) => (&bytes[..index], true),
                        None => (&bytes[..], false),
                    };
                    if !typed.is_empty() {
                        client.terminal_input(terminal, typed).await?;
                    }
                    if detach {
                        return Ok(Ended::Detached);
                    }
                }
            }
        }
    }
}

fn write_output(bytes: &[u8]) -> io::Result<()> {
    let mut stdout = io::stdout().lock();
    stdout.write_all(bytes)?;
    stdout.flush()
}

fn write_stderr(text: &str) {
    let _ = io::stderr().lock().write_all(text.as_bytes());
}

/// Raw mode for the local terminal, restored when dropped.
struct RawMode;

impl RawMode {
    fn enable() -> io::Result<Self> {
        tty::enable_raw_mode()?;
        Ok(Self)
    }
}

impl Drop for RawMode {
    fn drop(&mut self) {
        let _ = tty::disable_raw_mode();
    }
}

/// Where the attach's input comes from. Dropping it stops the producers.
struct Inputs {
    receiver: mpsc::UnboundedReceiver<InputMessage>,
    #[cfg(unix)]
    signals: tokio::task::JoinHandle<()>,
    #[cfg(windows)]
    stop: Arc<AtomicBool>,
}

impl Drop for Inputs {
    fn drop(&mut self) {
        #[cfg(unix)]
        self.signals.abort();
        #[cfg(windows)]
        self.stop.store(true, Ordering::Relaxed);
    }
}

/// Reads stdin on a thread of its own: the thread blocks in `read`, and the process ends with the attach.
#[cfg(unix)]
fn start_inputs() -> io::Result<Inputs> {
    use tokio::signal::unix::{SignalKind, signal};

    let (sender, receiver) = mpsc::unbounded_channel();
    let stdin_sender = sender.clone();
    std::thread::spawn(move || read_stdin(&stdin_sender));
    let mut window_change = signal(SignalKind::window_change())?;
    let mut hangup = signal(SignalKind::hangup())?;
    let mut terminate = signal(SignalKind::terminate())?;
    let signals = tokio::spawn(async move {
        loop {
            tokio::select! {
                Some(()) = window_change.recv() => {
                    let Ok((cols, rows)) = tty::size() else {
                        continue;
                    };
                    if sender.send(Ok(Input::Resize(cols, rows))).is_err() {
                        return;
                    }
                }
                _ = hangup.recv() => {
                    let _ = sender.send(Ok(Input::End));
                    return;
                }
                _ = terminate.recv() => {
                    let _ = sender.send(Ok(Input::End));
                    return;
                }
            }
        }
    });
    Ok(Inputs { receiver, signals })
}

#[cfg(unix)]
fn read_stdin(sender: &mpsc::UnboundedSender<InputMessage>) {
    use std::io::Read;

    let mut stdin = io::stdin().lock();
    let mut buffer = [0u8; 4096];
    loop {
        let message = match stdin.read(&mut buffer) {
            Ok(0) => Ok(Input::End),
            Ok(count) => Ok(Input::Bytes(buffer[..count].to_vec())),
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => Err(error),
        };
        let finished = !matches!(&message, Ok(Input::Bytes(_)));
        if sender.send(message).is_err() || finished {
            return;
        }
    }
}

/// Polls console events on a thread of its own, so the loop can stop it between polls.
#[cfg(windows)]
fn start_inputs() -> io::Result<Inputs> {
    let (sender, receiver) = mpsc::unbounded_channel();
    let stop = Arc::new(AtomicBool::new(false));
    let thread_stop = Arc::clone(&stop);
    std::thread::spawn(move || read_events(&sender, &thread_stop));
    Ok(Inputs { receiver, stop })
}

#[cfg(windows)]
fn read_events(sender: &mpsc::UnboundedSender<InputMessage>, stop: &AtomicBool) {
    use crossterm::event::{self, Event, KeyEventKind};

    while !stop.load(Ordering::Relaxed) {
        let message = match event::poll(Duration::from_millis(100)) {
            Ok(false) => continue,
            Ok(true) => match event::read() {
                Ok(Event::Key(key)) if key.kind != KeyEventKind::Release => match key_bytes(key) {
                    Some(bytes) => Ok(Input::Bytes(bytes)),
                    None => continue,
                },
                Ok(Event::Resize(cols, rows)) => Ok(Input::Resize(cols, rows)),
                Ok(_) => continue,
                Err(error) => Err(error),
            },
            Err(error) => Err(error),
        };
        let finished = message.is_err();
        if sender.send(message).is_err() || finished {
            return;
        }
    }
}

/// The bytes a terminal sends for one key press. A key without a byte form (a lone modifier, say) gives `None`.
#[cfg(windows)]
fn key_bytes(key: crossterm::event::KeyEvent) -> Option<Vec<u8>> {
    use crossterm::event::{KeyCode, KeyModifiers};

    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let alt = key.modifiers.contains(KeyModifiers::ALT);
    // Windows reports AltGr as Ctrl+Alt, and AltGr types a character, so it is not a control key.
    let altgr = ctrl && alt;
    let body: Vec<u8> = match key.code {
        KeyCode::Char(character) if ctrl && !alt => {
            control_byte(character).map_or_else(|| utf8(character), |byte| vec![byte])
        }
        KeyCode::Char(character) => utf8(character),
        KeyCode::Enter => b"\r".to_vec(),
        KeyCode::Tab => b"\t".to_vec(),
        KeyCode::BackTab => b"\x1b[Z".to_vec(),
        KeyCode::Backspace => vec![0x7f],
        KeyCode::Esc => vec![0x1b],
        KeyCode::Up => b"\x1b[A".to_vec(),
        KeyCode::Down => b"\x1b[B".to_vec(),
        KeyCode::Right => b"\x1b[C".to_vec(),
        KeyCode::Left => b"\x1b[D".to_vec(),
        KeyCode::Home => b"\x1b[H".to_vec(),
        KeyCode::End => b"\x1b[F".to_vec(),
        KeyCode::Insert => b"\x1b[2~".to_vec(),
        KeyCode::Delete => b"\x1b[3~".to_vec(),
        KeyCode::PageUp => b"\x1b[5~".to_vec(),
        KeyCode::PageDown => b"\x1b[6~".to_vec(),
        KeyCode::F(number) => function_key(number)?,
        KeyCode::Null => vec![0],
        _ => return None,
    };
    if alt && !altgr {
        // Alt sends Escape before the key, as xterm does.
        let mut prefixed = vec![0x1b];
        prefixed.extend(body);
        return Some(prefixed);
    }
    Some(body)
}

#[cfg(windows)]
fn utf8(character: char) -> Vec<u8> {
    character.encode_utf8(&mut [0; 4]).as_bytes().to_vec()
}

/// The control byte for Ctrl+`character`, or `None` when the key has none (Ctrl+1 types the 1).
#[cfg(windows)]
fn control_byte(character: char) -> Option<u8> {
    match character.to_ascii_lowercase() {
        lower @ 'a'..='z' => Some(lower as u8 - b'a' + 1),
        ' ' | '@' => Some(0),
        '[' => Some(0x1b),
        '\\' => Some(0x1c),
        ']' => Some(0x1d),
        '^' => Some(0x1e),
        '_' => Some(0x1f),
        '?' => Some(0x7f),
        _ => None,
    }
}

#[cfg(windows)]
fn function_key(number: u8) -> Option<Vec<u8>> {
    let sequence: &[u8] = match number {
        1 => b"\x1bOP",
        2 => b"\x1bOQ",
        3 => b"\x1bOR",
        4 => b"\x1bOS",
        5 => b"\x1b[15~",
        6 => b"\x1b[17~",
        7 => b"\x1b[18~",
        8 => b"\x1b[19~",
        9 => b"\x1b[20~",
        10 => b"\x1b[21~",
        11 => b"\x1b[23~",
        12 => b"\x1b[24~",
        _ => return None,
    };
    Some(sequence.to_vec())
}

#[cfg(all(test, windows))]
mod tests {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    use super::key_bytes;

    fn bytes(code: KeyCode, modifiers: KeyModifiers) -> Option<Vec<u8>> {
        key_bytes(KeyEvent::new(code, modifiers))
    }

    #[test]
    fn ctrl_bracket_right_is_the_detach_byte() {
        assert_eq!(
            bytes(KeyCode::Char(']'), KeyModifiers::CONTROL),
            Some(vec![0x1d])
        );
    }

    #[test]
    fn ctrl_letters_and_enter_and_backspace_send_terminal_bytes() {
        assert_eq!(
            bytes(KeyCode::Char('c'), KeyModifiers::CONTROL),
            Some(vec![0x03])
        );
        assert_eq!(
            bytes(KeyCode::Enter, KeyModifiers::NONE),
            Some(b"\r".to_vec())
        );
        assert_eq!(
            bytes(KeyCode::Backspace, KeyModifiers::NONE),
            Some(vec![0x7f])
        );
    }

    #[test]
    fn arrows_use_the_csi_forms() {
        assert_eq!(
            bytes(KeyCode::Up, KeyModifiers::NONE),
            Some(b"\x1b[A".to_vec())
        );
        assert_eq!(
            bytes(KeyCode::F(5), KeyModifiers::NONE),
            Some(b"\x1b[15~".to_vec())
        );
    }

    #[test]
    fn alt_prefixes_escape_and_altgr_types_the_character() {
        assert_eq!(
            bytes(KeyCode::Char('x'), KeyModifiers::ALT),
            Some(vec![0x1b, b'x'])
        );
        assert_eq!(
            bytes(
                KeyCode::Char('@'),
                KeyModifiers::CONTROL | KeyModifiers::ALT
            ),
            Some("@".as_bytes().to_vec())
        );
    }

    #[test]
    fn a_reply_to_the_cursor_position_query_passes_through_as_typed_characters() {
        let reply = "\x1b[12;40R";
        let mut out = Vec::new();
        for character in reply.chars() {
            let code = if character == '\x1b' {
                KeyCode::Esc
            } else {
                KeyCode::Char(character)
            };
            out.extend(bytes(code, KeyModifiers::NONE).expect("a key with bytes"));
        }
        assert_eq!(out, reply.as_bytes());
    }
}
