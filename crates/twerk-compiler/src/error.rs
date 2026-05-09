use thiserror::Error;

pub type CompilerResult<T> = Result<T, CompilerError>;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CompilerError {
    #[error("workflow source is not valid UTF-8: {message}")]
    InvalidUtf8 { message: Box<str> },
    #[error("unexpected expression token at {line}:{column}: {fragment}")]
    Lex {
        line: usize,
        column: usize,
        fragment: Box<str>,
    },
    #[error("invalid expression at {line}:{column}: {message}")]
    Parse {
        line: usize,
        column: usize,
        message: Box<str>,
    },
}

impl CompilerError {
    #[must_use]
    pub fn invalid_utf8(error: std::str::Utf8Error) -> Self {
        Self::InvalidUtf8 {
            message: error.to_string().into_boxed_str(),
        }
    }

    #[must_use]
    pub fn lex(line: usize, column: usize, fragment: &str) -> Self {
        Self::Lex {
            line,
            column,
            fragment: fragment.into(),
        }
    }

    #[must_use]
    pub fn parse(line: usize, column: usize, message: &str) -> Self {
        Self::Parse {
            line,
            column,
            message: message.into(),
        }
    }
}
