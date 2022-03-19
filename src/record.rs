// This file is part of rust-u4pak.
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use std::io::{Read, Write};
use std::fmt::Write as FmtWrite;
use aes::BLOCK_SIZE;

use crate::{Error, Result, check::NULL_SHA1, pak::{COMPR_NONE, HexDisplay, Sha1}};
use crate::decode;
use crate::decode::Decode;
use crate::encode;
use crate::encode::Encode;
use crate::pak::V3_RECORD_HEADER_SIZE;
use crate::util::align;

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
    sha1: Option<Sha1>,
    compression_blocks: Option<Vec<CompressionBlock>>,
    encrypted: bool,
    compression_block_size: u32,
}

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
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
        sha1: Option<Sha1>,
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

    pub fn v1(filename: String, offset: u64, size: u64, uncompressed_size: u64, compression_method: u32, timestamp: u64, sha1: Option<Sha1>) -> Self {
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

    pub fn v2(filename: String, offset: u64, size: u64, uncompressed_size: u64, compression_method: u32, sha1: Option<Sha1>) -> Self {
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

    pub fn v3(filename: String, offset: u64, size: u64, uncompressed_size: u64, compression_method: u32, sha1: Option<Sha1>,
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
    pub fn sha1(&self) -> &Option<Sha1> {
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

        Ok(Record::v1(filename, offset, size, uncompressed_size, compression_method, timestamp, Some(sha1)))
    }

    pub fn read_v2(reader: &mut impl Read, filename: String) -> Result<Record> {
        decode!(reader,
            offset: u64,
            size: u64,
            uncompressed_size: u64,
            compression_method: u32, // i32?
            sha1: Sha1,
        );

        Ok(Record::v2(filename, offset, size, uncompressed_size, compression_method, Some(sha1)))
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

        Ok(Record::v3(filename, offset, size, uncompressed_size, compression_method, Some(sha1), compression_blocks, encrypted != 0, compression_block_size))
    }

    pub fn decode_entry(reader: &mut impl Read, filename: String) -> Result<Record> {
        // Bitfield contains information about entry data:
        // 0-5  : Compression block size
        // 6-21 : Compression blocks count
        // 22   : Encrypted
        // 23-28: Compression method
        // 29   : Size 32-bit
        // 30   : Uncompressed size 32-bit
        // 31   : Offset 32-bit
        decode!(reader, bitfield: u32);
        let compression_method = (bitfield >> 23) & 0x3f;
        let offset: u64;
        let uncompressed_size: u64;
        let size: u64;
        let encrypted: bool = (bitfield & (1 << 22)) != 0;
        let compression_block_count: u32 = (bitfield >> 6) & 0xffff;
        let mut compression_block_size: u32 = (bitfield & 0x3f) << 11;
        let mut compression_blocks: Option<Vec<CompressionBlock>> = None;

        // Check if offset is 32 bit safe
        if (bitfield & (1 << 31)) != 0 {
            decode!(reader, x32_offset: u32);
            offset = x32_offset as u64;
        } else {
            decode!(reader, x64_offset: u64);
            offset = x64_offset;
        }

        if (bitfield & (1 << 30)) != 0 {
            decode!(reader, x32_uncompressed_size: u32);
            uncompressed_size = x32_uncompressed_size as u64;
        } else {
            decode!(reader, x64_uncompressed_size: u64);
            uncompressed_size = x64_uncompressed_size;
        }

        if compression_method != COMPR_NONE {
            if (bitfield & (1 << 29)) != 0 {
                decode!(reader, x32_size: u32);
                size = x32_size as u64;
            } else {
                decode!(reader, x64_size: u64);
                size = x64_size;
            }
        } else {
            size = uncompressed_size;
        }

        if compression_block_count > 0 {
            if uncompressed_size <= 0xffff {
                compression_block_size = uncompressed_size as u32;
            };

            if compression_block_count == 1 && !encrypted {
                let start = Record::get_serialized_size(compression_method, compression_block_count);
                compression_blocks = Some(vec![CompressionBlock {
                    start_offset: start,
                    end_offset: start + size
                }]);
            } else if compression_block_count > 0 {
                let mut blocks = vec![];
                let block_alignment = if encrypted {
                    BLOCK_SIZE as u64
                } else {
                    1
                };

                let mut start_offset = Record::get_serialized_size(compression_method, compression_block_count);
                for _ in 0..compression_block_count {
                    decode!(reader, block_size: u32);
                    let end_offset = start_offset + block_size as u64;

                    blocks.push(CompressionBlock {
                        start_offset,
                        end_offset
                    });
                    start_offset += align(block_size as u64, block_alignment)
                }

                compression_blocks = Some(blocks);
            }
        }

        Ok(Self::new(filename, offset, size, uncompressed_size, compression_method, None, None, compression_blocks, encrypted, compression_block_size))
    }

    pub fn read_conan_exiles(reader: &mut impl Read, filename: String) -> Result<Record> {
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
            unknown: u32,
        );

        if unknown != 0 {
            eprintln!("{}: WARNING: unknown field has other value than 0: {}", filename, unknown);
        }

        Ok(Record::v3(filename, offset, size, uncompressed_size, compression_method, Some(sha1), compression_blocks, encrypted != 0, compression_block_size))
    }

    fn get_serialized_size(compression_method: u32, compression_block_count: u32) -> u64 {
        let mut serialized_size = V3_RECORD_HEADER_SIZE;
        if compression_method != COMPR_NONE {
            // Block info * block count
            serialized_size += 16 * compression_block_count as u64 + 4;
        }
        serialized_size
    }

    pub fn write_v1(&self, writer: &mut impl Write) -> Result<()> {
        encode!(writer,
            self.offset,
            self.size,
            self.uncompressed_size,
            self.compression_method,
            self.timestamp.unwrap_or(0),
            self.sha1.as_ref().unwrap_or(&NULL_SHA1),
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
            self.sha1.as_ref().unwrap_or(&NULL_SHA1),
        );
        Ok(())
    }

    pub fn write_v2(&self, writer: &mut impl Write) -> Result<()> {
        encode!(writer,
            self.offset,
            self.size,
            self.uncompressed_size,
            self.compression_method,
            self.sha1.as_ref().unwrap_or(&NULL_SHA1),
        );
        Ok(())
    }

    pub fn write_v2_inline(&self, writer: &mut impl Write) -> Result<()> {
        encode!(writer,
            0u64,
            self.size,
            self.uncompressed_size,
            self.compression_method,
            self.sha1.as_ref().unwrap_or(&NULL_SHA1),
        );
        Ok(())
    }

    pub fn write_v3(&self, writer: &mut impl Write) -> Result<()> {
        encode!(writer,
            self.offset,
            self.size,
            self.uncompressed_size,
            self.compression_method,
            self.sha1.as_ref().unwrap_or(&NULL_SHA1),
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
            self.sha1.as_ref().unwrap_or(&NULL_SHA1),
            if let Some(blocks) = &self.compression_blocks {
                blocks [u32],
            }
            self.encrypted as u8,
            self.compression_block_size,
        );
        Ok(())
    }

    pub fn write_conan_exiles(&self, writer: &mut impl Write) -> Result<()> {
        encode!(writer,
            self.offset,
            self.size,
            self.uncompressed_size,
            self.compression_method,
            self.sha1.as_ref().unwrap_or(&NULL_SHA1),
            if let Some(blocks) = &self.compression_blocks {
                blocks [u32],
            }
            self.encrypted as u8,
            self.compression_block_size,
            0u32,
        );
        Ok(())
    }

    pub fn write_conan_exiles_inline(&self, writer: &mut impl Write) -> Result<()> {
        encode!(writer,
            0u64,
            self.size,
            self.uncompressed_size,
            self.compression_method,
            self.sha1.as_ref().unwrap_or(&NULL_SHA1),
            if let Some(blocks) = &self.compression_blocks {
                blocks [u32],
            }
            self.encrypted as u8,
            self.compression_block_size,
            // there are suppodes to be 20 more bytes of something that I don't know:
            NULL_SHA1,
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
                HexDisplay::new(self.sha1.as_ref().unwrap_or(&NULL_SHA1)),
                HexDisplay::new(other.sha1.as_ref().unwrap_or(&NULL_SHA1)));
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
