//! Reading a secret value: all of stdin when it is not a terminal, otherwise a prompt that does not echo. Never an
//! argument or an environment variable, which process listings and shell history keep.

use std::io::{self, IsTerminal, Read, Write};

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::terminal;

use crate::error::{CliError, exit};

/// The value for `name` in `scope`; refuses an empty one so nothing reaches the service.
pub fn read_value(scope: &str, name: &str) -> Result<String, CliError> {
    let stdin = io::stdin();
    let value = if stdin.is_terminal() {
        prompt_hidden(&format!("Value for {name} in {scope}: "))?
    } else {
        let mut text = String::new();
        stdin.lock().read_to_string(&mut text)?;
        strip_one_newline(text)
    };
    if value.is_empty() {
        return Err(CliError::new(
            "secret.empty_value",
            "the value is empty; nothing was stored",
            exit::ERROR,
        ));
    }
    Ok(value)
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

/// Leaves raw mode however the prompt ends.
struct RawMode;

impl Drop for RawMode {
    fn drop(&mut self) {
        let _ = terminal::disable_raw_mode();
    }
}

/// Reads one line from the terminal in raw mode, so typed characters are never echoed.
fn prompt_hidden(prompt: &str) -> Result<String, CliError> {
    let mut stderr = io::stderr();
    write!(stderr, "{prompt}")?;
    stderr.flush()?;
    terminal::enable_raw_mode()?;
    let guard = RawMode;
    let mut value = String::new();
    let outcome = loop {
        match event::read()? {
            Event::Paste(text) => value.push_str(&text),
            Event::Key(key) if key.kind != KeyEventKind::Release => match key.code {
                KeyCode::Enter => break Ok(()),
                KeyCode::Esc => break Err(()),
                KeyCode::Char('c' | 'd') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    break Err(());
                }
                KeyCode::Char(c) => value.push(c),
                KeyCode::Backspace => {
                    value.pop();
                }
                _ => {}
            },
            _ => {}
        }
    };
    drop(guard);
    writeln!(stderr)?;
    match outcome {
        Ok(()) => Ok(value),
        Err(()) => Err(CliError::new(
            "secret.cancelled",
            "cancelled; nothing was stored",
            exit::ERROR,
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::strip_one_newline;

    #[test]
    fn strips_exactly_one_trailing_newline() {
        assert_eq!(strip_one_newline("abc\n".to_owned()), "abc");
        assert_eq!(strip_one_newline("abc\r\n".to_owned()), "abc");
        assert_eq!(strip_one_newline("abc\n\n".to_owned()), "abc\n");
        assert_eq!(strip_one_newline("abc".to_owned()), "abc");
        assert_eq!(strip_one_newline("\n".to_owned()), "");
    }
}
