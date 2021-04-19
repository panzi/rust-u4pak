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

use std::io::Write;

use crate::sort::{sort, Order};
use crate::util::{format_size, print_table, Align::*};
use crate::result::Result;
use crate::record::Record;
use crate::pak::{Pak, compression_method_name, format_sha1};

#[derive(Debug, PartialEq)]
pub enum ListStyle {
    Table { human_readable: bool },
    OnlyNames { null_separated: bool },
}

pub struct ListOptions<'a> {
    pub order: Option<&'a Order>,
    pub style: ListStyle,
    pub filter: Option<&'a [&'a str]>,
}

impl ListOptions<'_> {
    #[inline]
    pub fn new() -> Self {
        ListOptions::default()
    }
}

impl Default for ListStyle {
    #[inline]
    fn default() -> Self {
        ListStyle::Table { human_readable: false }
    }
}

impl Default for ListOptions<'_> {
    #[inline]
    fn default() -> Self {
        Self {
            order: None,
            style: ListStyle::default(),
            filter: None,
        }
    }
}

pub fn list(pak: Pak, options: ListOptions) -> Result<()> {
    if let Some(order) = options.order {
        let mut records = pak.into_records();

        sort(&mut records, order);
        list_records(&records, options)
    } else {
        list_records(pak.records(), options)
    }
}

pub fn list_records(records: &[Record], options: ListOptions) -> Result<()> {
    match options.style {
        ListStyle::Table { human_readable } => {
            let mut table: Vec<Vec<String>> = Vec::new();

            let fmt_size = if human_readable {
                |size: u64| format_size(size)
            } else {
                |size: u64| format!("{}", size)
            };

            for record in records {
                table.push(vec![
                    format!("{}", record.offset()),
                    fmt_size(record.uncompressed_size()),
                    compression_method_name(record.compression_method()).to_owned(),
                    fmt_size(record.size()),
                    format_sha1(record.sha1()),
                    record.filename().to_owned(),
                ]);
            }

            print_table(
                &["Offset", "Size", "Compr-Method", "Compr-Size", "SHA1", "Filename"],
                 &[Right,    Right,  Left,           Right,        Right,  Left],
                &table);
        }
        ListStyle::OnlyNames { null_separated } => {
            let sep = [if null_separated { 0 } else { '\n' as u8 }];
            let mut stdout = std::io::stdout();
            for record in records {
                stdout.write_all(record.filename().as_bytes())?;
                stdout.write_all(&sep)?;
            }
        }
    }

    Ok(())
}
