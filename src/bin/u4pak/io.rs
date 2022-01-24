// This file is part of rust-u4pak.
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

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
