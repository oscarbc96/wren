use std::fmt;
use std::path::PathBuf;

/// A settings problem, always attributed to a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettingsError {
    /// The text is not valid JSONC. `line` and `col` are 1-based.
    Parse {
        path: PathBuf,
        line: usize,
        col: usize,
        message: String,
    },
    /// The text parsed but a value has the wrong type or is not allowed.
    /// `pointer` is a JSON pointer such as `/ui_font_size`.
    Type {
        path: PathBuf,
        pointer: String,
        message: String,
    },
}

impl fmt::Display for SettingsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse {
                path,
                line,
                col,
                message,
            } => write!(f, "{}:{line}:{col}: {message}", path.display()),
            Self::Type {
                path,
                pointer,
                message,
            } => write!(f, "{}: {pointer}: {message}", path.display()),
        }
    }
}

impl std::error::Error for SettingsError {}
