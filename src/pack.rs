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

use std::{convert::TryFrom, fs::File, io::{BufWriter, Read, Seek, SeekFrom, Write}, num::NonZeroU32, path::{Path, PathBuf}, time::UNIX_EPOCH};
use std::fs::OpenOptions;

use crypto::digest::Digest;
use crypto::sha1::{Sha1 as Sha1Hasher};
use flate2::{Compression, write::ZlibEncoder};

use crate::{Result, pak::{BUFFER_SIZE, COMPRESSION_BLOCK_HEADER_SIZE, DEFAULT_COMPRESSION_LEVEL, Encoding, V1_RECORD_HEADER_SIZE, V2_RECORD_HEADER_SIZE, V3_RECORD_HEADER_SIZE}, parse_compression_level, record::CompressionBlock, util::parse_pak_path, walkdir::walkdir};
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
    pub compression_block_size: Option<NonZeroU32>,
    pub compression_level: Option<NonZeroU32>,
    pub filename: &'a str,
    pub rename: Option<&'a str>,
}

impl<'a> PackPath<'a> {
    pub fn new(filename: &'a str) -> Self {
        Self {
            compression_method: COMPR_DEFAULT,
            compression_block_size: None,
            compression_level: None,
            filename,
            rename: None,
        }
    }

    pub fn compressed(filename: &'a str, compression_method: u32, compression_block_size: Option<NonZeroU32>, compression_level: Option<NonZeroU32>) -> Result<Self> {
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
            compression_level,
            filename,
            rename: None,
        })
    }
}

impl<'a> TryFrom<&'a str> for PackPath<'a> {
    type Error = crate::result::Error;

    fn try_from(filename: &'a str) -> std::result::Result<Self, Self::Error> {
        // :zlib,level=5,block_size=512,rename=egg/spam.txt:/foo/bar/baz.txt
        if filename.starts_with(':') {
            if let Some(index) = filename[1..].find(':') {
                let (param_str, filename) = filename.split_at(index + 2);
                let param_str = &param_str[1..param_str.len() - 1];

                let mut compression_method = COMPR_DEFAULT;
                let mut compression_block_size = None;
                let mut compression_level = None;
                let mut rename = None;

                for param in param_str.split(',') {
                    if param.eq_ignore_ascii_case("zlib") {
                        compression_method = COMPR_ZLIB;
                    } else if let Some(index) = param.find('=') {
                        let (key, value) = param.split_at(index + 1);
                        let key = &key[..key.len() - 1];

                        if key.eq_ignore_ascii_case("level") {
                            compression_level = Some(parse_compression_level(value)?);
                        } else if key.eq_ignore_ascii_case("block_size") {
                            if value.eq_ignore_ascii_case("default") {
                                compression_block_size = Some(DEFAULT_BLOCK_SIZE);
                            } else {
                                match value.parse() {
                                    Ok(block_size) if block_size > 0 => {
                                        compression_block_size = NonZeroU32::new(block_size);
                                    }
                                    _ => {
                                        return Err(Error::new(format!(
                                            "illegal path specification, illegal parameter value {:?} in: {:?}",
                                            param, filename)));
                                    }
                                }
                            }
                        } else if key.eq_ignore_ascii_case("rename") {
                            rename = Some(value);
                        } else {
                            return Err(Error::new(format!(
                                "illegal path specification, unhandeled parameter {:?} in: {:?}",
                                param, filename)));
                        }
                    } else {
                        return Err(Error::new(format!(
                            "illegal path specification, unhandeled parameter {:?} in: {:?}",
                            param, filename)));
                    }
                }

                return Ok(Self {
                    compression_block_size,
                    compression_level,
                    compression_method,
                    filename,
                    rename,
                });
            } else {
                return Err(Error::new(format!(
                    "illegal path specification, expected a second ':' in: {:?}",
                    filename)));
            }
        } else {
            return Ok(Self::new(filename));
        }
    }
}


pub struct PackOptions<'a> {
    pub version: u32,
    pub mount_point: Option<&'a str>,
    pub compression_method: u32,
    pub compression_block_size: NonZeroU32,
    pub compression_level: NonZeroU32,
    pub encoding: Encoding,
}

impl Default for PackOptions<'_> {
    fn default() -> Self {
        Self {
            version: 3,
            mount_point: None,
            compression_method: COMPR_NONE,
            compression_block_size: DEFAULT_BLOCK_SIZE,
            compression_level: DEFAULT_COMPRESSION_LEVEL,
            encoding: Encoding::default(),
        }
    }
}

pub fn pack(pak_path: impl AsRef<Path>, paths: &[PackPath], options: PackOptions) -> Result<Pak> {
    let write_record_inline = match options.version {
        1 => Record::write_v1_inline,
        2 => Record::write_v2_inline,
        3 => Record::write_v3_inline,
        _ => {
            return Err(Error::new(
                format!("unsupported version: {}", options.version)).
                with_path(pak_path));
        }
    };

    let compression_level = Compression::new(options.compression_level.get());

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
    let mut writer = BufWriter::new(&mut out_file);

    let mut hasher = Sha1Hasher::new();
    let mut buffer = vec![0u8; BUFFER_SIZE];
    let mut out_buffer = Vec::new();

    let mut records = Vec::new();
    let mut data_size = 0u64;

    let base_header_size = match options.version {
        1 => V1_RECORD_HEADER_SIZE,
        2 => V2_RECORD_HEADER_SIZE,
        3 => V3_RECORD_HEADER_SIZE,
        _ => {
            panic!("unsupported version: {}", options.version)
        }
    };

    for path in paths {
        let compression_method = if path.compression_method == COMPR_DEFAULT {
            options.compression_method
        } else {
            path.compression_method
        };

        if options.version < 2 && compression_method != COMPR_NONE {
            return Err(Error::new("Compression is only supported startig with version 2".to_string())
                .with_path(path.filename));
        }

        let source_path: PathBuf;
        let filename = if let Some(filename) = path.rename {
            source_path = path.filename.into();
            parse_pak_path(filename).collect::<Vec<_>>()
        } else {
            #[cfg(target_os = "windows")]
            let filename = path.filename
                .trim_end_matches(|ch| ch == '/' || ch == '\\')
                .split(|ch| ch == '/' || ch == '\\')
                .filter(|comp| !comp.is_empty())
                .collect::<Vec<_>>();

            #[cfg(not(target_os = "windows"))]
            let filename = path.filename
                .trim_end_matches('/')
                .split('/')
                .filter(|comp| !comp.is_empty())
                .collect::<Vec<_>>();

            source_path = filename.iter().collect();
            filename
        };

        let component_count = source_path.components().count();

        let mut handle_entry = |file_path: &Path| -> Result<()> {
            let offset = data_size;
            let compression_blocks;
            let mut compression_block_size = 0u32;
            let mut size;

            let mut in_file = match File::open(&file_path) {
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

            hasher.reset();

            match compression_method {
                self::COMPR_NONE => {
                    size = uncompressed_size;
                    compression_blocks = None;

                    writer.seek(SeekFrom::Current(base_header_size as i64))?;
                    data_size += base_header_size;

                    let mut remaining = uncompressed_size as usize;
                    {
                        // buffer might be bigger than BUFFER_SIZE if any previous
                        // compression_block_size is bigger than BUFFER_SIZE
                        let buffer = &mut buffer[..BUFFER_SIZE];
                        while remaining >= BUFFER_SIZE {
                            in_file.read_exact(buffer)?;
                            writer.write_all(buffer)?;
                            hasher.input(buffer);
                            remaining -= BUFFER_SIZE;
                        }
                    }

                    if remaining > 0 {
                        let buffer = &mut buffer[..remaining];
                        in_file.read_exact(buffer)?;
                        writer.write_all(buffer)?;
                        hasher.input(buffer);
                    }
                }
                self::COMPR_ZLIB => {
                    let compression_level = if let Some(compression_level) = path.compression_level {
                        Compression::new(compression_level.get())
                    } else {
                        compression_level
                    };
                    size = 0u64;
                    if options.version <= 2 {
                        writer.seek(SeekFrom::Current(base_header_size as i64))?;
                        data_size += base_header_size;

                        buffer.resize(uncompressed_size as usize, 0);
                        in_file.read_exact(&mut buffer)?;

                        out_buffer.clear();
                        let mut zlib = ZlibEncoder::new(&mut out_buffer, compression_level);
                        zlib.write_all(&buffer)?;
                        zlib.finish()?;
                        writer.write_all(&out_buffer)?;
                        hasher.input(&out_buffer);

                        size += out_buffer.len() as u64;

                        compression_blocks = None;
                    } else {
                        compression_block_size = path.compression_block_size
                            .unwrap_or(options.compression_block_size)
                            .get();

                        if compression_block_size as u64 > uncompressed_size {
                            compression_block_size = uncompressed_size as u32;
                        }

                        let mut header_size = base_header_size;
                        if uncompressed_size > 0 {
                            header_size += (1 + ((uncompressed_size - 1) / compression_block_size as u64)) * COMPRESSION_BLOCK_HEADER_SIZE;
                        }
                        writer.seek(SeekFrom::Current(header_size as i64))?;
                        data_size += header_size;

                        if buffer.len() < compression_block_size as usize {
                            buffer.resize(compression_block_size as usize, 0);
                        }

                        let buffer = &mut buffer[..compression_block_size as usize];
                        let mut blocks = Vec::<CompressionBlock>::new();
                        let mut remaining = uncompressed_size as usize;
                        let mut start_offset = if options.version >= 7 { offset } else { 0 };

                        while remaining >= compression_block_size as usize {
                            in_file.read_exact(buffer)?;

                            out_buffer.clear();
                            let mut zlib = ZlibEncoder::new(&mut out_buffer, compression_level);
                            zlib.write_all(&buffer)?;
                            zlib.finish()?;
                            writer.write_all(&out_buffer)?;
                            hasher.input(&out_buffer);

                            let compressed_block_size = out_buffer.len() as u64;
                            size += compressed_block_size;

                            remaining -= compression_block_size as usize;
                            let end_offset = start_offset + compressed_block_size;
                            blocks.push(CompressionBlock {
                                start_offset,
                                end_offset,
                            });
                            start_offset = end_offset;
                        }

                        if remaining > 0 {
                            let buffer = &mut buffer[..remaining];
                            in_file.read_exact(buffer)?;

                            out_buffer.clear();
                            let mut zlib = ZlibEncoder::new(&mut out_buffer, compression_level);
                            zlib.write_all(buffer)?;
                            zlib.finish()?;
                            writer.write_all(&out_buffer)?;
                            hasher.input(&out_buffer);

                            let compressed_block_size = out_buffer.len() as u64;
                            size += compressed_block_size;

                            let end_offset = start_offset + compressed_block_size;
                            blocks.push(CompressionBlock {
                                start_offset,
                                end_offset,
                            });
                        }

                        compression_blocks = Some(blocks);
                    }
                }
                _ => {
                    return Err(Error::new(
                        format!("{}: unsupported compression method: {} ({})",
                            path.filename, compression_method_name(compression_method), compression_method)).
                        with_path(pak_path))
                }
            }

            let mut sha1: Sha1 = [0u8; 20];
            hasher.result(&mut sha1);

            let mut pak_filename: Vec<String> = file_path.components()
                .skip(component_count)
                .map(|comp| comp.as_os_str().to_string_lossy().to_string())
                .collect();

            pak_filename.extend(filename.iter().map(|comp| comp.to_string()));

            records.push(Record::new(
                make_pak_path(pak_filename.iter()),
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

            Ok(())
        };

        let metadata = match source_path.metadata() {
            Ok(metadata) => metadata,
            Err(error) => return Err(Error::io_with_path(error, source_path))
        };

        if metadata.is_dir() {
            let iter = match walkdir(&source_path) {
                Ok(iter) => iter,
                Err(error) => return Err(Error::io_with_path(error, source_path))
            };
            for entry in iter {
                let entry = match entry {
                    Ok(entry) => entry,
                    Err(error) => return Err(Error::io_with_path(error, source_path))
                };
                handle_entry(&entry.path())?;
            }
        } else {
            handle_entry(&source_path)?;
        }
    }

    let index_offset = data_size;
    // FIXME: HOW IS THE FILE BIGGER THAN data_size AT THIS POINT!?
    eprintln!("index_offset: {}", index_offset);
    eprintln!("current:      {}", writer.seek(SeekFrom::Current(0))?);
    eprintln!("file size:    {}", writer.seek(SeekFrom::End(0))?);

    for record in &records {
        writer.seek(SeekFrom::Start(record.offset()))?;
        write_record_inline(record, &mut writer)?;
    }

    writer.seek(SeekFrom::Start(index_offset))?;

    let mut index_size = 0u64;

    let mount_pount = options.mount_point.unwrap_or("");

    hasher.reset();
    buffer.clear();

    write_path(&mut buffer, mount_pount, options.encoding)?;
    encode!(&mut buffer, records.len() as u32);
    writer.write_all(&buffer)?;
    hasher.input(&buffer);

    index_size += buffer.len() as u64;

    let write_record = match options.version {
        1 => Record::write_v1,
        2 => Record::write_v2,
        3 => Record::write_v3,
        _ => {
            return Err(Error::new(
                format!("unsupported version: {}", options.version)).
                with_path(pak_path));
        }
    };

    for record in &records {
        buffer.clear();
        write_path(&mut buffer, record.filename(), options.encoding)?;
        write_record(record, &mut buffer)?;

        writer.write_all(&buffer)?;
        hasher.input(&buffer);
        index_size += buffer.len() as u64;
    }

    let mut index_sha1: Sha1 = [0u8; 20];
    hasher.result(&mut index_sha1);

    encode!(&mut writer,
        PAK_MAGIC,
        options.version,
        index_offset,
        index_size,
        index_sha1,
    );
    writer.flush()?;

    Ok(Pak::new(
        options.version,
        index_offset,
        index_size,
        index_sha1,
        options.mount_point.map(str::to_string),
        records,
    ))
}

pub fn write_path(writer: &mut impl Write, path: &str, encoding: Encoding) -> Result<()> {
    match encoding {
        Encoding::UTF8 => {
            let bytes = path.as_bytes();
            writer.write_all(&bytes.len().to_le_bytes())?;
            writer.write_all(bytes)?;
        }
        Encoding::ASCII => {
            for ch in path.chars() {
                if ch > 127 as char {
                    return Err(Error::new(format!(
                        "Illegal char {:?} (0x{:x}) for ASCII codec in string: {:?}",
                        ch, ch as u32, path,
                    )));
                }
            }

            let bytes = path.as_bytes();
            writer.write_all(&bytes.len().to_le_bytes())?;
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
            writer.write_all(&bytes)?;
        }
    }
    Ok(())
}
