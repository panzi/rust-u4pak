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

use std::path::Path;
use std::fs::OpenOptions;

use crate::{Result, util::parse_pak_path};
use crate::Pak;
use crate::result::Error;
use crate::pak::{COMPR_NONE, COMPR_ZLIB, DEFAULT_BLOCK_SIZE, compression_method_name};
use crate::record::Record;

pub const COMPR_DEFAULT: u32 = u32::MAX;

pub struct PackPath<'a> {
    pub compression_method: u32,
    pub compression_block_size: u32,
    pub path: &'a str,
}

impl<'a> PackPath<'a> {
    pub fn new(path: &'a str) -> Self {
        Self {
            compression_method: COMPR_DEFAULT,
            compression_block_size: 0,
            path,
        }
    }

    pub fn compressed(path: &'a str, compression_method: u32, compression_block_size: u32) -> Result<Self> {
        match compression_method {
            self::COMPR_NONE | self::COMPR_ZLIB | self::COMPR_DEFAULT => {}
            _ => return Err(Error::new(
                format!("unsupported compression method: {} ({})",
                    compression_method_name(compression_method), compression_method)).
                with_path(path))
        }

        Ok(Self {
            compression_method,
            compression_block_size,
            path,
        })
    }

}

pub struct PackOptions<'a> {
    pub version: u32,
    pub mount_point: Option<&'a str>,
    pub compression_method: u32,
    pub compression_block_size: u32,
}

impl Default for PackOptions<'_> {
    fn default() -> Self {
        Self {
            version: 3,
            mount_point: None,
            compression_method: COMPR_NONE,
            compression_block_size: DEFAULT_BLOCK_SIZE,
        }
    }
}

pub fn pack(pak_path: impl AsRef<Path>, paths: &[PackPath], options: PackOptions) -> Result<Pak> {
    match options.version {
        1 | 2 | 3 => {}
        _ => return Err(Error::new(
            format!("unsupported version: {}", options.version)).
            with_path(pak_path))
    }

    match options.compression_method {
        self::COMPR_NONE | self::COMPR_ZLIB => {}
        _ => return Err(Error::new(
            format!("unsupported compression method: {} ({})",
                compression_method_name(options.compression_method), options.compression_method)).
            with_path(pak_path))
    }

    let pak_path = pak_path.as_ref();
    let out_file = match OpenOptions::new()
        .create(true)
        .write(true)
        .open(pak_path) {
            Ok(file) => file,
            Err(error) => return Err(Error::io_with_path(error, pak_path))
        };

    //let mut records = Vec::new();
    let mut data_size = 0usize;

    for path in paths {
        let compr_method = if path.compression_method == COMPR_DEFAULT {
            options.compression_method
        } else {
            path.compression_method
        };

        let parsed_path = parse_pak_path(path.path).collect::<Vec<_>>();

        match compr_method {
            self::COMPR_NONE => {
                // TODO
            }
            self::COMPR_ZLIB => {
                // TODO
                let block_size = if path.compression_block_size == 0 {
                    options.compression_block_size
                } else {
                    path.compression_block_size
                };
            }
            _ => {
                return Err(Error::new(
                    format!("{}: unsupported compression method: {} ({})",
                        path.path, compression_method_name(compr_method), compr_method)).
                    with_path(pak_path))
            }
        }
    }

    panic!("not implemented");
}
