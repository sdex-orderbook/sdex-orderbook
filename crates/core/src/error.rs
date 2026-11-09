// SPDX-License-Identifier: Apache-2.0
//! Error type shared by the crate. `code` values are stable (SPEC section 7).

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Code {
    /// Bad query parameter (HTTP 400).
    BadRequest,
    /// Asset or object not known (HTTP 404).
    NotFound,
    /// Network fetch failed (archive, lake, Horizon).
    Fetch,
    /// XDR, gzip or zstd decode failure.
    Decode,
    /// Ledger sequence gap or out-of-order apply.
    Sequence,
    /// A STATE pre-image did not match the reconstructed state.
    Consistency,
    /// Local file I/O.
    Io,
    /// Arithmetic error in a path or pool computation.
    Arithmetic,
}

impl Code {
    pub fn as_str(self) -> &'static str {
        match self {
            Code::BadRequest => "E_BAD_REQUEST",
            Code::NotFound => "E_NOT_FOUND",
            Code::Fetch => "E_FETCH",
            Code::Decode => "E_DECODE",
            Code::Sequence => "E_SEQUENCE",
            Code::Consistency => "E_CONSISTENCY",
            Code::Io => "E_IO",
            Code::Arithmetic => "E_ARITHMETIC",
        }
    }

    pub fn http_status(self) -> u16 {
        match self {
            Code::BadRequest => 400,
            Code::NotFound => 404,
            _ => 500,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Error {
    pub code: Code,
    pub message: String,
}

impl Error {
    pub fn new(code: Code, message: impl Into<String>) -> Error {
        Error { code, message: message.into() }
    }
    pub fn bad(message: impl Into<String>) -> Error {
        Error::new(Code::BadRequest, message)
    }
    pub fn io(path: &std::path::Path, e: std::io::Error) -> Error {
        Error::new(Code::Io, format!("{}: {e}", path.display()))
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code.as_str(), self.message)
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;
