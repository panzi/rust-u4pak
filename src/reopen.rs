// This file is part of rust-u4pak.
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use std::{fs::{File, OpenOptions}, path::PathBuf};

#[allow(unused)]
#[allow(non_camel_case_types)]
#[allow(non_snake_case)]
#[cfg(target_family="windows")]
mod windows {
    use std::os::windows::io::RawHandle;

    pub(crate) type WCHAR = u16;
    pub(crate) type DWORD = u32;

    pub(crate) const FILE_NAME_NORMALIZED: DWORD = 0x0;
    pub(crate) const FILE_NAME_OPENED: DWORD = 0x8;
    pub(crate) const MAX_PATH: DWORD = 260;

    pub(crate) const ERROR_NOT_ENOUGH_MEMORY: i32 = 8;

    #[link(name = "user32")]
    extern "stdcall" {
        pub(crate) fn GetFinalPathNameByHandleW(hFile: RawHandle, lpszFilePath: *mut WCHAR, cchFilePath: DWORD, dwFlags: DWORD) -> DWORD;
    }
}

pub trait Reopen: Sized {
    fn reopen(&self) -> std::io::Result<Self>;
    fn path(&self) -> std::io::Result<PathBuf>;
}

pub trait ReopenOptions {
    type File;

    fn reopen(&self, file: &Self::File) -> std::io::Result<Self::File>;
}

impl Reopen for File {
    #[inline]
    fn reopen(&self) -> std::io::Result<Self> {
        let path = get_file_path(self)?;
        File::open(path)
    }

    #[inline]
    fn path(&self) -> std::io::Result<PathBuf> {
        get_file_path(self)
    }
}

impl ReopenOptions for OpenOptions {
    type File = std::fs::File;

    #[inline]
    fn reopen(&self, file: &Self::File) -> std::io::Result<Self::File> {
        let path = get_file_path(file)?;
        self.open(path)
    }
}

#[cfg(target_family="unix")]
pub fn get_file_path(file: &File) -> std::io::Result<PathBuf> {
    use std::os::unix::io::AsRawFd;

    let fd = file.as_raw_fd();

    #[cfg(target_os="linux")]
    let path = PathBuf::from(format!("/proc/self/fd/{}", fd));

    #[cfg(not(target_os="linux"))]
    let path = PathBuf::from(format!("/dev/fd/{}", fd));

    Ok(path)
}

#[allow(non_camel_case_types)]
#[allow(non_snake_case)]
#[cfg(target_family="windows")]
pub fn get_file_path(file: &File) -> std::io::Result<PathBuf> {
    use std::os::windows::io::AsRawHandle;
    use std::os::windows::ffi::OsStringExt;
    use std::ffi::OsString;

    let hFile = file.as_raw_handle();
    let mut buf = vec![0u16; windows::MAX_PATH as usize];
    let size = unsafe { windows::GetFinalPathNameByHandleW(hFile, buf.as_mut_ptr(), buf.len() as windows::DWORD, windows::FILE_NAME_NORMALIZED) };

    if size == 0 {
        return Err(std::io::Error::last_os_error());
    } else if size as usize >= buf.len() {
        buf.resize(size as usize + 1, 0);
        let size = unsafe { windows::GetFinalPathNameByHandleW(hFile, buf.as_mut_ptr(), buf.len() as windows::DWORD, windows::FILE_NAME_NORMALIZED) };

        if size == 0 {
            return Err(std::io::Error::last_os_error());
        } else if size as usize >= buf.len() {
            return Err(std::io::Error::from_raw_os_error(windows::ERROR_NOT_ENOUGH_MEMORY));
        } else {
            buf.truncate(size as usize);
        }
    } else {
        buf.truncate(size as usize);
    }

    let path = OsString::from_wide(&buf[..]);
    let path = PathBuf::from(path);

    Ok(path)
}
