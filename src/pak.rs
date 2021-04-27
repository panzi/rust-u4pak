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

use std::{convert::TryFrom, fmt::Display, path::Path};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, BufReader};

use crypto::digest::Digest;
use crypto::sha1::{Sha1 as Sha1Hasher};

use crate::decode;
use crate::decode::Decode;
use crate::{Record, Result, Error};
use crate::record::CompressionBlock;

pub const BUFFER_SIZE: usize = 2 * 1024 * 1024;

pub const PAK_MAGIC: u32 = 0x5A6F12E1;

pub const COMPR_NONE       : u32 = 0x00;
pub const COMPR_ZLIB       : u32 = 0x01;
pub const COMPR_BIAS_MEMORY: u32 = 0x10;
pub const COMPR_BIAS_SPEED : u32 = 0x20;

pub const COMPR_METHODS: [u32; 4] = [COMPR_NONE, COMPR_ZLIB, COMPR_BIAS_MEMORY, COMPR_BIAS_SPEED];

pub type Sha1 = [u8; 20];

pub const NULL_SHA1: Sha1 = [0u8; 20];

macro_rules! check_error {
    ($error_count:ident, $abort_on_error:expr, $null_separator:expr, $($error:tt)*) => {
        {
            let error = $($error)*;
            $error_count += 1;
            if $abort_on_error {
                return Err(error);
            }
            eprint!("{}{}", error, $null_separator);
        }
    };
}

pub fn compression_method_name(compression_method: u32) -> &'static str {
    match compression_method {
        COMPR_NONE => "none",
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

pub struct Pak {
    version: u32,
    index_offset: u64,
    index_size: u64,
    footer_offset: u64,
    index_sha1: Sha1,
    mount_point: Option<String>,
    records: Vec<Record>,
}

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

pub fn read_path(reader: &mut impl Read, encoding: Encoding) -> Result<String> {
    let mut buf = [08; 4];
    reader.read_exact(&mut buf)?;
    let size = u32::from_le_bytes(buf);

    let mut buf = vec![0u8; size as usize];
    reader.read_exact(&mut buf)?;
    if let Some(index) = buf.iter().position(|byte| *byte == 0) {
        buf.truncate(index);
    }

    encoding.parse_vec(buf)
}

pub fn read_record_v1(reader: &mut impl Read, filename: String) -> Result<Record> {
    decode!(reader,
        offset: u64,
        size: u64,
        uncompressed_size: u64,
        compression_method: u32,
        timestamp: u64,
        sha1: Sha1,
    );

    Ok(Record::v1(filename, offset, size, uncompressed_size, compression_method, timestamp, sha1))
}

pub fn read_record_v2(reader: &mut impl Read, filename: String) -> Result<Record> {
    decode!(reader,
        offset: u64,
        size: u64,
        uncompressed_size: u64,
        compression_method: u32,
        sha1: Sha1,
    );

    Ok(Record::v2(filename, offset, size, uncompressed_size, compression_method, sha1))
}

pub fn read_record_v3(reader: &mut impl Read, filename: String) -> Result<Record> {
    decode!(reader,
        offset: u64,
        size: u64,
        uncompressed_size: u64,
        compression_method: u32,
        sha1: Sha1,
        if compression_method != COMPR_NONE {
            compression_blocks: CompressionBlock [u32],
        }
        encrypted: u8,
        compression_block_size: u32,
    );

    Ok(Record::v3(filename, offset, size, uncompressed_size, compression_method, sha1, compression_blocks, encrypted != 0, compression_block_size))
}

pub fn read_record_v4(reader: &mut impl Read, filename: String) -> Result<Record> {
    decode!(reader,
        offset: u64,
        size: u64,
        uncompressed_size: u64,
        compression_method: u32,
        sha1: Sha1,
        if compression_method != COMPR_NONE {
            compression_blocks: CompressionBlock [u32],
        }
        encrypted: u8,
        compression_block_size: u32,
        _unknown: u32,
    );

    Ok(Record::v4(filename, offset, size, uncompressed_size, compression_method, sha1, compression_blocks, encrypted != 0, compression_block_size))
}

impl Pak {
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

        let version = if let Some(version) = options.force_version {
            version
        } else {
            version
        };

        if !options.ignore_magic && magic != 0x5A6F12E1 {
            return Err(Error::new(format!("illegal file magic: 0x{:X}", magic)));
        }

        let read_record = match version {
            1 => read_record_v1,
            2 => read_record_v2,
            3 => read_record_v3,
            4 => read_record_v4,
            7 => read_record_v3,
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
            footer_offset,
            index_sha1,
            mount_point: if mount_point.is_empty() { None } else { Some(mount_point) },
            records,
        })
    }

    #[inline]
    pub fn check_integrity<R>(&self, reader: &mut R, abort_on_error: bool, ignore_null_checksums: bool, null_separated: bool) -> Result<usize>
    where R: Read, R: Seek {
        self.check_integrity_of(self.records.iter(), reader, abort_on_error, ignore_null_checksums, null_separated)
    }

    pub fn check_integrity_of<I, Item, R>(&self, records: I, reader: &mut R, abort_on_error: bool, ignore_null_checksums: bool, null_separated: bool) -> Result<usize>
    where
        Item: AsRef<Record>,
        I: std::iter::Iterator<Item=Item>,
        R: Read, R: Seek {
        let mut hasher = Sha1Hasher::new();
        let mut buffer = vec![0u8; BUFFER_SIZE];
        let mut error_count = 0usize;
        let mut actual_digest = [0u8; 20];
        let null_separator = if null_separated { '\0' } else { '\n' };

        let mut check_data = |filename: &str, offset: u64, size: u64, checksum: &Sha1| -> Result<()> {
            if ignore_null_checksums && checksum == &NULL_SHA1 {
                return Ok(());
            }
            reader.seek(SeekFrom::Start(offset))?;
            hasher.reset();
            let mut remaining = size;
            loop {
                if remaining >= BUFFER_SIZE as u64 {
                    reader.read_exact(&mut buffer)?;
                    hasher.input(&buffer);
                    remaining -= BUFFER_SIZE as u64;
                } else {
                    let buffer = &mut buffer[..remaining as usize];
                    reader.read_exact(buffer)?;
                    hasher.input(&buffer);
                    break;
                }
            }
            hasher.result(&mut actual_digest);
            if &actual_digest != checksum {
                return Err(Error::new(format!(
                    "checksum missmatch:\n\
                     expected: {}\n\
                     actual:   {}",
                     HexDisplay::new(checksum),
                     HexDisplay::new(&actual_digest)
                )).with_path(filename));
            }
            Ok(())
        };

        if let Err(error) = check_data("<archive index>", self.index_offset, self.index_size, &self.index_sha1) {
            check_error!(error_count, abort_on_error, null_separator, error);
        }

        for record in records {
            let record = record.as_ref();
            if !COMPR_METHODS.contains(&record.compression_method()) {
                check_error!(error_count, abort_on_error, null_separator, Error::new(format!(
                    "unknown compression method: 0x{:02x}",
                    record.compression_method(),
                )).with_path(record.filename()));
            }

            if record.compression_method() == COMPR_NONE && record.size() != record.uncompressed_size() {
                check_error!(error_count, abort_on_error, null_separator, Error::new(format!(
                    "file is not compressed but compressed size ({}) differes from uncompressed size ({})",
                    record.size(),
                    record.uncompressed_size(),
                )).with_path(record.filename()));
            }

            let offset = self.data_offset(record);
            if offset + record.size() > self.index_offset {
                check_error!(error_count, abort_on_error, null_separator, Error::new(
                    "data bleeds into index".to_string()
                ).with_path(record.filename()));
            }

            if let Err(error) = check_data(record.filename(), offset, record.size(), record.sha1()) {
                check_error!(error_count, abort_on_error, null_separator, error);
            }
        }

        Ok(error_count)
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
    pub fn footer_offset(&self) -> u64 {
        self.footer_offset
    }

    #[inline]
    pub fn index_sha1(&self) -> &Sha1 {
        &self.index_sha1
    }

    #[inline]
    pub fn mount_point(&self) -> &Option<String> {
        &self.mount_point
    }

    #[inline]
    pub fn records(&self) -> &[Record] {
        &self.records
    }

    #[inline]
    pub fn into_records(self) -> Vec<Record> {
        self.records
    }

    pub fn header_size(&self, record: &Record) -> u64 {
        match self.version {
            1 => 56,
            2 => 48,
            3 => {
                let mut size: u64 = 53;
                if let Some(blocks) = &record.compression_blocks() {
                    size += blocks.len() as u64 * 16;
                }
                size
            }
            4 => {
                let mut size: u64 = 57;
                if let Some(blocks) = &record.compression_blocks() {
                    size += blocks.len() as u64 * 16;
                }
                size
            }
            _ => {
                panic!("unsupported version: {}", self.version)
            }
        }
    }

    #[inline]
    pub fn data_offset(&self, record: &Record) -> u64 {
        self.header_size(record) + record.offset()
    }

    pub fn unpack(&self, record: &Record, input: &mut File, outdir: impl AsRef<Path>) -> Result<()> {
        if record.encrypted() {
            return Err(Error::new("encryption is not supported".to_string())
                .with_path(record.filename()));
        }

        match record.compression_method() {
            self::COMPR_NONE => {

            }
            self::COMPR_ZLIB => {

            }
            _ => {
                return Err(Error::new(format!(
                        "unsupported compression method: {}",
                        compression_method_name(record.compression_method())))
                    .with_path(record.filename()));
            }
        }
        Ok(())
    }
}
