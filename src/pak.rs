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

use std::{collections::HashSet, convert::TryFrom, fmt::Display, num::{NonZeroU32, NonZeroUsize}, path::Path, usize};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, BufReader, stderr};

use crossbeam_channel::unbounded;
use crossbeam_utils::thread;
use openssl::sha::Sha1 as OpenSSLSha1;

use crate::{decode, reopen::Reopen};
use crate::decode::Decode;
use crate::{Record, Result, Error};

pub const BUFFER_SIZE: usize = 2 * 1024 * 1024;

pub const PAK_MAGIC: u32 = 0x5A6F12E1;

pub const DEFAULT_BLOCK_SIZE: NonZeroU32 = unsafe { NonZeroU32::new_unchecked(64 * 1024) };
pub const DEFAULT_COMPRESSION_LEVEL: NonZeroU32 = unsafe { NonZeroU32::new_unchecked(6) };

pub const COMPR_NONE       : u32 = 0x00;
pub const COMPR_ZLIB       : u32 = 0x01;
pub const COMPR_BIAS_MEMORY: u32 = 0x10; // I'm not sure, maybe these are just flags for zlib?
pub const COMPR_BIAS_SPEED : u32 = 0x20;

pub const V1_RECORD_HEADER_SIZE: u64 = 56;
pub const V2_RECORD_HEADER_SIZE: u64 = 48;
pub const V3_RECORD_HEADER_SIZE: u64 = 53;
pub const COMPRESSION_BLOCK_HEADER_SIZE: u64 = 16;

pub const COMPR_METHODS: [u32; 4] = [COMPR_NONE, COMPR_ZLIB, COMPR_BIAS_MEMORY, COMPR_BIAS_SPEED];

pub type Sha1 = [u8; 20];

pub const NULL_SHA1: Sha1 = [0u8; 20];

macro_rules! check_error {
    ($ok:expr, $result_sender:expr, $abort_on_error:expr, $error:expr) => {
        {
            if let Err(_) = $result_sender.send(Err($error)) {
                return;
            }

            if $abort_on_error {
                return;
            }

            $ok = false;
        }
    };
}

macro_rules! io {
    () => { Ok(()) };
    ($expr:expr $(,)?) => { $expr };
    ($expr:expr, $($tail:expr),* $(,)?) => {
        if let Err(_error) = ($expr) {
            Err(_error)
        } else {
            io!($($tail),*)
        }
    };
}

pub fn compression_method_name(compression_method: u32) -> &'static str {
    match compression_method {
        COMPR_NONE => "-",
        COMPR_ZLIB => "zlib",
        COMPR_BIAS_MEMORY => "bias memory",
        COMPR_BIAS_SPEED  => "bias speed",
        _ => "unknown",
    }
}

#[derive(Debug)]
pub struct HexDisplay<'a> {
    data: &'a [u8]
}

impl<'a> HexDisplay<'a> {
    #[inline]
    pub fn new(data: &'a [u8]) -> Self {
        Self { data }
    }
}

impl<'a> Display for HexDisplay<'a> {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for byte in self.data {
            write!(f, "{:02x}", byte)?;
        }
        Ok(())
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Encoding {
    ASCII,
    Latin1,
    UTF8,
}

impl Default for Encoding {
    fn default() -> Self {
        Encoding::UTF8
    }
}

impl Encoding {
    pub fn parse_vec(self, buffer: Vec<u8>) -> Result<String> {
        match self {
            Encoding::UTF8 => Ok(String::from_utf8(buffer)?),
            Encoding::ASCII => {
                for byte in &buffer {
                    if *byte > 0x7F {
                        return Err(Error::new(format!(
                            "ASCII conversion error: byte outside of ASCII range: {}",
                            *byte)));
                    }
                }
                Ok(buffer.into_iter().map(|byte| byte as char).collect())
            }
            Encoding::Latin1 => Ok(buffer.into_iter().map(|byte| byte as char).collect())
        }
    }
}

impl TryFrom<&str> for Encoding {
    type Error = crate::result::Error;

    fn try_from(encoding: &str) -> std::result::Result<Self, Error> {
        if encoding.eq_ignore_ascii_case("utf-8") || encoding.eq_ignore_ascii_case("utf8") {
            Ok(Encoding::UTF8)
        } else if encoding.eq_ignore_ascii_case("ascii") {
            Ok(Encoding::ASCII)
        } else if encoding.eq_ignore_ascii_case("latin1") || encoding.eq_ignore_ascii_case("iso-8859-1") {
            Ok(Encoding::Latin1)
        } else {
            Err(Error::new(format!("unsupported encoding: {:?}", encoding)))
        }
    }
}

#[derive(Debug)]
pub struct Pak {
    version: u32,
    index_offset: u64,
    index_size: u64,
    index_sha1: Sha1,
    mount_point: Option<String>,
    records: Vec<Record>,
}

#[derive(Debug)]
pub struct Options {
    pub ignore_magic: bool,
    pub encoding: Encoding,
    pub force_version: Option<u32>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            ignore_magic: false,
            encoding: Encoding::UTF8,
            force_version: None,
        }
    }
}

#[derive(Debug)]
pub struct CheckOptions {
    pub abort_on_error: bool,
    pub ignore_null_checksums: bool,
    pub null_separated: bool,
    pub verbose: bool,
    pub thread_count: NonZeroUsize,
}

impl Default for CheckOptions {
    fn default() -> Self {
        Self {
            abort_on_error: false,
            ignore_null_checksums: false,
            null_separated: false,
            verbose: false,
            thread_count: NonZeroUsize::new(num_cpus::get()).unwrap_or(NonZeroUsize::new(1).unwrap()),
        }
    }
}

pub fn read_path(reader: &mut impl Read, encoding: Encoding) -> Result<String> {
    let mut buf = [08; 4];
    reader.read_exact(&mut buf)?;
    let size = u32::from_le_bytes(buf);

    let mut buf = vec![0u8; size as usize];
    reader.read_exact(&mut buf)?;
    if let Some(index) = buf.iter().position(|&byte| byte == 0) {
        buf.truncate(index);
    }

    encoding.parse_vec(buf)
}

fn check_data<R>(reader: &mut R, filename: &str, offset: u64, size: u64, checksum: &Sha1, ignore_null_checksums: bool, buffer: &mut Vec<u8>) -> Result<()>
where R: Read, R: Seek {
    if ignore_null_checksums && checksum == &NULL_SHA1 {
        return Ok(());
    }
    reader.seek(SeekFrom::Start(offset))?;
    let mut hasher = OpenSSLSha1::new();
    let mut remaining = size;
    buffer.resize(BUFFER_SIZE, 0);
    loop {
        if remaining >= BUFFER_SIZE as u64 {
            reader.read_exact(buffer)?;
            hasher.update(&buffer);
            remaining -= BUFFER_SIZE as u64;
        } else {
            let buffer = &mut buffer[..remaining as usize];
            reader.read_exact(buffer)?;
            hasher.update(&buffer);
            break;
        }
    }
    let actual_digest = hasher.finish();
    if &actual_digest != checksum {
        return Err(Error::new(format!(
            "checksum missmatch:\n\
             \texpected: {}\n\
             \tactual:   {}",
             HexDisplay::new(checksum),
             HexDisplay::new(&actual_digest)
        )).with_path(filename));
    }
    Ok(())
}

impl Pak {
    #[inline]
    pub(crate) fn new(
        version: u32,
        index_offset: u64,
        index_size: u64,
        index_sha1: Sha1,
        mount_point: Option<String>,
        records: Vec<Record>,
    ) -> Self {
        Self {
            version,
            index_offset,
            index_size,
            index_sha1,
            mount_point,
            records,
        }
    }

    pub fn from_path(path: impl AsRef<Path>, options: Options) -> Result<Pak> {
        match File::open(&path) {
            Ok(mut file) => match Self::from_file(&mut file, options) {
                Ok(package) => Ok(package),
                Err(error) => if error.path.is_none() {
                    Err(error.with_path(path))
                } else {
                    Err(error)
                },
            }
            Err(error) => Err(Error::io_with_path(error, path.as_ref().to_path_buf())),
        }
    }

    #[inline]
    pub fn from_file(file: &mut File, options: Options) -> Result<Pak> {
        Self::from_reader(&mut BufReader::new(file), options)
    }

    pub fn from_reader<R>(reader: &mut R, options: Options) -> Result<Pak>
    where R: Read, R: Seek {
        let footer_offset = reader.seek(SeekFrom::End(-44))?;

        decode!(reader,
            magic: u32,
            version: u32,
            index_offset: u64,
            index_size: u64,
            index_sha1: Sha1,
        );

        let version = options.force_version.unwrap_or(version);

        if !options.ignore_magic && magic != 0x5A6F12E1 {
            return Err(Error::new(format!("illegal file magic: 0x{:X}", magic)));
        }

        let read_record = match version {
            1 => Record::read_v1,
            2 => Record::read_v2,
            _ if version <= 4 || version == 7 => Record::read_v3,
            _ => {
                return Err(Error::new(format!("unsupported version: {}", version)));
            }
        };

        if index_offset + index_size > footer_offset {
            return Err(Error::new(format!(
                "illegal index offset/size: index_offset ({}) + index_size ({}) > footer_size ({})",
                index_offset, index_size, footer_offset)));
        }

        reader.seek(SeekFrom::Start(index_offset))?;
        let mount_point = read_path(reader, options.encoding)?;

        decode!(reader, entry_count: u32);

        let mut records = Vec::with_capacity(entry_count as usize);

        for _ in 0..entry_count {
            let filename = read_path(reader, options.encoding)?;
            let record = read_record(reader, filename)?;
            records.push(record);
        }

        let pos = reader.seek(SeekFrom::Current(0))?;
        if pos > footer_offset {
            return Err(Error::new("index bleeds into footer".to_owned()));
        }

        Ok(Self {
            version,
            index_offset,
            index_size,
            index_sha1,
            mount_point: if mount_point.is_empty() { None } else { Some(mount_point) },
            records,
        })
    }

    #[inline]
    pub fn check_integrity(&self, in_file: &mut File, options: CheckOptions) -> Result<usize> {
        self.check_integrity_of(self.records.iter(), in_file, options)
    }

    pub fn check_integrity_of<'a, I>(&self, records: I, in_file: &mut File, options: CheckOptions) -> Result<usize>
    where
        I: std::iter::Iterator<Item=&'a Record> {
        let CheckOptions { abort_on_error, ignore_null_checksums, null_separated, verbose, thread_count } = options;
        let mut error_count = 0usize;
        let mut filenames = HashSet::new();
        let pak_path = in_file.path()?;

        if let Err(error) = check_data(&mut BufReader::new(in_file), "<archive index>", self.index_offset, self.index_size, &self.index_sha1, ignore_null_checksums, &mut vec![0u8; BUFFER_SIZE]) {
            error_count += 1;
            if abort_on_error {
                return Err(error);
            } else {
                let _ = error.write_to(&mut stderr(), null_separated);
            }
        }

        let version = self.version;
        let read_record = match version {
            1 => Record::read_v1,
            2 => Record::read_v2,
            _ if version <= 4 || version == 7 => Record::read_v3,
            _ => {
                return Err(Error::new(format!("unsupported version: {}", version)));
            }
        };

        let thread_result = thread::scope::<_, Result<usize>>(|scope| {
            let (work_sender, work_receiver) = unbounded::<&Record>();
            let (result_sender, result_receiver) = unbounded::<Result<&str>>();

            for _ in 0..thread_count.get() {
                let work_receiver = work_receiver.clone();
                let result_sender = result_sender.clone();
                let in_file = File::open(&pak_path)?;

                scope.spawn(move |_| {
                    let mut reader = BufReader::new(in_file);
                    let mut buffer = vec![0u8; BUFFER_SIZE];

                    while let Ok(record) = work_receiver.recv() {
                        let mut ok = true;

                        if !COMPR_METHODS.contains(&record.compression_method()) {
                            check_error!(ok, result_sender, abort_on_error, Error::new(format!(
                                "unknown compression method: 0x{:02x}",
                                record.compression_method(),
                            )).with_path(record.filename()));
                        }

                        if record.compression_method() == COMPR_NONE && record.size() != record.uncompressed_size() {
                            check_error!(ok, result_sender, abort_on_error, Error::new(format!(
                                "file is not compressed but compressed size ({}) differes from uncompressed size ({})",
                                record.size(),
                                record.uncompressed_size(),
                            )).with_path(record.filename()));
                        }

                        let offset = record.offset() + Self::header_size(self.version, record);
                        if offset + record.size() > self.index_offset {
                            check_error!(ok, result_sender, abort_on_error, Error::new(
                                "data bleeds into index".to_string()
                            ).with_path(record.filename()));
                        }

                        if let Err(error) = reader.seek(SeekFrom::Start(record.offset())) {
                            check_error!(ok, result_sender, abort_on_error,
                                Error::io_with_path(error, record.filename()));
                        } else {
                            match read_record(&mut reader, record.filename().to_string()) {
                                Ok(other_record) => {
                                    if other_record.offset() != 0 {
                                        check_error!(ok, result_sender, abort_on_error,
                                            Error::new(format!("data record offset field is not 0 but {}",
                                                    other_record.offset()))
                                                .with_path(other_record.filename()));
                                    }

                                    if !record.same_metadata(&other_record) {
                                        check_error!(ok, result_sender, abort_on_error,
                                            Error::new(format!("metadata missmatch:\n{}",
                                                    record.metadata_diff(&other_record)))
                                                .with_path(other_record.filename()));
                                    }
                                }
                                Err(error) => {
                                    check_error!(ok, result_sender, abort_on_error, error);
                                }
                            };
                        }

                        if let Some(blocks) = record.compression_blocks() {
                            if !ignore_null_checksums || record.sha1() != &NULL_SHA1 {
                                let base_offset = if self.version >= 7 { record.offset() } else { 0 };
                                let mut hasher = OpenSSLSha1::new();

                                for block in blocks {
                                    let block_size = block.end_offset - block.start_offset;

                                    buffer.resize(block_size as usize, 0);
                                    if let Err(error) = io!{
                                        reader.seek(SeekFrom::Start(base_offset + block.start_offset)),
                                        reader.read_exact(&mut buffer)
                                    } {
                                        let _ = result_sender.send(Err(Error::io_with_path(error, record.filename())));
                                        return;
                                    }
                                    hasher.update(&buffer);
                                }

                                let actual_digest = hasher.finish();
                                if &actual_digest != record.sha1() {
                                    check_error!(ok, result_sender, abort_on_error, Error::new(format!(
                                        "checksum missmatch:\n\
                                        \texpected: {}\n\
                                        \tactual:   {}",
                                        HexDisplay::new(record.sha1()),
                                        HexDisplay::new(&actual_digest)
                                    )).with_path(record.filename()));
                                }
                            }
                        } else if let Err(error) = check_data(&mut reader, record.filename(), offset,
                                record.size(), record.sha1(), ignore_null_checksums, &mut buffer) {
                            check_error!(ok, result_sender, abort_on_error, error);
                        }

                        if ok {
                            let _ = result_sender.send(Ok(record.filename()));
                        }
                    }
                });
            }

            drop(work_receiver);
            drop(result_sender);

            for record in records {
                if !filenames.insert(record.filename()) {
                    let error = Error::new(
                        "filename not unique in archive".to_string()
                    ).with_path(record.filename());

                    error_count += 1;
                    if abort_on_error {
                        return Err(error);
                    } else {
                        let _ = error.write_to(&mut stderr(), null_separated);
                    }
                }

                match work_sender.send(record) {
                    Ok(()) => {}
                    Err(error) =>
                        return Err(Error::new(error.to_string()).with_path(record.filename()))
                }
            }

            drop(work_sender);

            let mut stderr = stderr();
            let linesep = if options.null_separated { '\0' } else { '\n' };

            while let Ok(result) = result_receiver.recv() {
                match result {
                    Ok(filename) => {
                        if verbose {
                            print!("{}: OK{}", filename, linesep);
                        }
                    }
                    Err(error) => {
                        let _ = error.write_to(&mut stderr, null_separated);
                    }
                }
            }

            Ok(error_count)
        });

        match thread_result {
            Err(error) => {
                return Err(Error::new(format!("threading error: {:?}", error)));
            }
            Ok(result) => result
        }
    }

    #[inline]
    pub fn version(&self) -> u32 {
        self.version
    }

    #[inline]
    pub fn index_offset(&self) -> u64 {
        self.index_offset
    }

    #[inline]
    pub fn index_size(&self) -> u64 {
        self.index_size
    }

    #[inline]
    pub fn index_sha1(&self) -> &Sha1 {
        &self.index_sha1
    }

    #[inline]
    pub fn mount_point(&self) -> Option<&str> {
        match &self.mount_point {
            Some(mount_point) => Some(mount_point),
            None => None
        }
    }

    #[inline]
    pub fn records(&self) -> &[Record] {
        &self.records
    }

    #[inline]
    pub fn into_records(self) -> Vec<Record> {
        self.records
    }

    //#[inline]
    //pub fn filter_records<'a>(&'a self, filter: &'a mut Filter<'a>) -> std::iter::Filter<impl Iterator<Item=&'a Record>, impl FnMut(&&'a Record) -> bool> {
    //    filter.filter(self.records.iter())
    //}

    pub fn header_size(version: u32, record: &Record) -> u64 {
        match version {
            1 => V1_RECORD_HEADER_SIZE,
            2 => V2_RECORD_HEADER_SIZE,
            _ if version <= 4 || version == 7 => {
                let mut size: u64 = V3_RECORD_HEADER_SIZE;
                if let Some(blocks) = &record.compression_blocks() {
                    size += blocks.len() as u64 * COMPRESSION_BLOCK_HEADER_SIZE;
                }
                size
            }
            _ => {
                panic!("unsupported version: {}", version)
            }
        }
    }
}
