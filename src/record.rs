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

use std::io::{Read, Write};
use std::fmt::Write as FmtWrite;

use crate::pak::{COMPR_NONE, HexDisplay, Sha1};
use crate::decode;
use crate::decode::Decode;
use crate::encode;
use crate::encode::Encode;
use crate::Result;

macro_rules! cmp_record_field {
    ($buf:expr, $field:ident, $r1:expr, $r2:expr) => {
        if $r1.$field != $r2.$field {
            let _ = write!($buf, "\t{}: {:?} != {:?}\n", stringify!($field), $r1.$field, $r2.$field);
        }
    };
}

#[derive(Debug, PartialEq)]
pub struct Record {
    filename: String,
    offset: u64,
    size: u64,
    uncompressed_size: u64,
    compression_method: u32,
    timestamp: Option<u64>,
    sha1: Sha1,
    compression_blocks: Option<Vec<CompressionBlock>>,
    encrypted: bool,
    compression_block_size: u32,
}

#[derive(Debug, PartialEq)]
pub struct CompressionBlock {
    pub start_offset: u64,
    pub end_offset: u64,
}

impl Record {
    #[inline]
    pub(crate) fn new(
        filename: String,
        offset: u64,
        size: u64,
        uncompressed_size: u64,
        compression_method: u32,
        timestamp: Option<u64>,
        sha1: Sha1,
        compression_blocks: Option<Vec<CompressionBlock>>,
        encrypted: bool,
        compression_block_size: u32,
    ) -> Self {
        Self {
            filename,
            offset,
            size,
            uncompressed_size,
            compression_method,
            timestamp,
            sha1,
            compression_blocks,
            encrypted,
            compression_block_size,
        }
    }

    pub fn v1(filename: String, offset: u64, size: u64, uncompressed_size: u64, compression_method: u32, timestamp: u64, sha1: Sha1) -> Self {
        Self {
            filename,
            offset,
            size,
            uncompressed_size,
            compression_method,
            timestamp: Some(timestamp),
            sha1,
            compression_blocks: None,
            encrypted: false,
            compression_block_size: 0,
        }
    }

    pub fn v2(filename: String, offset: u64, size: u64, uncompressed_size: u64, compression_method: u32, sha1: Sha1) -> Self {
        Self {
            filename,
            offset,
            size,
            uncompressed_size,
            compression_method,
            timestamp: None,
            sha1,
            compression_blocks: None,
            encrypted: false,
            compression_block_size: 0,
        }
    }

    pub fn v3(filename: String, offset: u64, size: u64, uncompressed_size: u64, compression_method: u32, sha1: Sha1,
              compression_blocks: Option<Vec<CompressionBlock>>, encrypted: bool, compression_block_size: u32) -> Self {
        Self {
            filename,
            offset,
            size,
            uncompressed_size,
            compression_method,
            timestamp: None,
            sha1,
            compression_blocks,
            encrypted,
            compression_block_size,
        }
    }

    #[inline]
    pub fn filename(&self) -> &str {
        &self.filename
    }

    #[inline]
    pub fn offset(&self) -> u64 {
        self.offset
    }

    #[inline]
    pub fn size(&self) -> u64 {
        self.size
    }

    #[inline]
    pub fn uncompressed_size(&self) -> u64 {
        self.uncompressed_size
    }

    #[inline]
    pub fn compression_method(&self) -> u32 {
        self.compression_method
    }

    #[inline]
    pub fn timestamp(&self) -> Option<u64> {
        self.timestamp
    }

    #[inline]
    pub fn sha1(&self) -> &Sha1 {
        &self.sha1
    }

    #[inline]
    pub fn compression_blocks(&self) -> &Option<Vec<CompressionBlock>> {
        &self.compression_blocks
    }

    #[inline]
    pub fn encrypted(&self) -> bool {
        self.encrypted
    }

    #[inline]
    pub fn compression_block_size(&self) -> u32 {
        self.compression_block_size
    }

    pub fn read_v1(reader: &mut impl Read, filename: String) -> Result<Record> {
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

    pub fn read_v2(reader: &mut impl Read, filename: String) -> Result<Record> {
        decode!(reader,
            offset: u64,
            size: u64,
            uncompressed_size: u64,
            compression_method: u32,
            sha1: Sha1,
        );

        Ok(Record::v2(filename, offset, size, uncompressed_size, compression_method, sha1))
    }

    pub fn read_v3(reader: &mut impl Read, filename: String) -> Result<Record> {
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

    pub fn write_v1(&self, writer: &mut impl Write) -> Result<()> {
        encode!(writer,
            self.offset,
            self.size,
            self.uncompressed_size,
            self.compression_method,
            self.timestamp.unwrap_or(0),
            self.sha1,
        );
        Ok(())
    }

    pub fn write_v1_inline(&self, writer: &mut impl Write) -> Result<()> {
        encode!(writer,
            0u64,
            self.size,
            self.uncompressed_size,
            self.compression_method,
            self.timestamp.unwrap_or(0),
            self.sha1,
        );
        Ok(())
    }

    pub fn write_v2(&self, writer: &mut impl Write) -> Result<()> {
        encode!(writer,
            self.offset,
            self.size,
            self.uncompressed_size,
            self.compression_method,
            self.sha1,
        );
        Ok(())
    }

    pub fn write_v2_inline(&self, writer: &mut impl Write) -> Result<()> {
        encode!(writer,
            0u64,
            self.size,
            self.uncompressed_size,
            self.compression_method,
            self.sha1,
        );
        Ok(())
    }

    pub fn write_v3(&self, writer: &mut impl Write) -> Result<()> {
        encode!(writer,
            self.offset,
            self.size,
            self.uncompressed_size,
            self.compression_method,
            self.sha1,
            if let Some(blocks) = &self.compression_blocks {
                blocks [u32],
            }
            self.encrypted as u8,
            self.compression_block_size,
        );
        Ok(())
    }

    pub fn write_v3_inline(&self, writer: &mut impl Write) -> Result<()> {
        encode!(writer,
            0u64,
            self.size,
            self.uncompressed_size,
            self.compression_method,
            self.sha1,
            if let Some(blocks) = &self.compression_blocks {
                blocks [u32],
            }
            self.encrypted as u8,
            self.compression_block_size,
        );
        Ok(())
    }

    pub fn same_metadata(&self, other: &Record) -> bool {
        // compare all metadata except for the filename
        // data records always have offset == 0 it seems, so skip that
        self.size                   == other.size               &&
        self.uncompressed_size      == other.uncompressed_size  &&
        self.compression_method     == other.compression_method &&
        self.timestamp              == other.timestamp          &&
        self.sha1                   == other.sha1               &&
        self.compression_blocks     == other.compression_blocks &&
        self.encrypted              == other.encrypted          &&
        self.compression_block_size == other.compression_block_size
    }

    pub fn metadata_diff(&self, other: &Record) -> String {
        let mut buf = String::new();

        cmp_record_field!(buf, size,                   self, other);
        cmp_record_field!(buf, uncompressed_size,      self, other);
        cmp_record_field!(buf, timestamp,              self, other);
        cmp_record_field!(buf, encrypted,              self, other);
        cmp_record_field!(buf, compression_block_size, self, other);

        if self.sha1 != other.sha1 {
            let _ = write!(buf, "\tsha1: {} != {}",
                HexDisplay::new(&self.sha1),
                HexDisplay::new(&other.sha1));
        }

        if self.compression_blocks != other.compression_blocks {
            let _ = write!(buf, "\tcompression_blocks:\n\t\t{:?}\n\t\t\t!=\n\t\t{:?}",
                self.compression_blocks,
                other.compression_blocks);
        }

        buf
    }

    pub(crate) fn move_to(&mut self, version: u32, new_offset: u64) {
        if version < 7 {
            if let Some(blocks) = &mut self.compression_blocks {
                for block in blocks {
                    block.start_offset = (block.start_offset - self.offset) + new_offset;
                    block.end_offset   = (block.end_offset   - self.offset) + new_offset
                }
            }
        }
        self.offset = new_offset;
    }
}

impl AsRef<Record> for Record {
    fn as_ref(&self) -> &Record {
        &self
    }
}
