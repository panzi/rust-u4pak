// This file is part of rust-u4pak.
//
// rust-u4pak is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// rust-u4pak is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with rust-u4pak.  If not, see <https://www.gnu.org/licenses/>.

use std::path::{PathBuf, Path};

#[derive(Debug)]
pub enum ErrorType {
    IO(std::io::Error),
    EntryNotADir(String),
    NoSuchEntry(String),
    Message(String),
}

#[derive(Debug)]
pub struct Error {
    pub(crate) path: Option<PathBuf>,
    pub(crate) error_type: ErrorType,
}

impl Error {
    #[inline]
    pub fn new(message: String) -> Self {
        Self {
            path: None,
            error_type: ErrorType::Message(message),
        }
    }

    #[inline]
    pub fn io(error: std::io::Error) -> Self {
        Error {
            path:       None,
            error_type: ErrorType::IO(error),
        }
    }

    #[inline]
    pub fn io_with_path(error: std::io::Error, path: impl AsRef<Path>) -> Self {
        Error {
            path:       Some(path.as_ref().to_path_buf()),
            error_type: ErrorType::IO(error),
        }
    }

    #[inline]
    pub fn entry_not_a_dir(path: impl AsRef<str>) -> Self {
        Error {
            path:       None,
            error_type: ErrorType::EntryNotADir(path.as_ref().to_owned()),
        }
    }

    #[inline]
    pub fn no_such_entry(path: impl AsRef<str>) -> Self {
        Error {
            path:       None,
            error_type: ErrorType::NoSuchEntry(path.as_ref().to_owned()),
        }
    }

    #[inline]
    pub fn error_type(&self) -> &ErrorType {
        &self.error_type
    }

    #[inline]
    pub fn path(&self) -> &Option<PathBuf> {
        &self.path
    }

    #[inline]
    pub fn with_path(self, path: impl AsRef<Path>) -> Self {
        Self {
            path: Some(path.as_ref().to_path_buf()),
            error_type: self.error_type,
        }
    }

    #[inline]
    pub fn with_path_if_none(self, path: impl AsRef<Path>) -> Self {
        if self.path.is_some() {
            return self;
        }
        Self {
            path: Some(path.as_ref().to_path_buf()),
            error_type: self.error_type,
        }
    }
}

impl std::fmt::Display for ErrorType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ErrorType::IO(err)             => err.fmt(f),
            ErrorType::EntryNotADir(path) => write!(f, "entry is not a directory: {:?}", path),
            ErrorType::NoSuchEntry(path)  => write!(f, "entry not found: {:?}", path),
            ErrorType::Message(msg)       => msg.fmt(f),
        }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(path) = &self.path {
            write!(f, "{:?}: {}", path, self.error_type)
        } else {
            self.error_type.fmt(f)
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Error {
            path: None,
            error_type: ErrorType::IO(error),
        }
    }
}

impl From<std::string::FromUtf8Error> for Error {
    fn from(error: std::string::FromUtf8Error) -> Self {
        Error::new(format!("UTF-8 conversion error: {}", error))
    }
}

impl From<std::str::Utf8Error> for Error {
    fn from(error: std::str::Utf8Error) -> Self {
        Error::new(format!("UTF-8 conversion error: {}", error))
    }
}

impl From<clap::Error> for Error {
    fn from(error: clap::Error) -> Self {
        Error::new(error.message)
    }
}

impl From<std::array::TryFromSliceError> for Error {
    fn from(error: std::array::TryFromSliceError) -> Self {
        Error::new(error.to_string())
    }
}

impl From<std::num::ParseIntError> for Error {
    fn from(error: std::num::ParseIntError) -> Self {
        Error::new(error.to_string())
    }
}

pub type Result<T> = core::result::Result<T, Error>;
