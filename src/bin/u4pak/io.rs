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

#[cfg(target_os = "linux")]
pub fn transfer(in_file: &mut std::fs::File, out_file: &mut std::fs::File, size: usize) -> std::io::Result<()> {
    use std::os::unix::io::AsRawFd;

    let in_fd  = in_file.as_raw_fd();
    let out_fd = out_file.as_raw_fd();

    let mut remaining = size;
    while remaining > 0 {
        unsafe {
            let result = libc::sendfile(out_fd, in_fd, std::ptr::null_mut(), remaining as libc::size_t);

            if result < 0 {
                return Err(std::io::Error::last_os_error());
            }

            remaining -= result as usize;
        }
    }

    Ok(())
}

#[cfg(not(target_os = "linux"))]
pub fn transfer(in_file: &mut std::fs::File, out_file: &mut std::fs::File, size: usize) -> std::io::Result<()> {
    use std::io::{Read, Write};
    use u4pak::pak::BUFFER_SIZE;

    // needs to be heap allocated since Windows has small stack sizes
    let mut buf = vec![0u8; std::cmp::min(BUFFER_SIZE, size)];

    let mut remaining = size;
    while remaining >= BUFFER_SIZE {
        in_file.read_exact(&mut buf)?;
        out_file.write_all(&buf)?;
        remaining -= BUFFER_SIZE;
    }

    if remaining > 0 {
        let buf = &mut buf[..remaining];
        in_file.read_exact(buf)?;
        out_file.write_all(buf)?;
    }

    Ok(())
}
