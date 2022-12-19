// This file is part of rust-u4pak.
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use std::io::Write;

use chrono::NaiveDateTime;

use u4pak::{Filter, util::print_headless_table};
use u4pak::util::{format_size, print_table, Align::*};
use u4pak::result::Result;
use u4pak::record::Record;
use u4pak::pak::{Pak, compression_method_name, HexDisplay};
use u4pak::check::NULL_SHA1;
use crate::sort::{sort, Order};

#[derive(Debug, PartialEq, Eq)]
pub enum ListStyle {
    Table { human_readable: bool, no_header: bool },
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
        ListStyle::Table { human_readable: false, no_header: false }
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
            let mut records = pak.index().records()
                .iter()
                .filter(|record| filter.visit(record.filename()))
                .collect();

            sort(&mut records, order);
            list_records(version, &records, options)?;
            filter.assert_all_visited()?;
        }
        (Some(order), None) => {
            let mut records = pak.index().records().iter().collect();

            sort(&mut records, order);
            list_records(version, &records, options)?;
        }
        (None, Some(paths)) => {
            let mut filter = Filter::from_paths(paths.iter().cloned());
            let records = pak.index().records()
                .iter()
                .filter(|record| filter.visit(record.filename()))
                .collect::<Vec<_>>();

            list_records(version, &records, options)?;
            filter.assert_all_visited()?;
        }
        (None, None) => {
            list_records(version, pak.index().records(), options)?;
        }
    }

    Ok(())
}

fn list_records(version: u32, records: &[impl AsRef<Record>], options: ListOptions) -> Result<()> {
    match options.style {
        ListStyle::Table { human_readable, no_header } => {
            let mut body: Vec<Vec<String>> = Vec::new();

            let fmt_size = if human_readable {
                format_size
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
                } else if version >= 3 {
                    row.push(if record.encrypted() { "Encrypted" } else { "-" }.to_string());
                }
                row.push(HexDisplay::new(record.sha1().as_ref().unwrap_or(&NULL_SHA1)).to_string());
                row.push(record.filename().to_owned());
                body.push(row);
            }

            if version == 1 {
                let align = [Right, Right, Right, Left, Right, Left, Left, Left];
                if no_header {
                    print_headless_table(&body, &align);
                } else {
                    print_table(
                        &["Offset", "Size", "Compr.", "Method", "Block-Size", "Timestamp", "SHA-1", "Filename"],
                        &align,
                        &body,
                    );
                }
            } else if version >= 3 {
                let align = [Right, Right, Right, Left, Right, Left, Left, Left];
                if no_header {
                    print_headless_table(&body, &align);
                } else {
                    print_table(
                        &["Offset", "Size", "Compr.", "Method", "Block-Size", "Encrypted", "SHA-1", "Filename"],
                        &align,
                        &body,
                    );
                }
            } else {
                let align = [Right, Right, Right, Left, Right, Left, Left];
                if no_header {
                    print_headless_table(&body, &align);
                } else {
                    print_table(
                        &["Offset", "Size", "Compr.", "Method", "Block-Size", "SHA-1", "Filename"],
                        &align,
                        &body,
                    );
                }
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
