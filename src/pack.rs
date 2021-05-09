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

use std::{collections::HashMap, convert::{TryFrom, TryInto}, io::{BufRead, BufReader, BufWriter, Read, Seek, SeekFrom, Write}, num::{NonZeroU32, NonZeroUsize}, path::{Path, PathBuf}, time::UNIX_EPOCH};
use std::fs::{OpenOptions, File};

use crossbeam_channel::{Receiver, Sender, unbounded};
use crossbeam_utils::thread;
use openssl::sha::Sha1 as OpenSSLSha1;
use flate2::{Compression, write::ZlibEncoder};

use crate::{Result, pak::{BUFFER_SIZE, COMPRESSION_BLOCK_HEADER_SIZE, DEFAULT_COMPRESSION_LEVEL, Encoding, V1_RECORD_HEADER_SIZE, V2_RECORD_HEADER_SIZE, V3_RECORD_HEADER_SIZE}, parse_compression_level, record::CompressionBlock, util::{parse_pak_path, parse_size}, walkdir::walkdir};
use crate::Pak;
use crate::result::Error;
use crate::pak::{PAK_MAGIC, Sha1, COMPR_NONE, COMPR_ZLIB, DEFAULT_BLOCK_SIZE, compression_method_name};
use crate::record::Record;
use crate::util::make_pak_path;
use crate::encode;
use crate::encode::Encode;

pub const COMPR_DEFAULT: u32 = u32::MAX;

#[derive(Debug, Clone)]
pub struct PackPath {
    pub compression_method: u32,
    pub compression_block_size: Option<NonZeroU32>,
    pub compression_level: Option<NonZeroU32>,
    pub filename: String,
    pub rename: Option<String>,
}

impl PackPath {
    pub fn new(filename: String) -> Self {
        Self {
            compression_method: COMPR_DEFAULT,
            compression_block_size: None,
            compression_level: None,
            filename,
            rename: None,
        }
    }

    #[inline]
    pub fn read_from_path(path: impl AsRef<Path>) -> Result<Vec<PackPath>> {
        match File::open(&path) {
            Ok(mut file) => match Self::read_from_file(&mut file) {
                Ok(res) => Ok(res),
                Err(error) => Err(error.with_path_if_none(path))
            },
            Err(error) => Err(Error::io_with_path(error, path))
        }
    }

    #[inline]
    pub fn read_from_file(file: &mut File) -> Result<Vec<PackPath>> {
        Self::read_from_reader(BufReader::new(file))
    }

    #[inline]
    pub fn read_from_reader(mut reader: impl BufRead) -> Result<Vec<PackPath>> {
        let mut paths = Vec::new();
        let mut lineno = 1usize;
        loop {
            let mut line = String::new();
            let count = reader.read_line(&mut line)?;

            if count == 0 {
                break;
            }

            let line = line.trim();

            if !line.starts_with('#') {
                let path = match line.try_into() {
                    Ok(path) => path,
                    Err(error) =>
                        return Err(Error::new(format!(":{}: {}", lineno, error)))
                };

                paths.push(path);
            }

            lineno += 1;
        }
        Ok(paths)
    }
}

impl TryFrom<&str> for PackPath {
    type Error = crate::result::Error;

    fn try_from(path_spec: &str) -> std::result::Result<Self, Self::Error> {
        // :zlib,level=5,block_size=512,rename=egg/spam.txt:/foo/bar/baz.txt
        if path_spec.starts_with(':') {
            if let Some(index) = path_spec[1..].find(':') {
                let (param_str, filename) = path_spec.split_at(index + 2);
                let param_str = &param_str[1..param_str.len() - 1];

                let mut compression_method = COMPR_DEFAULT;
                let mut compression_block_size = None;
                let mut compression_level = None;
                let mut rename = None;

                for param in param_str.split(',') {
                    if param.eq_ignore_ascii_case("zlib") {
                        compression_method = COMPR_ZLIB;
                    } else if param.eq_ignore_ascii_case("none") {
                        compression_method = COMPR_NONE;
                    } else if let Some(index) = param.find('=') {
                        let (key, value) = param.split_at(index + 1);
                        let key = &key[..key.len() - 1];

                        if key.eq_ignore_ascii_case("level") {
                            compression_level = Some(parse_compression_level(value)?);
                        } else if key.eq_ignore_ascii_case("block_size") {
                            if value.eq_ignore_ascii_case("default") {
                                compression_block_size = Some(DEFAULT_BLOCK_SIZE);
                            } else {
                                match parse_size(value) {
                                    Ok(block_size) if block_size > 0 && block_size <= u32::MAX as usize => {
                                        compression_block_size = NonZeroU32::new(block_size as u32);
                                    }
                                    _ => {
                                        return Err(Error::new(format!(
                                            "illegal path specification, illegal parameter value {:?} in: {:?}",
                                            param, path_spec)));
                                    }
                                }
                            }
                        } else if key.eq_ignore_ascii_case("rename") {
                            rename = Some(value.to_string());
                        } else {
                            return Err(Error::new(format!(
                                "illegal path specification, unhandeled parameter {:?} in: {:?}",
                                param, path_spec)));
                        }
                    } else {
                        return Err(Error::new(format!(
                            "illegal path specification, unhandeled parameter {:?} in: {:?}",
                            param, path_spec)));
                    }
                }

                return Ok(Self {
                    compression_block_size,
                    compression_level,
                    compression_method,
                    filename: filename.to_string(),
                    rename,
                });
            } else {
                return Err(Error::new(format!(
                    "illegal path specification, expected a second ':' in: {:?}",
                    path_spec)));
            }
        } else {
            return Ok(Self::new(path_spec.to_string()));
        }
    }
}

#[derive(Debug)]
pub struct PackOptions<'a> {
    pub version: u32,
    pub mount_point: Option<&'a str>,
    pub compression_method: u32,
    pub compression_block_size: NonZeroU32,
    pub compression_level: NonZeroU32,
    pub encoding: Encoding,
    pub verbose: bool,
    pub null_separated: bool,
    pub thread_count: NonZeroUsize,
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
            verbose: false,
            null_separated: false,
            thread_count: NonZeroUsize::new(num_cpus::get()).unwrap_or(NonZeroUsize::new(1).unwrap()),
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
        .truncate(true)
        .open(pak_path) {
            Ok(file) => file,
            Err(error) => return Err(Error::io_with_path(error, pak_path))
        };

    let mut records = Vec::new();
    let mut buffer = Vec::with_capacity(BUFFER_SIZE);
    let mut writer = BufWriter::new(&mut out_file);

    let mut data_size = 0u64;

    let thread_result = thread::scope::<_, Result<()>>(|scope| {
        let mut filenames = HashMap::new();
        let (work_sender, work_receiver) = unbounded();
        let (result_sender, result_receiver) = unbounded();

        for _ in 0..options.thread_count.get() {
            let work_receiver = work_receiver.clone();
            let result_sender = result_sender.clone();

            scope.spawn(|_| {
                if let Err(error) = worker_proc(&options, work_receiver, result_sender) {
                    if !error.error_type().is_channel_disconnected() {
                        eprintln!("error in worker thread: {}", error);
                    }
                }
            });
        }

        drop(work_receiver);
        drop(result_sender);

        for path in paths {
            let compression_method = if path.compression_method == COMPR_DEFAULT {
                options.compression_method
            } else {
                path.compression_method
            };

            if options.version < 2 && compression_method != COMPR_NONE {
                return Err(Error::new("Compression is only supported startig with version 2".to_string())
                    .with_path(&path.filename));
            }

            let source_path: PathBuf;
            let filename = if let Some(filename) = &path.rename {
                source_path = (&path.filename).into();
                parse_pak_path(&filename).collect::<Vec<_>>()
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

            let metadata = match source_path.metadata() {
                Ok(metadata) => metadata,
                Err(error) => return Err(Error::io_with_path(error, source_path))
            };

            let mut make_filename = |file_path: &Path| -> Result<String> {
                let mut pak_filename: Vec<String> = file_path.components()
                    .skip(component_count)
                    .map(|comp| comp.as_os_str().to_string_lossy().to_string())
                    .collect();

                pak_filename.extend(filename.iter().map(|comp| comp.to_string()));

                let filename = make_pak_path(pak_filename.iter());

                if let Some(other_path) = filenames.insert(filename.clone(), file_path.to_owned()) {
                    return Err(Error::new(
                        format!("{}: filename not unique in archive, other path: {:?}", filename, other_path)
                    ).with_path(file_path));
                }

                Ok(filename)
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
                    let file_path = entry.path();
                    let filename = make_filename(&file_path)?;
                    match work_sender.send(Work {
                        compression_method,
                        file_path,
                        filename,
                        path,
                    }) {
                        Ok(()) => {}
                        Err(error) =>
                            return Err(Error::new(error.to_string()).with_path(entry.path()))
                    }
                }
            } else {
                let file_path = source_path.clone();
                let filename = make_filename(&file_path)?;
                match work_sender.send(Work {
                    compression_method,
                    file_path,
                    filename,
                    path,
                }) {
                    Ok(()) => {}
                    Err(error) =>
                        return Err(Error::new(error.to_string()).with_path(source_path))
                }
            }
        }

        drop(work_sender);

        let seperator = if options.null_separated { '\0' } else { '\n' };

        while let Ok(result) = result_receiver.recv() {
            let (mut record, mut data) = result?;

            record.move_to(options.version, data_size);

            buffer.clear();
            write_record_inline(&record, &mut buffer)?;

            data.splice(0..buffer.len(), buffer.iter().cloned());

            writer.write_all(&data)?;
            data_size += data.len() as u64;

            if options.verbose {
                print!("{}{}", record.filename(), seperator);
            }

            records.push(record);
        }

        drop(result_receiver);

        Ok(())
    });

    match thread_result {
        Err(error) => {
            return Err(Error::new(format!("threading error: {:?}", error)).with_path(pak_path));
        }
        Ok(result) => result?
    }

    let index_offset = data_size;

    writer.seek(SeekFrom::Start(index_offset))?;

    let mut index_size = 0u64;

    let mount_pount = options.mount_point.unwrap_or("");

    let mut hasher = OpenSSLSha1::new();

    buffer.clear();

    write_path(&mut buffer, mount_pount, options.encoding)?;
    encode!(&mut buffer, records.len() as u32);
    writer.write_all(&buffer)?;
    hasher.update(&buffer);

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
        hasher.update(&buffer);
        index_size += buffer.len() as u64;
    }

    let index_sha1: Sha1 = hasher.finish();

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
            if bytes.len() > (u32::MAX - 1) as usize {
                return Err(Error::new(format!("path is too long: {:?}", path)));
            }
            let size = (bytes.len() + 1) as u32;
            writer.write_all(&size.to_le_bytes())?;
            writer.write_all(bytes)?;
            writer.write_all(&[0])?;
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
            if bytes.len() > (u32::MAX - 1) as usize {
                return Err(Error::new(format!("path is too long: {:?}", path)));
            }
            let size = (bytes.len() + 1) as u32;
            writer.write_all(&size.to_le_bytes())?;
            writer.write_all(bytes)?;
            writer.write_all(&[0])?;
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

            let mut bytes: Vec<_> = path.chars().map(|ch| ch as u8).collect();
            bytes.push(0);
            if bytes.len() > u32::MAX as usize {
                return Err(Error::new(format!("path is too long: {:?}", path)));
            }
            let size = bytes.len() as u32;
            writer.write_all(&size.to_le_bytes())?;
            writer.write_all(&bytes)?;
        }
    }
    Ok(())
}

#[derive(Debug)]
struct Work<'a> {
    filename: String,
    file_path: PathBuf,
    path: &'a PackPath,
    compression_method: u32,
}

fn worker_proc(options: &PackOptions, work_channel: Receiver<Work>, result_channel: Sender<Result<(Record, Vec<u8>)>>) -> Result<()> {
    let mut buffer = vec![0u8; BUFFER_SIZE];
    let mut out_buffer = Vec::new();

    let compression_level = Compression::new(options.compression_level.get());

    let base_header_size = match options.version {
        1 => V1_RECORD_HEADER_SIZE,
        2 => V2_RECORD_HEADER_SIZE,
        3 => V3_RECORD_HEADER_SIZE,
        _ => {
            panic!("unsupported version: {}", options.version)
        }
    };
    let mut header_buffer = vec![0u8; base_header_size as usize];

    while let Ok(Work { filename, file_path, path, mut compression_method }) = work_channel.recv() {
        let mut data = Vec::new();
        let offset = 0;
        let compression_blocks;
        let mut compression_block_size = 0u32;
        let mut size;

        let mut in_file = match File::open(&file_path) {
            Ok(file) => file,
            Err(error) => {
                result_channel.send( Err(Error::io_with_path(error, file_path)))?;
                break;
            }
        };

        let metadata = match in_file.metadata() {
            Ok(metadata) => metadata,
            Err(error) => {
                result_channel.send( Err(Error::io_with_path(error, file_path)))?;
                break;
            }
        };

        let uncompressed_size = metadata.len();

        let timestamp = if options.version == 1 {
            let created = match metadata.created() {
                Ok(created) => created,
                Err(error) => {
                    result_channel.send( Err(Error::io_with_path(error, file_path)))?;
                    break;
                }
            };
            let timestamp = match created.duration_since(UNIX_EPOCH) {
                Ok(timestamp) => timestamp,
                Err(error) => {
                    result_channel.send(Err(Error::new(error.to_string()).with_path(file_path)))?;
                    break;
                }
            };
            Some(timestamp.as_secs())
        } else {
            None
        };

        let mut hasher = OpenSSLSha1::new();

        if uncompressed_size <= 100 {
            // It makes no sense to compress data <= 100 bytes because of
            // compression overhead.
            // In any case, the compression code below can't handle
            // uncompressed_size == 0.
            compression_method = COMPR_NONE;
        }

        match compression_method {
            self::COMPR_NONE => {
                size = uncompressed_size;
                compression_blocks = None;

                data.write_all(&header_buffer[..base_header_size as usize])?;

                let mut remaining = uncompressed_size as usize;
                {
                    // buffer might be bigger than BUFFER_SIZE if any previous
                    // compression_block_size is bigger than BUFFER_SIZE
                    if buffer.len() < BUFFER_SIZE {
                        buffer.resize(BUFFER_SIZE, 0);
                    }
                    let buffer = &mut buffer[..BUFFER_SIZE];
                    while remaining >= BUFFER_SIZE {
                        in_file.read_exact(buffer)?;
                        data.write_all(buffer)?;
                        hasher.update(buffer);
                        remaining -= BUFFER_SIZE;
                    }
                }

                if remaining > 0 {
                    let buffer = &mut buffer[..remaining];
                    in_file.read_exact(buffer)?;
                    data.write_all(buffer)?;
                    hasher.update(buffer);
                }
            }
            self::COMPR_ZLIB => {
                let compression_level = if let Some(compression_level) = path.compression_level {
                    Compression::new(compression_level.get())
                } else {
                    compression_level
                };
                if options.version <= 2 {
                    data.write_all(&header_buffer[..base_header_size as usize])?;

                    if buffer.len() < uncompressed_size as usize {
                        buffer.resize(uncompressed_size as usize, 0);
                    }
                    let buffer = &mut buffer[..uncompressed_size as usize];
                    in_file.read_exact(buffer)?;

                    out_buffer.clear();
                    let mut zlib = ZlibEncoder::new(&mut out_buffer, compression_level);
                    zlib.write_all(&buffer)?;
                    zlib.finish()?;
                    data.write_all(&out_buffer)?;
                    hasher.update(&out_buffer);

                    size = out_buffer.len() as u64;

                    compression_blocks = None;
                } else {
                    size = 0u64;
                    compression_block_size = path.compression_block_size
                        .unwrap_or(options.compression_block_size)
                        .get();

                    if compression_block_size as u64 > uncompressed_size {
                        compression_block_size = uncompressed_size as u32;
                    }

                    let mut header_size = base_header_size + 4;
                    if uncompressed_size > 0 {
                        header_size += (1 + ((uncompressed_size - 1) / compression_block_size as u64)) * COMPRESSION_BLOCK_HEADER_SIZE;
                    }
                    if header_buffer.len() < header_size as usize {
                        header_buffer.resize(header_size as usize, 0);
                    }
                    data.write_all(&header_buffer[..header_size as usize])?;

                    if buffer.len() < compression_block_size as usize {
                        buffer.resize(compression_block_size as usize, 0);
                    }

                    let buffer = &mut buffer[..compression_block_size as usize];
                    let mut blocks = Vec::<CompressionBlock>::new();
                    let mut remaining = uncompressed_size as usize;
                    let mut start_offset = header_size;

                    while remaining >= compression_block_size as usize {
                        in_file.read_exact(buffer)?;

                        out_buffer.clear();
                        let mut zlib = ZlibEncoder::new(&mut out_buffer, compression_level);
                        zlib.write_all(&buffer)?;
                        zlib.finish()?;
                        data.write_all(&out_buffer)?;
                        hasher.update(&out_buffer);

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
                        data.write_all(&out_buffer)?;
                        hasher.update(&out_buffer);

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
                result_channel.send(Err(Error::new(
                    format!("{}: unsupported compression method: {} ({})",
                        path.filename, compression_method_name(compression_method), compression_method))))?;
                break;
            }
        }

        let sha1: Sha1 = hasher.finish();

        let record = Record::new(
            filename,
            offset,
            size,
            uncompressed_size,
            compression_method,
            timestamp,
            sha1,
            compression_blocks,
            false,
            compression_block_size,
        );

        result_channel.send(Ok((record, data)))?;
    }

    Ok(())
}
