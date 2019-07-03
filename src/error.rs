use std::fmt::{self, Display};

/// This crate's error type.
#[derive(Debug)]
pub(crate) struct Error {
    kind: ErrorKind,
}

impl Error {
    pub(crate) fn subprocess(cmd: &str) -> Self {
        Self {
            kind: ErrorKind::Subprocess {
                cmd: cmd.into(),
                msg: Message::None,
            },
        }
    }

    pub(crate) fn subprocess_with_err<E>(cmd: &str, e: E) -> Self
    where
        E: std::error::Error + 'static,
    {
        Self {
            kind: ErrorKind::Subprocess {
                cmd: cmd.into(),
                msg: Message::Error(Box::new(e)),
            },
        }
    }

    pub(crate) fn subprocess_with_str(cmd: &str, s: &str) -> Self {
        Self {
            kind: ErrorKind::Subprocess {
                cmd: cmd.into(),
                msg: Message::String(s.into()),
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
    Subprocess { cmd: String, msg: Message },
    NormalisationPregainInvalid,
}

impl Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ErrorKind::Subprocess { cmd, msg } => match msg {
                Message::None => write!(f, "Failed to execute {:?}.", cmd),
                Message::Error(ref e) => write!(f, "Failed to execute {:?}. Error: {}", cmd, e),
                Message::String(ref s) => write!(f, "Failed to execute {:?}. Error: {}", cmd, s),
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
