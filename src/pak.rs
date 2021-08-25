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
use crate::index;
use crate::index::{Encoding, Index};

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

#[derive(Debug)]
pub struct Options<'a> {
    pub variant: Variant,
    pub ignore_magic: bool,
    pub encoding: Encoding,
    pub force_version: Option<u32>,
    pub encryption_key: Option<&'a str>,
}

impl Default for Options<'_> {
    fn default() -> Self {
        Self {
            variant: Variant::default(),
            ignore_magic: false,
            encoding: Encoding::UTF8,
            force_version: None,
            encryption_key: None,
        }
    }
}

pub struct Footer {
    footer_offset: u64,
    encryption_uuid: u128,
    encrypted: bool,
    magic: u32,
    version: u32,
    index_offset: u64,
    index_size: u64,
    index_sha1: Sha1,
    frozen: bool,
    compression: [u8; 160],
}

#[derive(Debug)]
pub struct Pak {
    variant: Variant,
    version: u32,
    index_offset: u64,
    index_size: u64,
    index_sha1: Sha1,
    index: Index,
}

impl Pak {
    #[inline]
    pub(crate) fn new(
        variant: Variant,
        version: u32,
        index_offset: u64,
        index_size: u64,
        index_sha1: Sha1,
        index: Index,
    ) -> Self {
        Self {
            variant,
            version,
            index_offset,
            index_size,
            index_sha1,
            index,
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
        let mut pak_file_version: u32 = 9;
        let mut footer: Footer;

        if options.force_version.is_some() {
            pak_file_version = options.force_version.unwrap();
            footer = Self::decode_footer(reader, pak_file_version)?;
        } else {
            loop {
                footer = Self::decode_footer(reader, pak_file_version)?;

                if options.ignore_magic || footer.magic == 0x5A6F12E1 {
                    break;
                } else if pak_file_version < 1 {
                    return Err(Error::new(format!(
                        "illegal file magic: 0x{:X}",
                        footer.magic
                    )));
                }

                pak_file_version -= 1;
            }
        }

        let variant = options.variant;

        if footer.index_offset + footer.index_size > footer.footer_offset {
            return Err(Error::new(format!(
                "illegal index offset/size: index_offset ({}) + index_size ({}) > footer_offset ({})",
                footer.index_offset, footer.index_size, footer.footer_offset)));
        }

        reader.seek(SeekFrom::Start(footer.index_offset))?;
        let mut buff = vec![0; footer.index_size as usize];
        reader.read_exact(&mut buff)?;

        let index = Index::read(
            &mut buff,
            footer.version,
            variant,
            options.encoding,
            match footer.encrypted {
                true => options.encryption_key,
                false => None,
            },
        );

        let pos = reader.seek(SeekFrom::Current(0))?;
        if pos > footer.footer_offset {
            return Err(Error::new("index bleeds into footer".to_owned()));
        }

        Ok(Self {
            variant,
            version: footer.version,
            index_offset: footer.index_offset,
            index_size: footer.index_size,
            index_sha1: footer.index_sha1,
            index,
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
        match &self.index.mount_point {
            Ok(mount_point) => Some(mount_point),
            Error => None,
        }
    }

    #[inline]
    pub fn records(&self) -> &[Record] {
        &self.index.records
    }

    #[inline]
    pub fn into_records(self) -> Vec<Record> {
        self.index.records
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
                _ if version <= 5 || version == 7 || version == 9 => {
                    let mut size: u64 = V3_RECORD_HEADER_SIZE;

                    if let Some(blocks) = &record.compression_blocks() {
                        size += blocks.len() as u64 * COMPRESSION_BLOCK_HEADER_SIZE;
                        if version == 9 {
                            size += 4;
                        }
                    }
                    size
                }
                _ => {
                    panic!("unsupported version: {}", version)
                }
            },
        }
    }

    pub fn footer_size(version: u32) -> i64 {
        // Same in every version
        let encrypted = std::mem::size_of::<bool>();
        let magic = std::mem::size_of::<u32>();
        let version_size = std::mem::size_of::<u32>();
        let index_offset = std::mem::size_of::<u64>();
        let index_size = std::mem::size_of::<u64>();
        let index_sha1 = std::mem::size_of::<Sha1>();
        let mut size: usize =
            encrypted + magic + version_size + index_offset + index_size + index_sha1;

        // Version 7 has encryption key guid
        if version >= 7 {
            size += std::mem::size_of::<u128>();
        }

        // Version 8 has Compression method
        if version >= 8 {
            size += std::mem::size_of::<[u8; 160]>();
        }

        // Version 9 has frozen index flag
        if version >= 9 {
            size += std::mem::size_of::<bool>();
        }

        return i64::try_from(size).unwrap();
    }

    pub fn decode_footer<R>(reader: &mut R, target_version: u32) -> Result<Footer>
    where
        R: Read,
        R: Seek,
    {
        let footer_offset = reader
            .seek(SeekFrom::End(-Self::footer_size(target_version)))
            .expect("Failed to get footer offset.");

        let encryption_uuid: u128 = 0;
        let frozen: bool = false;
        let compression: [u8; 160] = [0; 160];

        match target_version {
            9 => {
                decode!(
                    reader,
                    encryption_uuid: u128,
                    encrypted: bool,
                    magic: u32,
                    version: u32,
                    index_offset: u64,
                    index_size: u64,
                    index_sha1: Sha1,
                    frozen: bool,
                    compression: [u8; 160]
                );
                return Ok(Footer {
                    footer_offset,
                    encryption_uuid,
                    encrypted,
                    magic,
                    version,
                    index_offset,
                    index_size,
                    index_sha1,
                    frozen,
                    compression,
                });
            }
            8 => {
                decode!(
                    reader,
                    encryption_uuid: u128,
                    encrypted: bool,
                    magic: u32,
                    version: u32,
                    index_offset: u64,
                    index_size: u64,
                    index_sha1: Sha1,
                    frozen: bool,
                    compression: [u8; 160]
                );
                return Ok(Footer {
                    footer_offset,
                    encryption_uuid,
                    encrypted,
                    magic,
                    version,
                    index_offset,
                    index_size,
                    index_sha1,
                    frozen,
                    compression,
                });
            }
            _ => {
                decode!(
                    reader,
                    encrypted: bool,
                    magic: u32,
                    version: u32,
                    index_offset: u64,
                    index_size: u64,
                    index_sha1: Sha1,
                );
                return Ok(Footer {
                    footer_offset,
                    encryption_uuid,
                    encrypted,
                    magic,
                    version,
                    index_offset,
                    index_size,
                    index_sha1,
                    frozen,
                    compression,
                });
            }
        }
    }
}
