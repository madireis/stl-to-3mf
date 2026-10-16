//! Error types for STL parsing and serialization.

use thiserror::Error;

#[derive(Error, Debug, Clone, PartialEq)]
pub enum StlError {
    #[error("File too small: input buffer length {0} is smaller than minimum required header")]
    FileTooSmall(usize),

    #[error("Unexpected end of file while reading facet at offset {0}")]
    UnexpectedEof(usize),

    #[error("Invalid STL format: {0}")]
    InvalidFormat(String),

    #[error("Failed to parse floating-point number '{value}' at line {line}: {reason}")]
    InvalidFloat {
        value: String,
        line: usize,
        reason: String,
    },

    #[error("Expected keyword '{expected}' at line {line}, found '{found}'")]
    UnexpectedKeyword {
        expected: &'static str,
        found: String,
        line: usize,
    },

    #[error("Integer overflow calculating facet buffer size")]
    BufferOverflow,
}
