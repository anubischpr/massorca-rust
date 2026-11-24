//! An in-memory user PATH for tests, so no test reads or writes the real registry.

use std::cell::RefCell;
use std::rc::Rc;

use super::user_path::{PathValue, UNREADABLE, UserPath};
use crate::error::{CliError, exit};

/// Clones share one state, so a test keeps a handle while the environment owns another.
#[derive(Debug, Clone)]
pub struct FakeUserPath {
    state: Rc<RefCell<FakeState>>,
}

#[derive(Debug)]
struct FakeState {
    value: PathValue,
    writes: u32,
    unreadable: bool,
}

impl FakeUserPath {
    pub fn with_text(text: &str) -> Self {
        Self {
            state: Rc::new(RefCell::new(FakeState {
                value: PathValue {
                    text: text.to_owned(),
                    expandable: true,
                },
                writes: 0,
                unreadable: false,
            })),
        }
    }

    pub fn unreadable() -> Self {
        let fake = Self::with_text("");
        fake.state.borrow_mut().unreadable = true;
        fake
    }

    pub fn text(&self) -> String {
        self.state.borrow().value.text.clone()
    }

    pub fn set_text(&self, text: &str) {
        self.state.borrow_mut().value.text = text.to_owned();
    }

    pub fn writes(&self) -> u32 {
        self.state.borrow().writes
    }
}

impl UserPath for FakeUserPath {
    fn read(&self) -> Result<PathValue, CliError> {
        let state = self.state.borrow();
        if state.unreadable {
            return Err(CliError::new(
                UNREADABLE,
                "the fake user PATH cannot be read",
                exit::ERROR,
            ));
        }
        Ok(state.value.clone())
    }

    fn write(&self, value: &PathValue) -> Result<(), CliError> {
        let mut state = self.state.borrow_mut();
        state.value = value.clone();
        state.writes += 1;
        Ok(())
    }
}
