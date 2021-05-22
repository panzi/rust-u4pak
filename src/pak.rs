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

use std::{convert::TryFrom, fmt::Display, num::NonZeroU32, path::Path, usize};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, BufReader};

use crate::{Error, Record, Result};
use crate::decode;
use crate::decode::Decode;

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
pub const CONAN_EXILE_RECORD_HEADER_SIZE: u64 = 57;
pub const COMPRESSION_BLOCK_HEADER_SIZE: u64 = 16;

pub const COMPR_METHODS: [u32; 4] = [COMPR_NONE, COMPR_ZLIB, COMPR_BIAS_MEMORY, COMPR_BIAS_SPEED];

pub type Sha1 = [u8; 20];

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
pub enum Variant {
    Standard,
    ConanExiles,
}

impl Default for Variant {
    fn default() -> Self {
        Variant::Standard
    }
}

impl TryFrom<&str> for Variant {
    type Error = crate::result::Error;

    fn try_from(variant: &str) -> std::result::Result<Self, Error> {
        let trimmed_variant = variant.trim();
        if trimmed_variant.eq_ignore_ascii_case("standard") {
            Ok(Variant::Standard)
        } else if trimmed_variant.eq_ignore_ascii_case("conan_exiles") || trimmed_variant.eq_ignore_ascii_case("conanexiles") || trimmed_variant.eq_ignore_ascii_case("conan exiles") {
            Ok(Variant::ConanExiles)
        } else {
            Err(Error::new(format!("illegal variant: {:?}", variant)))
        }
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
    variant: Variant,
    version: u32,
    index_offset: u64,
    index_size: u64,
    index_sha1: Sha1,
    mount_point: Option<String>,
    records: Vec<Record>,
}

#[derive(Debug)]
pub struct Options {
    pub variant: Variant,
    pub ignore_magic: bool,
    pub encoding: Encoding,
    pub force_version: Option<u32>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            variant: Variant::default(),
            ignore_magic: false,
            encoding: Encoding::UTF8,
            force_version: None,
        }
    }
}

pub fn read_path(reader: &mut impl Read, encoding: Encoding) -> Result<String> {
    let mut buf = [0; 4];
    reader.read_exact(&mut buf)?;
    let size = u32::from_le_bytes(buf);

    let mut buf = vec![0u8; size as usize];
    reader.read_exact(&mut buf)?;
    if let Some(index) = buf.iter().position(|&byte| byte == 0) {
        buf.truncate(index);
    }

    encoding.parse_vec(buf)
}

impl Pak {
    #[inline]
    pub(crate) fn new(
        variant: Variant,
        version: u32,
        index_offset: u64,
        index_size: u64,
        index_sha1: Sha1,
        mount_point: Option<String>,
        records: Vec<Record>,
    ) -> Self {
        Self {
            variant,
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

        let variant = options.variant;
        let read_record = match variant {
            Variant::ConanExiles => {
                if version != 4 {
                    return Err(Error::new(format!("Only know how to handle Conan Exile paks of version 4, but version was {}.", version)));
                }
                Record::read_conan_exiles
            }
            Variant::Standard => match version {
                1 => Record::read_v1,
                2 => Record::read_v2,
                _ if version <= 5 || version == 7 => Record::read_v3,
                _ => {
                    return Err(Error::new(format!("unsupported version: {}", version)));
                }
            }
        };

        if index_offset + index_size > footer_offset {
            return Err(Error::new(format!(
                "illegal index offset/size: index_offset ({}) + index_size ({}) > footer_offset ({})",
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
            variant,
            version,
            index_offset,
            index_size,
            index_sha1,
            mount_point: if mount_point.is_empty() { None } else { Some(mount_point) },
            records,
        })
    }

    #[inline]
    pub fn variant(&self) -> Variant {
        self.variant
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

    // FIXME: inline header has different size in some versions/variants!
    pub fn header_size(version: u32, variant: Variant, record: &Record) -> u64 {
        match variant {
            Variant::ConanExiles => {
                if version != 4 {
                    panic!("unsupported Conan Exile pak version: {}", version)
                }
                CONAN_EXILE_RECORD_HEADER_SIZE
            }
            Variant::Standard => match version {
                1 => V1_RECORD_HEADER_SIZE,
                2 => V2_RECORD_HEADER_SIZE,
                _ if version <= 5 || version == 7 => {
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
}
