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

use std::cmp::Ordering;
use std::convert::TryFrom;

use crate::result::{Result, Error};
use crate::record::Record;

#[derive(Debug)]
pub enum SortKey {
    Name,
    Offset,
    Size,
    ComprMethod,
    UncomprSize,
    RevName,
    RevOffset,
    RevSize,
    RevComprMethod,
    RevUncomprSize,
}

pub type Order = [SortKey];

pub const DEFAULT_ORDER:  [SortKey; 1] = [SortKey::Name];
pub const PHYSICAL_ORDER: [SortKey; 1] = [SortKey::Offset];

impl TryFrom<&str> for SortKey {
    type Error = Error;

    fn try_from(value: &str) -> Result<SortKey> {
        if value.eq_ignore_ascii_case("name") || value.eq_ignore_ascii_case("path") || value.eq_ignore_ascii_case("filename") {
            Ok(SortKey::Name)
        } else if value.eq_ignore_ascii_case("size") || value.eq_ignore_ascii_case("compressed-size") {
            Ok(SortKey::Size)
        } else if value.eq_ignore_ascii_case("offset") {
            Ok(SortKey::Offset)
        } else if value.eq_ignore_ascii_case("compr-method") || value.eq_ignore_ascii_case("compression-method") {
            Ok(SortKey::ComprMethod)
        } else if value.eq_ignore_ascii_case("uncompressed-size") {
            Ok(SortKey::UncomprSize)
        } else if value.eq_ignore_ascii_case("-name") {
            Ok(SortKey::RevName)
        } else if value.eq_ignore_ascii_case("-size") || value.eq_ignore_ascii_case("-compressed-size") {
            Ok(SortKey::RevSize)
        } else if value.eq_ignore_ascii_case("-offset") {
            Ok(SortKey::RevOffset)
        } else if value.eq_ignore_ascii_case("-compr-method") || value.eq_ignore_ascii_case("-compression-method") {
            Ok(SortKey::RevComprMethod)
        } else if value.eq_ignore_ascii_case("-uncompressed-size") {
            Ok(SortKey::RevUncomprSize)
        } else {
            Err(Error::new(format!("illegal argument --sort={:?}", value)))
        }
    }
}

impl SortKey {
    #[inline]
    pub fn to_cmp(&self) -> impl Fn(&Record, &Record) -> Ordering {
        match self {
            SortKey::Name           => |a: &Record, b: &Record| a.filename().cmp(&b.filename()),
            SortKey::Size           => |a: &Record, b: &Record| a.size().cmp(&b.size()),
            SortKey::Offset         => |a: &Record, b: &Record| a.offset().cmp(&b.offset()),
            SortKey::ComprMethod    => |a: &Record, b: &Record| a.compression_method().cmp(&b.compression_method()),
            SortKey::UncomprSize    => |a: &Record, b: &Record| a.uncompressed_size().cmp(&b.uncompressed_size()),

            SortKey::RevName        => |a: &Record, b: &Record| b.filename().cmp(&a.filename()),
            SortKey::RevSize        => |a: &Record, b: &Record| b.size().cmp(&a.size()),
            SortKey::RevOffset      => |a: &Record, b: &Record| b.offset().cmp(&a.offset()),
            SortKey::RevComprMethod => |a: &Record, b: &Record| b.compression_method().cmp(&a.compression_method()),
            SortKey::RevUncomprSize => |a: &Record, b: &Record| b.uncompressed_size().cmp(&a.uncompressed_size()),
        }
    }
}

fn chain<'a>(cmp1: Box<dyn Fn(&Record, &Record) -> Ordering>, cmp2: Box<dyn Fn(&Record, &Record) -> Ordering>) -> Box<dyn Fn(&Record, &Record) -> Ordering> {
    Box::new(move |a: &Record, b: &Record|
        match cmp1(a, b) {
            Ordering::Equal => cmp2(a, b),
            ord => ord,
        }
    )
}

fn make_chain(cmp1: Box<dyn Fn(&Record, &Record) -> Ordering>, mut iter: std::slice::Iter<SortKey>) -> Box<dyn Fn(&Record, &Record) -> Ordering> {
    if let Some(key) = iter.next() {
        make_chain(chain(cmp1, Box::new(key.to_cmp())), iter)
    } else {
        cmp1
    }
}

pub fn sort(list: &mut Vec<Record>, order: &Order) {
    let mut iter = order.iter();

    if let Some(first_key) = iter.next() {
        let cmp = make_chain(Box::new(first_key.to_cmp()), iter);
        list.sort_by(cmp);
    }
}

pub fn parse_order(value: &str) -> Result<Vec<SortKey>> {
    let mut order = Vec::new();
    for key in value.split(',') {
        order.push(SortKey::try_from(key)?);
    }
    Ok(order)
}
