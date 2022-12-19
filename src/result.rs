// This file is part of rust-u4pak.
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use std::{
    io::Write,
    path::{Path, PathBuf},
};

use crossbeam_channel::SendError;

#[derive(Debug)]
pub enum ErrorType {
    IO(std::io::Error),
    Message(String),
    ChannelDisconnected,
}

impl ErrorType {
    #[inline]
    pub fn is_io(&self) -> bool {
        matches!(self, Self::IO(_))
    }

    #[inline]
    pub fn is_message(&self) -> bool {
        matches!(self, Self::Message(_))
    }

    #[inline]
    pub fn is_channel_disconnected(&self) -> bool {
        matches!(self, Self::ChannelDisconnected)
    }
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
        Self {
            path: None,
            error_type: ErrorType::IO(error),
        }
    }

    #[inline]
    pub fn io_with_path(error: std::io::Error, path: impl AsRef<Path>) -> Self {
        Self {
            path: Some(path.as_ref().to_path_buf()),
            error_type: ErrorType::IO(error),
        }
    }

    #[inline]
    pub fn channel_disconnected() -> Self {
        Self {
            path: None,
            error_type: ErrorType::ChannelDisconnected,
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

    pub fn write_to(&self, writer: &mut impl Write, null_separated: bool) -> std::io::Result<()> {
        if let Some(path) = &self.path {
            #[cfg(target_family = "unix")]
            {
                use std::os::unix::ffi::OsStrExt;
                writer.write_all(path.as_os_str().as_bytes())?;
                writer.write_all(b": ")?;
            }

            #[cfg(not(target_family = "unix"))]
            {
                write!(writer, "{}: ", path.to_string_lossy())?
            }
        }

        write!(
            writer,
            "{}{}",
            self.error_type,
            if null_separated { '\0' } else { '\n' }
        )
    }
}

impl std::fmt::Display for ErrorType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ErrorType::IO(err) => err.fmt(f),
            ErrorType::Message(msg) => msg.fmt(f),
            ErrorType::ChannelDisconnected => write!(f, "sending on a disconnected channel"),
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

impl From<std::string::FromUtf16Error> for Error {
    fn from(error: std::string::FromUtf16Error) -> Self {
        Error::new(format!("UTF-16 conversion error: {}", error))
    }
}

impl From<clap::Error> for Error {
    fn from(error: clap::Error) -> Self {
        Error::new(error.to_string())
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

impl From<std::time::SystemTimeError> for Error {
    fn from(error: std::time::SystemTimeError) -> Self {
        Error::new(error.to_string())
    }
}

impl From<flate2::DecompressError> for Error {
    fn from(error: flate2::DecompressError) -> Self {
        Error::new(error.to_string())
    }
}

impl<T: Sized> From<SendError<Result<T>>> for Error {
    fn from(_error: SendError<Result<T>>) -> Self {
        Error::channel_disconnected()
    }
}

pub type Result<T> = core::result::Result<T, Error>;
