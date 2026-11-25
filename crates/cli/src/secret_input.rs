//! Reading a secret value: all of stdin when it is not a terminal, otherwise a prompt that does not echo. Never an
//! argument or an environment variable, which process listings and shell history keep.

use std::io::{self, IsTerminal, Read, Write};
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::terminal;

use crate::error::{CliError, exit};

/// How long input must stay quiet before a refused paste counts as fully drained.
const DRAIN_QUIET: Duration = Duration::from_millis(50);

/// The value for `name` in `scope`; refuses an empty one so nothing reaches the service.
pub fn read_value(scope: &str, name: &str) -> Result<String, CliError> {
    let value = read_hidden_or_stdin(&format!("Value for {name} in {scope}: "))?;
    if value.is_empty() {
        return Err(CliError::new(
            "secret.empty_value",
            "the value is empty; nothing was stored",
            exit::ERROR,
        ));
    }
    Ok(value)
}

/// All of stdin when it is not a terminal, else one line from a prompt that does not echo. May be empty.
pub fn read_hidden_or_stdin(prompt: &str) -> Result<String, CliError> {
    let stdin = io::stdin();
    if stdin.is_terminal() {
        return prompt_hidden(prompt);
    }
    let mut text = String::new();
    stdin.lock().read_to_string(&mut text)?;
    Ok(strip_one_newline(text))
}

/// Drops one trailing `\n` or `\r\n`, the one `echo` and here-strings add; anything else is part of the value.
fn strip_one_newline(mut text: String) -> String {
    if text.ends_with('\n') {
        text.pop();
        if text.ends_with('\r') {
            text.pop();
        }
    }
    text
}

/// Terminal events, so the prompt's logic runs against a scripted source in tests.
trait EventSource {
    fn read(&mut self) -> io::Result<Event>;
    /// Whether another event arrives within `wait`.
    fn pending(&mut self, wait: Duration) -> io::Result<bool>;
}

struct Terminal;

impl EventSource for Terminal {
    fn read(&mut self) -> io::Result<Event> {
        event::read()
    }

    fn pending(&mut self, wait: Duration) -> io::Result<bool> {
        event::poll(wait)
    }
}

/// What one event does to the value being typed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Step {
    Continue,
    Submit,
    Cancel,
}

fn apply_event(event: Event, value: &mut String) -> Step {
    match event {
        Event::Paste(text) => {
            value.push_str(&text);
            Step::Continue
        }
        Event::Key(key) if key.kind != KeyEventKind::Release => apply_key(key, value),
        _ => Step::Continue,
    }
}

fn apply_key(key: KeyEvent, value: &mut String) -> Step {
    let control = key.modifiers.contains(KeyModifiers::CONTROL);
    if control && matches!(key.code, KeyCode::Char('c' | 'd')) {
        return Step::Cancel;
    }
    if is_chord(key.modifiers) {
        return Step::Continue;
    }
    match key.code {
        KeyCode::Enter => Step::Submit,
        KeyCode::Esc => Step::Cancel,
        KeyCode::Char(c) => {
            value.push(c);
            Step::Continue
        }
        KeyCode::Backspace => {
            value.pop();
            Step::Continue
        }
        _ => Step::Continue,
    }
}

/// A Control or Alt chord is a shortcut, not text.
fn is_chord(modifiers: KeyModifiers) -> bool {
    let control = modifiers.contains(KeyModifiers::CONTROL);
    let alt = modifiers.contains(KeyModifiers::ALT);
    // Windows reports AltGr, which types `@`, `\` and others on many layouts, as Control+Alt.
    if cfg!(windows) && control && alt {
        return false;
    }
    control || alt
}

/// Reads one value from `events` until Enter. Input still pending after Enter is a multi-line paste cut at its
/// first line: it is drained, so the rest never reaches the shell, and refused.
fn read_hidden(events: &mut impl EventSource) -> Result<String, CliError> {
    let mut value = String::new();
    loop {
        match apply_event(events.read()?, &mut value) {
            Step::Continue => {}
            Step::Cancel => {
                return Err(CliError::new(
                    "secret.cancelled",
                    "cancelled; nothing was stored",
                    exit::ERROR,
                ));
            }
            Step::Submit if events.pending(Duration::ZERO)? => {
                while events.pending(DRAIN_QUIET)? {
                    events.read()?;
                }
                return Err(CliError::new(
                    "secret.paste_incomplete",
                    "more input followed Enter, as a multi-line paste does; nothing was stored. \
                     Pipe a multi-line value through stdin instead",
                    exit::ERROR,
                ));
            }
            Step::Submit => return Ok(value),
        }
    }
}

/// Leaves bracketed paste and raw mode however the prompt ends.
struct RawMode;

impl RawMode {
    fn enter() -> io::Result<Self> {
        terminal::enable_raw_mode()?;
        // Windows consoles deliver a paste as key events; bracketed paste there turns its markers into keys.
        #[cfg(not(windows))]
        {
            let _ = crossterm::execute!(io::stderr(), event::EnableBracketedPaste);
        }
        Ok(Self)
    }
}

impl Drop for RawMode {
    fn drop(&mut self) {
        #[cfg(not(windows))]
        {
            let _ = crossterm::execute!(io::stderr(), event::DisableBracketedPaste);
        }
        let _ = terminal::disable_raw_mode();
    }
}

/// Reads one line from the terminal in raw mode, so typed characters are never echoed.
fn prompt_hidden(prompt: &str) -> Result<String, CliError> {
    let mut stderr = io::stderr();
    write!(stderr, "{prompt}")?;
    stderr.flush()?;
    let guard = RawMode::enter()?;
    let outcome = read_hidden(&mut Terminal);
    drop(guard);
    writeln!(stderr)?;
    outcome
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::io;
    use std::time::Duration;

    use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

    use super::{EventSource, read_hidden, strip_one_newline};

    /// A scripted terminal: every event is already pending.
    struct Script(VecDeque<Event>);

    impl EventSource for Script {
        fn read(&mut self) -> io::Result<Event> {
            self.0
                .pop_front()
                .ok_or_else(|| io::Error::from(io::ErrorKind::UnexpectedEof))
        }

        fn pending(&mut self, _wait: Duration) -> io::Result<bool> {
            Ok(!self.0.is_empty())
        }
    }

    fn key(code: KeyCode, modifiers: KeyModifiers) -> Event {
        Event::Key(KeyEvent::new(code, modifiers))
    }

    fn typed(text: &str) -> Vec<Event> {
        text.chars()
            .map(|c| key(KeyCode::Char(c), KeyModifiers::NONE))
            .collect()
    }

    fn enter() -> Event {
        key(KeyCode::Enter, KeyModifiers::NONE)
    }

    fn run(events: Vec<Event>) -> Result<String, String> {
        read_hidden(&mut Script(events.into())).map_err(|error| error.code)
    }

    #[test]
    fn strips_exactly_one_trailing_newline() {
        assert_eq!(strip_one_newline("abc\n".to_owned()), "abc");
        assert_eq!(strip_one_newline("abc\r\n".to_owned()), "abc");
        assert_eq!(strip_one_newline("abc\n\n".to_owned()), "abc\n");
        assert_eq!(strip_one_newline("abc".to_owned()), "abc");
        assert_eq!(strip_one_newline("\n".to_owned()), "");
    }

    #[test]
    fn typed_keys_and_backspace_build_the_value() {
        let mut events = typed("abx");
        events.push(key(KeyCode::Backspace, KeyModifiers::NONE));
        events.extend(typed("c"));
        events.push(enter());
        assert_eq!(run(events), Ok("abc".to_owned()));
    }

    #[test]
    fn a_bracketed_paste_keeps_its_newlines() {
        let events = vec![Event::Paste("line one\nline two".to_owned()), enter()];
        assert_eq!(run(events), Ok("line one\nline two".to_owned()));
    }

    #[test]
    fn input_pending_after_enter_is_refused_and_drained() {
        let mut events = typed("first");
        events.push(enter());
        events.extend(typed("second"));
        events.push(enter());
        let mut script = Script(events.into());
        let error = read_hidden(&mut script).unwrap_err();
        assert_eq!(error.code, "secret.paste_incomplete");
        assert!(!error.message.contains("first") && !error.message.contains("second"));
        assert!(script.0.is_empty(), "the rest of the paste was drained");
    }

    #[test]
    fn control_and_alt_chords_are_not_text() {
        let mut events = typed("a");
        events.push(key(KeyCode::Char('v'), KeyModifiers::CONTROL));
        events.push(key(KeyCode::Char('x'), KeyModifiers::ALT));
        events.push(key(KeyCode::Enter, KeyModifiers::ALT));
        events.push(enter());
        assert_eq!(run(events), Ok("a".to_owned()));
    }

    #[test]
    fn control_c_d_and_escape_cancel() {
        for cancel in [
            key(KeyCode::Char('c'), KeyModifiers::CONTROL),
            key(KeyCode::Char('d'), KeyModifiers::CONTROL),
            key(KeyCode::Esc, KeyModifiers::NONE),
        ] {
            let mut events = typed("abc");
            events.push(cancel);
            assert_eq!(run(events), Err("secret.cancelled".to_owned()));
        }
    }

    #[cfg(windows)]
    #[test]
    fn altgr_characters_are_text_on_windows() {
        let altgr = KeyModifiers::CONTROL | KeyModifiers::ALT;
        let events = vec![key(KeyCode::Char('@'), altgr), enter()];
        assert_eq!(run(events), Ok("@".to_owned()));
    }
}
