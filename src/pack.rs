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

use std::{fs::File, io::{BufWriter, Write}, path::{Path, PathBuf}, time::UNIX_EPOCH};
use std::fs::OpenOptions;

use crate::{Result, pak::Encoding, record::CompressionBlock, util::parse_pak_path};
use crate::Pak;
use crate::result::Error;
use crate::pak::{PAK_MAGIC, Sha1, COMPR_NONE, COMPR_ZLIB, DEFAULT_BLOCK_SIZE, compression_method_name};
use crate::record::Record;
use crate::util::make_pak_path;
use crate::encode;
use crate::encode::Encode;

pub const COMPR_DEFAULT: u32 = u32::MAX;

pub struct PackPath<'a> {
    pub compression_method: u32,
    pub compression_block_size: u32,
    pub filename: &'a str,
}

impl<'a> PackPath<'a> {
    pub fn new(filename: &'a str) -> Self {
        Self {
            compression_method: COMPR_DEFAULT,
            compression_block_size: 0,
            filename,
        }
    }

    pub fn compressed(filename: &'a str, compression_method: u32, compression_block_size: u32) -> Result<Self> {
        match compression_method {
            self::COMPR_NONE | self::COMPR_ZLIB | self::COMPR_DEFAULT => {}
            _ => return Err(Error::new(
                format!("unsupported compression method: {} ({})",
                    compression_method_name(compression_method), compression_method)).
                with_path(filename))
        }

        Ok(Self {
            compression_method,
            compression_block_size,
            filename,
        })
    }

}

pub struct PackOptions<'a> {
    pub version: u32,
    pub mount_point: Option<&'a str>,
    pub compression_method: u32,
    pub compression_block_size: u32,
    pub encoding: Encoding,
}

impl Default for PackOptions<'_> {
    fn default() -> Self {
        Self {
            version: 3,
            mount_point: None,
            compression_method: COMPR_NONE,
            compression_block_size: DEFAULT_BLOCK_SIZE,
            encoding: Encoding::default(),
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
    let mut out_file = match OpenOptions::new()
        .create(true)
        .write(true)
        .open(pak_path) {
            Ok(file) => file,
            Err(error) => return Err(Error::io_with_path(error, pak_path))
        };

    let mut records = Vec::new();
    let mut data_size = 0u64;

    for path in paths {
        let offset = data_size;
        let compression_method = if path.compression_method == COMPR_DEFAULT {
            options.compression_method
        } else {
            path.compression_method
        };

        let filename = parse_pak_path(path.filename).collect::<Vec<_>>();
        let compression_blocks;
        let mut compression_block_size = 0u32;
        let mut size = 0u64; // TODO
        let mut sha1: Sha1 = [0u8; 20]; // TODO

        let file_path: PathBuf = filename.iter().collect();
        let in_file = match File::open(&file_path) {
                Ok(file) => file,
                Err(error) => return Err(Error::io_with_path(error, file_path))
        };

        let metadata = match in_file.metadata() {
            Ok(metadata) => metadata,
            Err(error) => return Err(Error::io_with_path(error, file_path))
        };

        let uncompressed_size = metadata.len();

        let timestamp = if options.version == 1 {
            let created = match metadata.created() {
                Ok(created) => created,
                Err(error) => return Err(Error::io_with_path(error, file_path))
            };
            let timestamp = match created.duration_since(UNIX_EPOCH) {
                Ok(timestamp) => timestamp,
                Err(error) =>
                    return Err(Error::new(error.to_string()).with_path(file_path))
            };
            Some(timestamp.as_secs())
        } else {
            None
        };

        match compression_method {
            self::COMPR_NONE => {
                // TODO
                size = uncompressed_size;
                compression_blocks = None;
            }
            self::COMPR_ZLIB => {
                // TODO
                compression_block_size = if path.compression_block_size == 0 {
                    options.compression_block_size
                } else {
                    path.compression_block_size
                };

                let mut blocks = Vec::<CompressionBlock>::new();
                compression_blocks = Some(blocks);
            }
            _ => {
                return Err(Error::new(
                    format!("{}: unsupported compression method: {} ({})",
                        path.filename, compression_method_name(compression_method), compression_method)).
                    with_path(pak_path))
            }
        }

        records.push(Record::new(
            make_pak_path(filename.iter()),
            offset,
            size,
            uncompressed_size,
            compression_method,
            timestamp,
            sha1,
            compression_blocks,
            false,
            compression_block_size,
        ));

        data_size += size;
    }

    let index_offset = data_size;

    let mut writer = BufWriter::new(&mut out_file);
    let mut index_size = 0u64;

    let mount_pount = if let Some(mount_point) = options.mount_point {
        mount_point
    } else {
        ""
    };

    index_size += write_path(&mut writer, mount_pount, options.encoding)?;

    match options.version {
        1 => {
            for record in &records {
                index_size += write_path(&mut writer, record.filename(), options.encoding)?;
                index_size += record.write_v1(&mut writer)?; // TODO: index_sha1
            }
        }
        2 => {
            for record in &records {
                index_size += write_path(&mut writer, record.filename(), options.encoding)?;
                index_size += record.write_v2(&mut writer)?; // TODO: index_sha1
            }
        }
        3 => {
            for record in &records {
                index_size += write_path(&mut writer, record.filename(), options.encoding)?;
                index_size += record.write_v3(&mut writer)?; // TODO: index_sha1
            }
        }
        _ => {
            return Err(Error::new(
                format!("unsupported version: {}", options.version)).
                with_path(pak_path));
        }
    }

    let index_sha1: Sha1 = [0u8; 20]; // TODO

    encode!(&mut writer,
        PAK_MAGIC,
        options.version,
        index_offset,
        index_size,
        index_sha1,
    );

    Ok(Pak::new(
        options.version,
        index_offset,
        index_size,
        index_sha1,
        if let Some(mount_point) = options.mount_point {
            Some(mount_point.to_string())
        } else {
            None
        },
        records,
    ))
}

pub fn write_path(writer: &mut impl Write, path: &str, encoding: Encoding) -> Result<u64> {
    let mut size = 4u64;
    match encoding {
        Encoding::UTF8 => {
            let bytes = path.as_bytes();
            writer.write_all(&bytes.len().to_le_bytes())?;

            size += bytes.len() as u64;
            writer.write_all(bytes)?;
        }
        Encoding::ASCII => {
            let bytes = path.as_bytes();
            for &byte in bytes {
                if byte > 127 {
                    return Err(Error::new(format!(
                        "Illegal byte 0x{:02x} ({}) for ASCII codec in string: {:?}",
                        byte, byte, path,
                    )));
                }
            }
            writer.write_all(&bytes.len().to_le_bytes())?;

            size += bytes.len() as u64;
            writer.write_all(bytes)?;
        }
        Encoding::Latin1 => {
            for ch in path.chars() {
                if ch > 255 as char {
                    return Err(Error::new(format!(
                        "Illegal char {:?} (0x{:x}) for Latin1 codec in string: {:?}",
                        ch, ch as u32, path,
                    )));
                }
            }

            let bytes: Vec<_> = path.chars().map(|ch| ch as u8).collect();

            writer.write_all(&bytes.len().to_le_bytes())?;

            size += bytes.len() as u64;
            writer.write_all(&bytes)?;
        }
    }
    Ok(size)
}
