// This file is part of rust-u4pak.
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use std::{convert::TryFrom, fmt::Display, num::{NonZeroU32, NonZeroU64}, path::Path, usize};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, BufReader};
use log::{debug};

use crate::{Error, Record, Result};
use crate::decode;
use crate::decode::Decode;
use crate::index::{Encoding, Index};

pub const BUFFER_SIZE: usize = 2 * 1024 * 1024;

pub const PAK_MAGIC: u32 = 0x5A6F12E1;
pub const PAK_RELATIVE_COMPRESSION_OFFSET_VERSION: u32 = 5;
pub const PAK_MAX_SUPPORTED_VERSION: u32 = 11;

pub const DEFAULT_BLOCK_SIZE: NonZeroU32 = unsafe { NonZeroU32::new_unchecked(64 * 1024) };
pub const DEFAULT_COMPRESSION_LEVEL: NonZeroU32 = unsafe { NonZeroU32::new_unchecked(6) };
pub const DEFAULT_MIN_COMPRESSION_SIZE: NonZeroU64 = unsafe { NonZeroU64::new_unchecked(100) };

pub const COMPR_NONE       : u32 = 0x00;
pub const COMPR_ZLIB       : u32 = 0x01;
pub const COMPR_BIAS_MEMORY: u32 = 0x10; // I'm not sure, maybe these are just flags for zlib?
pub const COMPR_BIAS_SPEED : u32 = 0x20;

pub const V1_RECORD_HEADER_SIZE: u64 = 56;
pub const V2_RECORD_HEADER_SIZE: u64 = 48;
pub const V3_RECORD_HEADER_SIZE: u64 = 53;
pub const CONAN_EXILE_RECORD_HEADER_SIZE: u64 = 57;
pub const COMPRESSION_BLOCK_HEADER_SIZE: u64 = 16;

pub const PAK_BOOL_SIZE: usize = 1;
// TODO: Version 8 can have a max of 4 (version 4.22) or 5 (version 4.23-4.24)
pub const V8_PAK_COMPRESSION_METHOD_COUNT: usize = 5;
pub const PAK_COMPRESSION_METHOD_COUNT: usize = 5;
pub const PAK_COMPRESSION_METHOD_SIZE: usize = 32;
pub const PAK_ENCRYPTION_GUID_SIZE: usize = std::mem::size_of::<u128>();

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

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
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
pub struct Options {
    pub variant: Variant,
    pub ignore_magic: bool,
    pub encoding: Encoding,
    pub force_version: Option<u32>,
    pub encryption_key: Option<Vec<u8>>,
}

impl Default for Options {
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
    compression: Vec<u8>,
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
            Err(error) => Err(Error::io_with_path(error, path.as_ref())),
        }
    }

    #[inline]
    pub fn from_file(file: &mut File, options: Options) -> Result<Pak> {
        Self::from_reader(&mut BufReader::new(file), options)
    }

    pub fn from_reader<R>(reader: &mut R, options: Options) -> Result<Pak>
    where R: Read, R: Seek {
        let footer: Footer;
        
        if let Some(force_version) = options.force_version {
            footer = Self::decode_footer(reader, force_version)?;
            if !options.ignore_magic && footer.magic != 0x5A6F12E1 {
                return Err(Error::new(format!(
                    "illegal file magic: 0x{:X}",
                    footer.magic
                )));
            }
        } else if let Ok(version) = Self::get_version(reader) {
            debug!("Determined pak version {}", version);
            footer = Self::decode_footer(reader, version)?;
        } else if options.ignore_magic {
            footer = Self::decode_footer(reader, PAK_MAX_SUPPORTED_VERSION)?;
        } else {
            return Err(Error::new("Failed to determine pak file version.".to_string()))
        }

        let variant = options.variant;

        if footer.index_offset + footer.index_size > footer.footer_offset {
            return Err(Error::new(format!(
                "illegal index offset/size: index_offset ({}) + index_size ({}) > footer_offset ({})",
                footer.index_offset, footer.index_size, footer.footer_offset)));
        }

        reader.seek(SeekFrom::Start(footer.index_offset))?;

        let index = Index::read(
            reader,
            footer.index_size as usize,
            footer.version,
            variant,
            options.encoding,
            match footer.encrypted {
                true => options.encryption_key,
                false => None,
            },
        )?;

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
    pub fn index(&self) -> &Index {
        &self.index
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
                _ => {
                    let mut size: u64 = V3_RECORD_HEADER_SIZE;

                    if let Some(blocks) = &record.compression_blocks() {
                        size += blocks.len() as u64 * COMPRESSION_BLOCK_HEADER_SIZE;
                        if version >= 3 {
                            size += 4;
                        }
                    }
                    size
                }
            },
        }
    }

    pub fn footer_size(version: u32) -> i64 {
        // Same in every version
        let magic = std::mem::size_of::<u32>();
        let version_size = std::mem::size_of::<u32>();
        let index_offset = std::mem::size_of::<u64>();
        let index_size = std::mem::size_of::<u64>();
        let index_sha1 = std::mem::size_of::<Sha1>();
        let mut size: usize = magic + version_size + index_offset + index_size + index_sha1;

        // Version >= 4 has encrypted index flag
        if version >= 4 {
            size += PAK_BOOL_SIZE;
        }

        // Version 7 has encryption key guid
        if version >= 7 {
            size += PAK_ENCRYPTION_GUID_SIZE;
        }

        // Version 8 has Compression method
        if version == 8 {
            size += V8_PAK_COMPRESSION_METHOD_COUNT * PAK_COMPRESSION_METHOD_SIZE;
        } else if version > 8 {
            size += PAK_COMPRESSION_METHOD_COUNT * PAK_COMPRESSION_METHOD_SIZE;
        }

        // Version 9 has frozen index flag and version 10 upwards does not
        if version == 9 {
            size += PAK_BOOL_SIZE;
        }

        return i64::try_from(size).unwrap();
    }

    pub fn get_version<R>(reader: &mut R) -> Result<u32>
    where
        R: Read,
        R: Seek,
    {
        // Check if version >= 10 footer is found
        if reader.seek(SeekFrom::End(-Self::footer_size(10) +
                (PAK_ENCRYPTION_GUID_SIZE + PAK_BOOL_SIZE) as i64)).is_ok() {
            decode!(reader, magic: u32, version: u32);
            if magic == PAK_MAGIC {
                return Ok(version);
            }
        }

        // Check if version 9 footer is found
        if reader.seek(SeekFrom::End(-Self::footer_size(9) +
                (PAK_ENCRYPTION_GUID_SIZE + PAK_BOOL_SIZE) as i64)).is_ok() {
            decode!(reader, magic: u32, version: u32);
            if magic == PAK_MAGIC {
                return Ok(version);
            }
        }

        // Check if version 8 footer is found
        if reader.seek(SeekFrom::End(-Self::footer_size(8) +
                (PAK_ENCRYPTION_GUID_SIZE + PAK_BOOL_SIZE) as i64)).is_ok() {
            decode!(reader, magic: u32, version: u32);
            if magic == PAK_MAGIC {
                return Ok(version);
            }
        }

        // Check if version <= 7 footer is found
        if reader.seek(SeekFrom::End(-Self::footer_size(7) + (PAK_BOOL_SIZE) as i64)).is_ok() {
            decode!(reader, magic: u32, version: u32);
            if magic == PAK_MAGIC {
                return Ok(version);
            }
        }

        // Check if version <= 3 footer is found
        if reader.seek(SeekFrom::End(-Self::footer_size(3) as i64)).is_ok() {
            decode!(reader, magic: u32, version: u32);
            if magic == PAK_MAGIC {
                return Ok(version);
            }
        }

        Err(Error::new(String::from("No valid version detected")))
    }

    pub fn decode_footer<R>(reader: &mut R, target_version: u32) -> Result<Footer>
    where
        R: Read,
        R: Seek,
    {
        let footer_offset = reader
            .seek(SeekFrom::End(-Self::footer_size(target_version)));
        
        if let Ok(offset) = footer_offset {
            
            let encryption_uuid: u128 = 0;
            let frozen: bool = false;
            
            match target_version {
                _ if target_version >= 10 => {
                    decode!(
                        reader,
                        encryption_uuid: u128,
                        encrypted: bool,
                        magic: u32,
                        version: u32,
                        index_offset: u64,
                        index_size: u64,
                        index_sha1: Sha1,
                        compression: [u8; PAK_COMPRESSION_METHOD_COUNT * PAK_COMPRESSION_METHOD_SIZE]
                    );
                    Ok(Footer {
                        footer_offset: offset,
                        encryption_uuid,
                        encrypted,
                        magic,
                        version,
                        index_offset,
                        index_size,
                        index_sha1,
                        frozen,
                        compression: compression.to_vec(),
                    })
                }
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
                        compression: [u8; PAK_COMPRESSION_METHOD_COUNT * PAK_COMPRESSION_METHOD_SIZE]
                    );
                    Ok(Footer {
                        footer_offset: offset,
                        encryption_uuid,
                        encrypted,
                        magic,
                        version,
                        index_offset,
                        index_size,
                        index_sha1,
                        frozen,
                        compression: compression.to_vec(),
                    })
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
                        compression: [u8; V8_PAK_COMPRESSION_METHOD_COUNT * PAK_COMPRESSION_METHOD_SIZE]
                    );
                    Ok(Footer {
                        footer_offset: offset,
                        encryption_uuid,
                        encrypted,
                        magic,
                        version,
                        index_offset,
                        index_size,
                        index_sha1,
                        frozen,
                        compression: compression.to_vec(),
                    })
                }
                7 => {
                    decode!(
                        reader,
                        encryption_uuid: u128,
                        encrypted: bool,
                        magic: u32,
                        version: u32,
                        index_offset: u64,
                        index_size: u64,
                        index_sha1: Sha1
                    );
                    Ok(Footer {
                        footer_offset: offset,
                        encryption_uuid,
                        encrypted,
                        magic,
                        version,
                        index_offset,
                        index_size,
                        index_sha1,
                        frozen,
                        compression: vec![],
                    })
                }
                _ if target_version >= 4 => {
                    decode!(
                        reader,
                        encrypted: bool,
                        magic: u32,
                        version: u32,
                        index_offset: u64,
                        index_size: u64,
                        index_sha1: Sha1,
                    );
                    Ok(Footer {
                        footer_offset: offset,
                        encryption_uuid,
                        encrypted,
                        magic,
                        version,
                        index_offset,
                        index_size,
                        index_sha1,
                        frozen,
                        compression: vec![],
                    })
                }
                _ => {
                    decode!(
                        reader,
                        magic: u32,
                        version: u32,
                        index_offset: u64,
                        index_size: u64,
                        index_sha1: Sha1,
                    );
                    Ok(Footer {
                        footer_offset: offset,
                        encryption_uuid,
                        encrypted: false,
                        magic,
                        version,
                        index_offset,
                        index_size,
                        index_sha1,
                        frozen,
                        compression: vec![],
                    })
                }
            }
        } else if let Err(error) = footer_offset {
            Err(Error::from(error))
        } else {
            Err(Error::new("Failed to read footer.".to_string()))
        }
    }
}
