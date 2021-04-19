use std::{convert::TryFrom, path::Path};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, BufReader};

use crate::record::CompressionBlock;
use crate::{Record, Result, Error};

pub const PAK_MAGIC: u32 = 0x5A6F12E1;

pub const COMPR_NONE       : u32 = 0x00;
pub const COMPR_ZLIB       : u32 = 0x01;
pub const COMPR_BIAS_MEMORY: u32 = 0x10;
pub const COMPR_BIAS_SPEED : u32 = 0x20;

pub type Sha1 = [u8; 20];

pub const NULL_SHA1: Sha1 = [0u8; 20];

pub fn compression_method_name(compression_method: u32) -> &'static str {
    match compression_method {
        COMPR_NONE => "none",
        COMPR_ZLIB => "zlib",
        COMPR_BIAS_MEMORY => "bias memory",
        COMPR_BIAS_SPEED  => "bias speed",
        _ => "unknown",
    }
}

pub fn format_sha1(sha1: &Sha1) -> String {
    format!("{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        sha1[ 0], sha1[ 1], sha1[ 2], sha1[ 3], sha1[ 4], sha1[ 5], sha1[ 6], sha1[ 7], sha1[ 8], sha1[ 9],
        sha1[10], sha1[11], sha1[12], sha1[13], sha1[14], sha1[15], sha1[16], sha1[17], sha1[18], sha1[19])
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
    pub check_integrity: bool,
    pub ignore_magic: bool,
    pub encoding: Encoding,
    pub force_version: Option<u32>,
    pub ignore_null_checksums: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            check_integrity: false,
            ignore_magic: false,
            encoding: Encoding::UTF8,
            force_version: None,
            ignore_null_checksums: false,
        }
    }
}

pub fn read_path(file: &mut impl Read, encoding: Encoding) -> Result<String> {
    let mut buf = [08; 4];
    file.read_exact(&mut buf)?;
    let size = u32::from_le_bytes(buf);

    let mut buf = vec![0u8; size as usize];
    file.read_exact(&mut buf)?;
    if let Some(index) = buf.iter().position(|byte| *byte == 0) {
        buf.truncate(index);
    }

    encoding.parse_vec(buf)
}

pub trait Unpack: Sized {
    fn unpack(reader: &mut impl Read) -> Result<Self>;
}

impl Unpack for u32 {
    #[inline]
    fn unpack(reader: &mut impl Read) -> Result<Self> {
        let mut buffer = [0u8; 4];
        reader.read_exact(&mut buffer)?;
        Ok(Self::from_le_bytes(buffer))
    }
}

impl Unpack for u8 {
    #[inline]
    fn unpack(reader: &mut impl Read) -> Result<Self> {
        let mut buffer = [0u8; 1];
        reader.read_exact(&mut buffer)?;
        Ok(buffer[0])
    }
}

impl Unpack for u64 {
    #[inline]
    fn unpack(reader: &mut impl Read) -> Result<Self> {
        let mut buffer = [0u8; 8];
        reader.read_exact(&mut buffer)?;
        Ok(Self::from_le_bytes(buffer))
    }
}

// TODO: this all might be inefficient for T=u8
impl<T: Unpack, const N: usize> Unpack for [T; N] where T: Default, T: Copy {
    #[inline]
    fn unpack(reader: &mut impl Read) -> Result<Self> {
        let mut items: [T; N] = [T::default(); N];
        for index in 0..N {
            items[index] = T::unpack(reader)?;
        }
        Ok(items)
    }
}

impl Unpack for CompressionBlock {
    #[inline]
    fn unpack(reader: &mut impl Read) -> Result<Self> {
        let start_offset = u64::unpack(reader)?;
        let end_offset   = u64::unpack(reader)?;

        Ok(Self {
            start_offset,
            end_offset,
        })
    }
}

macro_rules! unpack {
    ($reader:expr, $($rest:tt)*) => {
        unpack!(@decl $($rest)*);
        unpack!(@unpack () ($reader) $($rest)*);
    };

    (@unpack ($($wrap:tt)*) ($reader:expr) $(,)?) => {};

    (@unpack ($($wrap:tt)*) ($reader:expr) if $($rest:tt)*) => {
        unpack!(@if ($($wrap)*) ($reader) () $($rest)*);
    };

    (@if ($($wrap:tt)*) ($reader:expr) ($($cond:tt)*) { $($body:tt)* } $($rest:tt)*) => {
        if $($cond)* {
            unpack!(@unpack (Some) ($reader) $($body)*);
        } else {
            unpack!(@none $($body)*);
        }
        unpack!(@unpack ($($wrap)*) ($reader) $($rest)*);
    };

    (@if ($($wrap:tt)*) ($reader:expr) ($($cond:tt)*) $tok:tt $($rest:tt)*) => {
        unpack!(@if ($($wrap)*) ($reader) ($($cond)* $tok) $($rest)*);
    };

    (@decl $(,)?) => {};

    (@decl if $($rest:tt)*) => {
        unpack!(@decl_if () $($rest)*);
    };

    (@decl $(#[$($attrs:tt)*])? $name:ident : $type:ty $([$($count:tt)*])? $(,)?) => {
        let $name;
    };

    (@decl $(#[$($attrs:tt)*])? $name:ident : $type:ty $([$($count:tt)*])?, $($rest:tt)*) => {
        let $name;
        unpack!(@decl $($rest)*);
    };

    (@decl_if ($($cond:tt)*) { $($body:tt)* } $($rest:tt)*) => {
        unpack!(@decl $($body)*);
        unpack!(@decl $($rest)*);
    };

    (@decl_if ($($cond:tt)*) $tok:tt $($rest:tt)*) => {
        unpack!(@decl_if ($($cond)* $tok) $($rest)*);
    };

    (@none $(,)?) => {};

    (@none if $($rest:tt)*) => {
        unpack!(@none_if () $($rest)*);
    };

    (@none $(#[$($attrs:tt)*])? $name:ident : $type:ty $([$($count:tt)*])? $(,)?) => {
        $name = None;
    };

    (@none $(#[$($attrs:tt)*])? $name:ident : $type:ty $([$($count:tt)*])?, $($rest:tt)*) => {
        $name = None;
        unpack!(@none $($rest)*);
    };

    (@none_if ($cond:expr) { $($body:tt)* } $($rest:tt)*) => {
        unpack!(@none $($body)*);
        unpack!(@none $($rest)*);
    };

    (@none_if ($($cond:tt)*) $tok:tt $($rest:tt)*) => {
        unpack!(@none_if ($($cond)* $tok) $($rest)*);
    };

    (@unpack ($($wrap:tt)*) ($reader:expr) $(#[$($attrs:tt)*])? $name:ident : $type:ty $([$($count:tt)*])? $(,)?) => {
        unpack!(@read ($($wrap)*) ($reader) ($($($attrs)*)?) $name $type $([$($count)*])?);
    };

    (@unpack ($($wrap:tt)*) ($reader:expr) $(#[$($attrs:tt)*])? $name:ident : $type:ty $([$($count:tt)*])?, $($rest:tt)*) => {
        unpack!(@read ($($wrap)*) ($reader) ($($($attrs)*)?) $name $type $([$($count)*])?);
        unpack!(@unpack ($($wrap)*) ($reader) $($rest)*);
    };

    // FIXME: This never matches! Why?
    (@read ($($wrap:tt)*) ($reader:expr) ($($attrs:tt)*) $name:ident String) => {
        $name = {
            let _encoding = unpack!(@attr_encoding $($attrs)*);
            let _size = <unpack!(@attr_size $($attrs)*)>::unpack($reader)? as usize;
            let _buffer = vec![0u8; _size];
            if let Some(_index) = _buffer.iter().position(|_byte| *_byte == 0) {
                _buffer.truncate(_index);
            }
            $($wrap)*(_encoding.parse_vec(_buffer))
        };
    };

    (@read ($($wrap:tt)*) ($reader:expr) ($($attrs:tt)*) $name:ident $type:ty) => {
        $name = $($wrap)*(<$type>::unpack($reader)?);
    };

    (@read ($($wrap:tt)*) ($reader:expr) ($($attrs:tt)*) $name:ident $type:ty [$count:ty]) => {
        $name = {
            let _count = <$count>::unpack($reader)? as usize;
            let mut _items = Vec::with_capacity(_count);
            for _ in 0..(_count) {
                _items.push(<$type>::unpack($reader)?);
            }
            $($wrap)*(_items)
        };
    };

    (@read ($($wrap:tt)*) ($reader:expr) ($($attrs:tt)*) $name:ident $type:ty [$count:expr]) => {
        $name = {
            let _count = $count;
            let mut _items = Vec::with_capacity(_count);
            for _ in 0..(_count) {
                _items.push(<$type>::unpack($reader)?);
            }
            $($wrap)*(_items)
        };
    };

    (@attr_size size = $value:expr $(,)?) => {
        $value
    };

    (@attr_size size = $value:expr, $($attrs:tt)*) => {
        $value
    };

    (@attr_size $attr:ident = $value:expr, $($attrs:tt)*) => {
        unpack!(@attr_size $($attrs)*);
    };

    (@attr_size $(,)?) => {
        u32
    };

    (@attr_encoding encoding = $value:expr $(,)?) => {
        $value
    };

    (@attr_encoding encoding = $value:expr, $($attrs:tt)*) => {
        $value
    };

    (@attr_encoding $attr:ident = $value:expr, $($attrs:tt)*) => {
        unpack!(@attr_encoding $($attrs)*);
    };

    (@attr_encoding $(,)?) => {
        Encoding::UTF8
    };
}

pub fn read_record_v1(reader: &mut impl Read, filename: String) -> Result<Record> {
    unpack!(reader,
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
    unpack!(reader,
        offset: u64,
        size: u64,
        uncompressed_size: u64,
        compression_method: u32,
        sha1: Sha1,
    );

    Ok(Record::v2(filename, offset, size, uncompressed_size, compression_method, sha1))
}

pub fn read_record_v3(reader: &mut impl Read, filename: String) -> Result<Record> {
    unpack!(reader,
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
    unpack!(reader,
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

    fn from_file(file: &mut File, options: Options) -> Result<Pak> {
        let mut reader = BufReader::new(file);
        let footer_offset = reader.seek(SeekFrom::End(-44))?;

        unpack!(&mut reader,
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
        let mount_point = read_path(&mut reader, options.encoding)?;

        unpack!(&mut reader, entry_count: u32);

        let mut records = Vec::with_capacity(entry_count as usize);

        for _ in 0..entry_count {
            let filename = read_path(&mut reader, options.encoding)?;
            let record = read_record(&mut reader, filename)?;
            records.push(record);
        }

        let pos = reader.seek(SeekFrom::Current(0))?;
        if pos > footer_offset {
            return Err(Error::new("index bleeds into footer".to_owned()));

        }

        if options.check_integrity {
            panic!("integrity check not implemented")
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

    pub fn version(&self) -> u32 {
        self.version
    }

    pub fn index_offset(&self) -> u64 {
        self.index_offset
    }

    pub fn index_size(&self) -> u64 {
        self.index_size
    }

    pub fn footer_offset(&self) -> u64 {
        self.footer_offset
    }

    pub fn index_sha1(&self) -> &Sha1 {
        &self.index_sha1
    }

    pub fn mount_point(&self) -> &Option<String> {
        &self.mount_point
    }

    pub fn records(&self) -> &[Record] {
        &self.records
    }

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
}
