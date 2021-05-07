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
    } else if size as usize > buf.len() {
        buf.resize(size as usize + 1, 0);
        let size = unsafe { windows::GetFinalPathNameByHandleW(hFile, buf.as_mut_ptr(), buf.len() as windows::DWORD, windows::FILE_NAME_NORMALIZED) };

        if size == 0 {
            return Err(std::io::Error::last_os_error());
        } else if size as usize > buf.len() {
            return Err(std::io::Error::from_raw_os_error(windows::ERROR_NOT_ENOUGH_MEMORY));
        } else {
            buf.truncate(size as usize + 1);
        }
    } else {
        buf.truncate(size as usize + 1);
    }

    if let Some(index) = buf.iter().position(|&ch| ch == 0) {
        buf.truncate(index);
    }

    let path = OsString::from_wide(&buf[..]);
    let path = PathBuf::from(path);

    Ok(path)
}
