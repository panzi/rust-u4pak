use crate::decode;
use crate::decode::Decode;
use crate::decrypt::decrypt;
use crate::Variant;
use crate::{Error, Record, Result};

use std::convert::TryFrom;
use std::io::Cursor;
use std::io::Read;

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
                            *byte
                        )));
                    }
                }
                Ok(buffer.into_iter().map(|byte| byte as char).collect())
            }
            Encoding::Latin1 => Ok(buffer.into_iter().map(|byte| byte as char).collect()),
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
        } else if encoding.eq_ignore_ascii_case("latin1")
            || encoding.eq_ignore_ascii_case("iso-8859-1")
        {
            Ok(Encoding::Latin1)
        } else {
            Err(Error::new(format!("unsupported encoding: {:?}", encoding)))
        }
    }
}

#[derive(Debug)]
pub struct Index {
    pub mount_point: Result<String>,
    pub records: Vec<Record>,
}

impl Index {
    pub(crate) fn new(mount_point: Result<String>, records: Vec<Record>) -> Self {
        Self {
            mount_point,
            records,
        }
    }
    pub fn read(
        data: &mut Vec<u8>,
        version: u32,
        variant: Variant,
        encoding: Encoding,
        encryption_key: Option<&str>,
    ) -> Self {
        if encryption_key.is_some() {
            decrypt(data, encryption_key.unwrap());
        }

        let decrypted_index = &mut Cursor::new(data);

        let mount_point = read_path(decrypted_index, encoding);
        let records = read_records(decrypted_index, version, variant, encoding)
            .expect("Failed to read index records");

        Self {
            mount_point,
            records,
        }
    }
}

pub fn read_path(reader: &mut impl Read, encoding: Encoding) -> Result<String> {
    let mut buf = [0; 4];
    reader.read_exact(&mut buf)?;
    let size = i32::from_le_bytes(buf);

    if size < 0 {
        let utf16_size = -(size as isize) as usize;
        let mut buf = vec![0u8; 2 * utf16_size];
        reader.read_exact(&mut buf)?;

        let mut utf16 = Vec::with_capacity(utf16_size);
        let mut index = 0usize;
        while index < buf.len() {
            let bytes = [buf[index], buf[index + 1]];
            utf16.push(u16::from_le_bytes(bytes));
            index += 2;
        }

        if let Some(index) = utf16.iter().position(|&ch| ch == 0) {
            utf16.truncate(index);
        }

        return Ok(String::from_utf16(&utf16)?);
    }

    let mut buf = vec![0u8; size as usize];
    reader.read_exact(&mut buf)?;
    if let Some(index) = buf.iter().position(|&byte| byte == 0) {
        buf.truncate(index);
    }

    encoding.parse_vec(buf)
}

pub fn read_records(
    reader: &mut impl Read,
    version: u32,
    variant: Variant,
    encoding: Encoding,
) -> Result<Vec<Record>> {
    let read_record = match variant {
        Variant::ConanExiles => {
            if version != 4 {
                return Err(Error::new(format!(
                    "Only know how to handle Conan Exile paks of version 4, but version was {}.",
                    version
                )));
            }
            Record::read_conan_exiles
        }
        Variant::Standard => match version {
            1 => Record::read_v1,
            2 => Record::read_v2,
            _ if version <= 5 || version == 7 || version == 9 => Record::read_v3,
            _ => {
                return Err(Error::new(format!("unsupported version: {}", version)));
            }
        },
    };

    decode!(reader, entry_count: u32);

    let mut records = Vec::with_capacity(entry_count as usize);

    for _ in 0..entry_count {
        let filename = read_path(reader, encoding)?;
        let record = read_record(reader, filename)?;
        records.push(record);
    }

    Ok(records)
}
