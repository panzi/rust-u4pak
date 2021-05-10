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

use chrono::NaiveDateTime;

use crate::{Filter, sort::{sort, Order}};
use crate::util::{format_size, print_table, Align::*};
use crate::result::Result;
use crate::record::Record;
use crate::pak::{Pak, compression_method_name, HexDisplay};

#[derive(Debug, PartialEq)]
pub enum ListStyle {
    Table { human_readable: bool },
    OnlyNames { null_separated: bool },
}

pub struct ListOptions<'a> {
    pub order: Option<&'a Order>,
    pub style: ListStyle,
    pub paths: Option<&'a [&'a str]>,
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
            paths: None,
        }
    }
}

pub fn list(pak: Pak, options: ListOptions) -> Result<()> {
    let version = pak.version();
    match (options.order, options.paths) {
        (Some(order), Some(paths)) => {
            let mut filter = Filter::from_paths(paths.iter().cloned());
            let mut records = pak.records()
                .iter()
                .filter(|record| filter.visit(record.filename()))
                .collect();

            sort(&mut records, order);
            list_records(version, &records, options)?;
            filter.assert_all_visited()?;
        }
        (Some(order), None) => {
            let mut records = pak.into_records();

            sort(&mut records, order);
            list_records(version, &records, options)?;
        }
        (None, Some(paths)) => {
            let mut filter = Filter::from_paths(paths.iter().cloned());
            let records = pak.records()
                .iter()
                .filter(|record| filter.visit(record.filename()))
                .collect::<Vec<_>>();

            list_records(version, &records, options)?;
            filter.assert_all_visited()?;
        }
        (None, None) => {
            list_records(version, pak.records(), options)?;
        }
    }

    Ok(())
}

fn list_records(version: u32, records: &[impl AsRef<Record>], options: ListOptions) -> Result<()> {
    match options.style {
        ListStyle::Table { human_readable } => {
            let mut table: Vec<Vec<String>> = Vec::new();

            let fmt_size = if human_readable {
                |size: u64| format_size(size)
            } else {
                |size: u64| format!("{}", size)
            };

            for record in records {
                let record = record.as_ref();
                let mut row = vec![
                    format!("{}", record.offset()),
                    fmt_size(record.uncompressed_size()),
                    fmt_size(record.size()),
                    compression_method_name(record.compression_method()).to_owned(),
                    fmt_size(record.compression_block_size() as u64),
                ];
                if version == 1 {
                    if let Some(timestamp) = record.timestamp() {
                        if let Some(timestamp) = NaiveDateTime::from_timestamp_opt(timestamp as i64, 0) {
                            row.push(timestamp.format("%Y-%m-%d %H:%M:%S").to_string());
                        } else {
                            row.push("-".to_string());
                        }
                    } else {
                        row.push("-".to_string());
                    }
                }
                row.push(HexDisplay::new(record.sha1()).to_string());
                row.push(record.filename().to_owned());
                table.push(row);
            }

            if version == 1 {
                print_table(
                    &["Offset", "Size", "Compr-Size", "Compr-Method", "Compr-Block-Size", "Timestamp", "SHA-1", "Filename"],
                    &[Right,    Right,  Right,        Left,           Right,              Left,        Left,    Left],
                    &table);
            } else {
                print_table(
                    &["Offset", "Size", "Compr-Size", "Compr-Method", "Compr-Block-Size", "SHA-1", "Filename"],
                    &[Right,    Right,  Right,        Left,           Right,              Left,    Left],
                    &table);
            }
        }
        ListStyle::OnlyNames { null_separated } => {
            let sep = [if null_separated { 0 } else { b'\n' }];
            let mut stdout = std::io::stdout();
            for record in records {
                stdout.write_all(record.as_ref().filename().as_bytes())?;
                stdout.write_all(&sep)?;
            }
        }
    }

    Ok(())
}
