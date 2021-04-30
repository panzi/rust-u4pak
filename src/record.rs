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

use crate::pak::{COMPRESSION_BLOCK_HEADER_SIZE, COMPR_NONE, Sha1, V1_RECORD_HEADER_SIZE, V2_RECORD_HEADER_SIZE, V3_RECORD_HEADER_SIZE};
use crate::decode;
use crate::decode::Decode;
use crate::encode;
use crate::encode::Encode;
use crate::Result;

#[derive(Debug)]
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

#[derive(Debug)]
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

    pub fn v4(filename: String, offset: u64, size: u64, uncompressed_size: u64, compression_method: u32, sha1: Sha1,
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

    pub fn read_v4(reader: &mut impl Read, filename: String) -> Result<Record> {
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

    pub fn write_v1(&self, writer: &mut impl Write) -> Result<u64> {
        encode!(writer,
            self.offset,
            self.size,
            self.uncompressed_size,
            self.compression_method,
            self.timestamp.unwrap_or(0),
            self.sha1,
        );
        Ok(V1_RECORD_HEADER_SIZE)
    }

    pub fn write_v2(&self, writer: &mut impl Write) -> Result<u64> {
        encode!(writer,
            self.offset,
            self.size,
            self.uncompressed_size,
            self.compression_method,
            self.sha1,
        );
        Ok(V2_RECORD_HEADER_SIZE)
    }

    pub fn write_v3(&self, writer: &mut impl Write) -> Result<u64> {
        let mut size: u64 = V3_RECORD_HEADER_SIZE;
        if let Some(blocks) = &self.compression_blocks {
            size += blocks.len() as u64 * COMPRESSION_BLOCK_HEADER_SIZE;
        }
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
        Ok(size)
    }
}

impl AsRef<Record> for Record {
    fn as_ref(&self) -> &Record {
        &self
    }
}
