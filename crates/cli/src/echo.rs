use std::fmt;
use std::io::{self, IsTerminal, Write};

use owo_colors::OwoColorize;

use crate::command::ColorMode;

// Terminal-palette equivalents of the docs site's teal and gold accents, with
// conventional green and red reserved for success and failure states.
pub(crate) struct Echo {
    stdout_colored: bool,
    stderr_colored: bool,
}

impl Echo {
    pub(crate) fn new(mode: ColorMode, machine: bool) -> Self {
        let color_allowed = !machine && std::env::var_os("NO_COLOR").is_none();
        let (stdout_colored, stderr_colored) = match mode {
            ColorMode::Always if color_allowed => (true, true),
            ColorMode::Auto if color_allowed => {
                (io::stdout().is_terminal(), io::stderr().is_terminal())
            }
            ColorMode::Always | ColorMode::Auto | ColorMode::Never => (false, false),
        };

        Self {
            stdout_colored,
            stderr_colored,
        }
    }

    pub(crate) fn line(&self, message: &str) {
        let _ = writeln!(io::stdout().lock(), "{message}");
    }

    pub(crate) fn raw(&self, bytes: &[u8]) {
        let _ = io::stdout().lock().write_all(bytes);
    }

    pub(crate) fn error(&self, message: &str) {
        let prefix = if self.stderr_colored {
            "error".bright_red().bold().to_string()
        } else {
            "error".to_owned()
        };
        let _ = writeln!(io::stderr().lock(), "{prefix}: {message}");
    }

    pub(crate) fn field(&self, label: &str, value: impl fmt::Display) {
        self.line(&format!("{}: {value}", self.label(label)));
    }

    pub(crate) fn label(&self, message: &str) -> String {
        if self.stdout_colored {
            message.cyan().bold().to_string()
        } else {
            message.to_owned()
        }
    }

    pub(crate) fn success(&self, message: &str) -> String {
        if self.stdout_colored {
            message.green().bold().to_string()
        } else {
            message.to_owned()
        }
    }

    pub(crate) fn path(&self, path: impl fmt::Display) -> String {
        let path = path.to_string();
        if self.stdout_colored {
            path.bright_cyan().underline().to_string()
        } else {
            path
        }
    }

    pub(crate) fn warning(&self, message: &str) -> String {
        if self.stdout_colored {
            message.yellow().bold().to_string()
        } else {
            message.to_owned()
        }
    }

    pub(crate) fn failure(&self, message: &str) -> String {
        if self.stdout_colored {
            message.bright_red().bold().to_string()
        } else {
            message.to_owned()
        }
    }
}
