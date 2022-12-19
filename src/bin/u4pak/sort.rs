// This file is part of rust-u4pak.
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use std::cmp::Ordering;
use std::convert::TryFrom;

use u4pak::record::Record;
use u4pak::result::{Error, Result};

#[derive(Debug)]
pub enum SortKey {
    Name,
    Offset,
    Size,
    ComprMethod,
    UncomprSize,
    ComprBlockSize,
    Timestamp,
    Encrypted,

    RevName,
    RevOffset,
    RevSize,
    RevComprMethod,
    RevUncomprSize,
    RevComprBlockSize,
    RevTimestamp,
    RevEncrypted,
}

pub type Order = [SortKey];

pub const DEFAULT_ORDER: [SortKey; 1] = [SortKey::Name];
pub const PHYSICAL_ORDER: [SortKey; 1] = [SortKey::Offset];

impl TryFrom<&str> for SortKey {
    type Error = Error;

    fn try_from(value: &str) -> Result<SortKey> {
        if value.eq_ignore_ascii_case("p")
            || value.eq_ignore_ascii_case("name")
            || value.eq_ignore_ascii_case("path")
            || value.eq_ignore_ascii_case("filename")
        {
            Ok(SortKey::Name)
        } else if value.eq_ignore_ascii_case("s")
            || value.eq_ignore_ascii_case("size")
            || value.eq_ignore_ascii_case("compressed-size")
        {
            Ok(SortKey::Size)
        } else if value.eq_ignore_ascii_case("o") || value.eq_ignore_ascii_case("offset") {
            Ok(SortKey::Offset)
        } else if value.eq_ignore_ascii_case("c")
            || value.eq_ignore_ascii_case("compr-method")
            || value.eq_ignore_ascii_case("compression-method")
        {
            Ok(SortKey::ComprMethod)
        } else if value.eq_ignore_ascii_case("u") || value.eq_ignore_ascii_case("uncompressed-size")
        {
            Ok(SortKey::UncomprSize)
        } else if value.eq_ignore_ascii_case("b")
            || value.eq_ignore_ascii_case("compression-block-size")
        {
            Ok(SortKey::ComprBlockSize)
        } else if value.eq_ignore_ascii_case("t") || value.eq_ignore_ascii_case("timestamp") {
            Ok(SortKey::Timestamp)
        } else if value.eq_ignore_ascii_case("e") || value.eq_ignore_ascii_case("encrypted") {
            Ok(SortKey::Encrypted)
        } else if value.eq_ignore_ascii_case("-p")
            || value.eq_ignore_ascii_case("-name")
            || value.eq_ignore_ascii_case("-path")
            || value.eq_ignore_ascii_case("-filename")
        {
            Ok(SortKey::RevName)
        } else if value.eq_ignore_ascii_case("-s")
            || value.eq_ignore_ascii_case("-size")
            || value.eq_ignore_ascii_case("-compressed-size")
        {
            Ok(SortKey::RevSize)
        } else if value.eq_ignore_ascii_case("-o") || value.eq_ignore_ascii_case("-offset") {
            Ok(SortKey::RevOffset)
        } else if value.eq_ignore_ascii_case("-c")
            || value.eq_ignore_ascii_case("-compr-method")
            || value.eq_ignore_ascii_case("-compression-method")
        {
            Ok(SortKey::RevComprMethod)
        } else if value.eq_ignore_ascii_case("-u")
            || value.eq_ignore_ascii_case("-uncompressed-size")
        {
            Ok(SortKey::RevUncomprSize)
        } else if value.eq_ignore_ascii_case("-b")
            || value.eq_ignore_ascii_case("-compression-block-size")
        {
            Ok(SortKey::RevComprBlockSize)
        } else if value.eq_ignore_ascii_case("-t") || value.eq_ignore_ascii_case("-timestamp") {
            Ok(SortKey::RevTimestamp)
        } else if value.eq_ignore_ascii_case("-e") || value.eq_ignore_ascii_case("-encrypted") {
            Ok(SortKey::RevEncrypted)
        } else {
            Err(Error::new(format!("illegal argument --sort={:?}", value)))
        }
    }
}

impl SortKey {
    #[inline]
    pub fn to_cmp(&self) -> impl Fn(&Record, &Record) -> Ordering {
        match self {
            SortKey::Name => |a: &Record, b: &Record| a.filename().cmp(b.filename()),
            SortKey::Size => |a: &Record, b: &Record| a.size().cmp(&b.size()),
            SortKey::Offset => |a: &Record, b: &Record| a.offset().cmp(&b.offset()),
            SortKey::ComprMethod => {
                |a: &Record, b: &Record| a.compression_method().cmp(&b.compression_method())
            }
            SortKey::UncomprSize => {
                |a: &Record, b: &Record| a.uncompressed_size().cmp(&b.uncompressed_size())
            }
            SortKey::ComprBlockSize => {
                |a: &Record, b: &Record| a.compression_block_size().cmp(&b.compression_block_size())
            }
            SortKey::Timestamp => |a: &Record, b: &Record| a.timestamp().cmp(&b.timestamp()),
            SortKey::Encrypted => |a: &Record, b: &Record| a.encrypted().cmp(&b.encrypted()),

            SortKey::RevName => |a: &Record, b: &Record| b.filename().cmp(a.filename()),
            SortKey::RevSize => |a: &Record, b: &Record| b.size().cmp(&a.size()),
            SortKey::RevOffset => |a: &Record, b: &Record| b.offset().cmp(&a.offset()),
            SortKey::RevComprMethod => {
                |a: &Record, b: &Record| b.compression_method().cmp(&a.compression_method())
            }
            SortKey::RevUncomprSize => {
                |a: &Record, b: &Record| b.uncompressed_size().cmp(&a.uncompressed_size())
            }
            SortKey::RevComprBlockSize => {
                |a: &Record, b: &Record| b.compression_block_size().cmp(&a.compression_block_size())
            }
            SortKey::RevTimestamp => |a: &Record, b: &Record| b.timestamp().cmp(&a.timestamp()),
            SortKey::RevEncrypted => |a: &Record, b: &Record| b.encrypted().cmp(&a.encrypted()),
        }
    }
}

fn chain(
    cmp1: Box<dyn Fn(&Record, &Record) -> Ordering>,
    cmp2: Box<dyn Fn(&Record, &Record) -> Ordering>,
) -> Box<dyn Fn(&Record, &Record) -> Ordering> {
    Box::new(move |a: &Record, b: &Record| match cmp1(a, b) {
        Ordering::Equal => cmp2(a, b),
        ord => ord,
    })
}

fn make_chain(
    cmp1: Box<dyn Fn(&Record, &Record) -> Ordering>,
    mut iter: std::slice::Iter<SortKey>,
) -> Box<dyn Fn(&Record, &Record) -> Ordering> {
    if let Some(key) = iter.next() {
        make_chain(chain(cmp1, Box::new(key.to_cmp())), iter)
    } else {
        cmp1
    }
}

pub fn sort(list: &mut Vec<impl AsRef<Record>>, order: &Order) {
    let mut iter = order.iter();

    if let Some(first_key) = iter.next() {
        let cmp = make_chain(Box::new(first_key.to_cmp()), iter);
        list.sort_by(|a, b| cmp(a.as_ref(), b.as_ref()));
    }
}

pub fn parse_order(value: &str) -> Result<Vec<SortKey>> {
    let mut order = Vec::new();
    for key in value.split(',') {
        order.push(SortKey::try_from(key)?);
    }
    Ok(order)
}
