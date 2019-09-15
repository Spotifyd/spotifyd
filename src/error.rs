use std::{
    error::Error as StdError,
    fmt::{self, Display},
};

#[derive(Clone, Debug)]
pub struct ParseError(String);

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "failed to parse config entry: {}", self.0)
    }
}

impl StdError for ParseError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        None
    }
}

/// This crate's error type.
#[derive(Debug)]
pub(crate) struct Error {
    kind: ErrorKind,
}

impl Error {
    pub(crate) fn subprocess(shell: &str, cmd: &str) -> Self {
        Self {
            kind: ErrorKind::Subprocess {
                cmd: cmd.into(),
                msg: Message::None,
                shell: shell.into(),
            },
        }
    }

    pub(crate) fn subprocess_with_err<E>(shell: &str, cmd: &str, e: E) -> Self
    where
        E: std::error::Error + 'static,
    {
        Self {
            kind: ErrorKind::Subprocess {
                cmd: cmd.into(),
                msg: Message::Error(Box::new(e)),
                shell: shell.into(),
            },
        }
    }

    pub(crate) fn subprocess_with_str(shell: &str, cmd: &str, s: &str) -> Self {
        Self {
            kind: ErrorKind::Subprocess {
                cmd: cmd.into(),
                msg: Message::String(s.into()),
                shell: shell.into(),
            },
        }
    }
}

impl From<ErrorKind> for Error {
    fn from(kind: ErrorKind) -> Self {
        Self { kind }
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.kind)
    }
}

impl std::error::Error for Error {}

/// This crate's error kind type.
#[derive(Debug)]
pub(crate) enum ErrorKind {
    Subprocess {
        cmd: String,
        msg: Message,
        shell: String,
    },
    #[allow(unused)]
    NormalisationPregainInvalid,
}

impl Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ErrorKind::Subprocess { cmd, msg, shell } => match msg {
                Message::None => write!(f, "Failed to execute {:?} using {:?}.", cmd, shell),
                Message::Error(ref e) => write!(
                    f,
                    "Failed to execute {:?} using {:?}. Error: {}",
                    cmd, shell, e
                ),
                Message::String(ref s) => write!(
                    f,
                    "Failed to execute {:?} using {:?}. Error: {}",
                    cmd, shell, s
                ),
            },
            ErrorKind::NormalisationPregainInvalid => write!(
                f,
                "normalisation-pregain must be a valid 32-bit floating point number."
            ),
        }
    }
}

#[derive(Debug)]
pub(crate) enum Message {
    None,
    String(String),
    Error(Box<dyn std::error::Error + 'static>),
}
