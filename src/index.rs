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

use crate::decode;
use crate::decode::Decode;
use crate::decrypt::decrypt;
use crate::Variant;
use crate::{Error, Record, Result};

use std::convert::TryFrom;
use std::io::{Cursor, Read, Seek, SeekFrom};
use log::{debug, error, trace, warn};

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
pub struct IndexLoadParams {
    keep_full_directory: bool,
    validate_pruning: bool,
    delay_pruning: bool,
    write_path_hash: bool,
    write_full_directory_index: bool,
}

#[derive(Debug, Default)]
pub struct SecondaryIndexInfo {
    has_path_hash_index: bool,
    path_hash_index_offset: i64,
    path_hash_index_size: i64,
    has_full_directory_index: bool,
    full_directory_index_offset: i64,
    full_directory_index_size: i64,
    encoded_record_info: Vec<u8>,
}

#[derive(Debug)]
pub struct Index {
    mount_point: Option<String>,
    records: Vec<Record>,
}

impl Index {
    pub(crate) fn new(mount_point: Option<String>, records: Vec<Record>) -> Self {
        Self {
            mount_point,
            records,
        }
    }
    pub fn read<R>(
        reader: &mut R,
        index_size: usize,
        version: u32,
        variant: Variant,
        encoding: Encoding,
        encryption_key: Option<Vec<u8>>,
    ) -> Result<Self> 
    where
        R: Read,
        R: Seek,
    {
        let mut index_buff = vec![0; index_size as usize];
        reader.read_exact(&mut index_buff)?;
        if let Some(encryption_key) = encryption_key.clone() {
            decrypt(&mut index_buff, encryption_key)
        }

        let decrypted_index = &mut Cursor::new(index_buff);

        let mount_point = read_path(decrypted_index, encoding)?;
        let records;
        if version < 10 {
            records = read_records_legacy(decrypted_index, version, variant, encoding)
                .expect("Failed to read index records");
        } else {
            if let Ok((index_info, mut r)) = read_records(decrypted_index, encoding) {
                if let Ok(mut sec_records) = read_secondary_index_records(reader, &index_info, encryption_key, encoding) {
                    r.append(&mut sec_records);
                }

                records = r;
            } else {
                return Err(Error::new(format!(
                    "Only know how to handle Conan Exile paks of version 4, but version was {}.",
                    version
                )));
            }
        };

        Ok(Self {
            mount_point: if mount_point.is_empty() { None } else { Some(mount_point) },
            records,
        })
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
    pub fn into_records<'a>(self) -> Vec<Record> {
        self.records
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

pub fn read_records_legacy(
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
            _ if version <= 5 || version <= 9 => Record::read_v3,
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

pub fn read_records(
    reader: &mut impl Read,
    encoding: Encoding,
) -> Result<(SecondaryIndexInfo, Vec<Record>)> {
    decode!(
        reader,
        entry_count: i32,
        path_hash_seed: u64,
        has_path_hash_index: u32
    );

    let mut secondary_index_info = SecondaryIndexInfo::default();
    secondary_index_info.has_path_hash_index = has_path_hash_index != 0;

    if secondary_index_info.has_path_hash_index {
        decode!(
            reader,
            path_hash_index_offset: i64,
            path_hash_index_size: i64,
            path_hash_index_hash: [u8; 20]
        );

        secondary_index_info.has_path_hash_index = path_hash_index_size != -1;
        secondary_index_info.path_hash_index_offset = path_hash_index_offset;
        secondary_index_info.path_hash_index_size = path_hash_index_size;
    }
    decode!(reader, has_full_directory_index: u32);
    secondary_index_info.has_full_directory_index = has_full_directory_index != 0;

    if secondary_index_info.has_full_directory_index {
        decode!(
            reader,
            full_directory_index_offset: i64,
            full_directory_index_size: i64,
            full_directory_index_hash: [u8; 20]
        );
        secondary_index_info.has_full_directory_index = full_directory_index_size != -1;
        secondary_index_info.full_directory_index_offset = full_directory_index_offset;
        secondary_index_info.full_directory_index_size = full_directory_index_size;
    }
    decode!(reader, pak_entries_size: i32);
    let mut pak_entries = vec![0u8; pak_entries_size as usize];
    reader.read_exact(&mut pak_entries)?;
    secondary_index_info.encoded_record_info = pak_entries;

    decode!(reader, file_count: u32);
    let mut records = Vec::with_capacity(file_count as usize);
    for _ in 0..file_count {
        let filename = read_path(reader, encoding)?;
        let record = Record::read_v3(reader, filename)?;
        records.push(record);
    }

    Ok((secondary_index_info, records))
}

fn read_secondary_index_records<R>(
    reader: &mut R,
    index_info: &SecondaryIndexInfo,
    encryption_key: Option<Vec<u8>>,
    encoding: Encoding,
) -> Result<Vec<Record>> where
    R: Read,
    R: Seek,
{
    debug!("Reading secondary index");

    let mut records = vec![];
    let mut encoded_record_info = Cursor::new(&index_info.encoded_record_info[..]);
    if index_info.has_full_directory_index {
        debug!("Reading full directory index");
        let mut full_directory_index_data =
            vec![0u8; index_info.full_directory_index_size as usize];
        if let Err(err) = reader.seek(SeekFrom::Start(
            index_info.full_directory_index_offset as u64,
        )) {
            error!("Failed to load fill directory index: {}", err);
            return Err(Error::from(err));
        }
        if let Err(err) = reader.read_exact(&mut full_directory_index_data) {
            error!("Failed to read full directory index: {}", err);
            return Err(Error::from(err));
        }

        if let Some(key) = encryption_key {
            decrypt(&mut full_directory_index_data, key);
        }

        let mut index_buff = &full_directory_index_data[..];
        decode!(&mut index_buff, dir_count: u32);
        for i in 0..dir_count {
            let path = read_path(&mut index_buff, encoding);
            decode!(&mut index_buff, file_count: u32);
            let mut file_path = String::new();
            if let Ok(p) = path {
                trace!("Reading {} files from directory {}", file_count, p);
                if p != "/" {
                    file_path.push_str(&p);
                }
            } else {
                warn!("Failed to resolve path for file {}. Skipping.", i);
                continue;
            }

            for _ in 0..file_count {
                let file_name = read_path(&mut index_buff, encoding);
                decode!(&mut index_buff, entry: u32);

                if let Ok(name) = file_name {
                    let mut p = file_path.clone();
                    p.push_str(&name);

                    encoded_record_info.seek(SeekFrom::Start(entry as u64));
                    trace!("Decoding file {} from location {}", p, entry);
                    if let Ok(record) = Record::decode_entry(&mut encoded_record_info, p.clone()) {
                        records.push(record);
                    } else {
                        warn!("Failed to read record for file {}. Skipping.", p);
                    }
                } else {
                    warn!("Failed to resolve name for file {} in folder {}. Skipping.", i, file_path);
                    continue;
                }
            }
        }
    } else if index_info.has_path_hash_index {
        warn!("Hash index is used as no full directory index was found. Filenames and paths can not be restored using this index!");
        debug!("Reading path hash index from {} with size {}", index_info.path_hash_index_offset, index_info.path_hash_index_size);
        let mut path_hash_index_data =
            vec![0u8; index_info.path_hash_index_size as usize];
        if let Err(err) = reader.seek(SeekFrom::Start(
            index_info.path_hash_index_offset as u64,
        )) {
            error!("Failed to load fill directory index: {}", err);
            return Err(Error::from(err));
        }
        if let Err(err) = reader.read_exact(&mut path_hash_index_data) {
            error!("Failed to read full directory index: {}", err);
            return Err(Error::from(err));
        }

        if let Some(key) = encryption_key {
            decrypt(&mut path_hash_index_data, key);
        }

        let mut index_buff = &path_hash_index_data[..];
        decode!(&mut index_buff, file_count: u32);
        debug!("Found {} files in hash index", file_count);
        for _ in 0..file_count {
            decode!(&mut index_buff, hash: u64, entry: u32);
            
            encoded_record_info.seek(SeekFrom::Start(entry as u64));
            trace!("Decoding file {:x} from location {}", hash, entry);
            if let Ok(record) = Record::decode_entry(&mut encoded_record_info, format!("{:x}", hash)) {
                records.push(record);
            } else {
                warn!("Failed to read record for file {:x}. Skipping.", hash);
            }
        }
    } else {
        warn!("Neither full direcotry nor hash index found! Files are probably missing!");
    }

    debug!("Read {} records from secondary index", records.len());

    Ok(records)
}
