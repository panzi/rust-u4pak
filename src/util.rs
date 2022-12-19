// This file is part of rust-u4pak.
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use std::io::Read;
use std::str::FromStr;
use core::num::NonZeroU32;

use crate::{Result, Error};

pub fn format_size(size: u64) -> String {
    if size >= 1024 * 1024 * 1024 * 1024 * 1024 * 1024 {
        format!("{:.1} E", (size / (1024 * 1024 * 1024 * 1024 * 1024)) as f64 / 1024.0)
    } else if size >= 1024 * 1024 * 1024 * 1024 * 1024 {
        format!("{:.1} P", (size / (1024 * 1024 * 1024 * 1024)) as f64 / 1024.0)
    } else if size >= 1024 * 1024 * 1024 * 1024 {
        format!("{:.1} T", (size / (1024 * 1024 * 1024)) as f64 / 1024.0)
    } else if size >= 1024 * 1024 * 1024 {
        format!("{:.1} G", (size / (1024 * 1024)) as f64 / 1024.0)
    } else if size >= 1024 * 1024 {
        format!("{:.1} M", (size / 1024) as f64 / 1024.0)
    } else if size >= 1024 {
        format!("{:.1} K", size as f64 / 1024.0)
    } else {
        format!("{} B", size)
    }
}

pub enum Align {
    Left,
    Right
}

impl Align {
    #[allow(unused)]
    pub fn is_left(&self) -> bool {
        match self {
            Align::Left  => true,
            Align::Right => false,
        }
    }

    pub fn is_right(&self) -> bool {
        match self {
            Align::Left  => false,
            Align::Right => true,
        }
    }
}

pub fn print_row(row: &[impl AsRef<str>], lens: &[usize], align: &[Align]) {
    let cell_count = row.len();
    if cell_count > 0 {
        let mut first = true;
        let last_index = cell_count - 1;
        for (index, ((cell, len), align)) in row.iter().zip(lens.iter()).zip(align.iter()).enumerate() {
            if first {
                first = false;
            } else {
                print!("  "); // cell spacing
            }

            if align.is_right() {
                print!("{:>1$}", cell.as_ref(), *len);
            } else if index == last_index {
                print!("{}", cell.as_ref());
            } else {
                print!("{:<1$}", cell.as_ref(), *len);
            }
        }
    }

    println!();
}

pub fn print_table(header: &[impl AsRef<str>], align: &[Align], body: &[Vec<impl AsRef<str>>]) {
    // TODO: maybe count graphemes? needs extra lib. haven't seen non-ASCII filenames anyway
    let mut lens: Vec<usize> = vec![0; align.len()];

    for (cell, max_len) in header.iter().zip(lens.iter_mut()) {
        let len = cell.as_ref().chars().count();
        if len > *max_len {
            *max_len = len;
        }
    }

    for row in body {
        for (cell, max_len) in row.iter().zip(lens.iter_mut()) {
            let len = cell.as_ref().chars().count();
            if len > *max_len {
                *max_len = len;
            }
        }
    }

    print_row(header, &lens, align);
    let line_len = if lens.is_empty() { 0 } else {
        lens.iter().sum::<usize>() + 2 * (lens.len() - 1)
    };
    for _ in 0..line_len {
        print!("-");
    }
    println!();

    for row in body {
        print_row(row, &lens, align);
    }
}

pub fn print_headless_table(body: &[Vec<impl AsRef<str>>], align: &[Align]) {
    let mut lens = Vec::new();

    for row in body {
        while lens.len() < row.len() {
            lens.push(0);
        }
        for (cell, max_len) in row.iter().zip(lens.iter_mut()) {
            let len = cell.as_ref().chars().count();
            if len > *max_len {
                *max_len = len;
            }
        }
    }

    for row in body {
        print_row(row, &lens, align);
    }
}

pub fn parse_size(value: &str) -> std::result::Result<usize, <usize as FromStr>::Err> {
    let mut value = value.trim();

    if value.ends_with('B') {
        value = &value[..value.len() - 1];
    }

    if value.ends_with('K') {
        value = &value[..value.len() - 1].trim_end();
        Ok(value.parse::<usize>()? * 1024)
    } else if value.ends_with('M') {
        value = &value[..value.len() - 1].trim_end();
        Ok(value.parse::<usize>()? * 1024 * 1024)
    } else if value.ends_with('G') {
        value = &value[..value.len() - 1].trim_end();
        Ok(value.parse::<usize>()? * 1024 * 1024 * 1024)
    } else if value.ends_with('T') {
        value = &value[..value.len() - 1].trim_end();
        Ok(value.parse::<usize>()? * 1024 * 1024 * 1024 * 1024)
    } else if value.ends_with('P') {
        value = &value[..value.len() - 1].trim_end();
        Ok(value.parse::<usize>()? * 1024 * 1024 * 1024 * 1024 * 1024)
    } else if value.ends_with('E') {
        value = &value[..value.len() - 1].trim_end();
        Ok(value.parse::<usize>()? * 1024 * 1024 * 1024 * 1024 * 1024 * 1024)
    } else if value.ends_with('Z') {
        value = &value[..value.len() - 1].trim_end();
        Ok(value.parse::<usize>()? * 1024 * 1024 * 1024 * 1024 * 1024 * 1024 * 1024)
    } else if value.ends_with('Y') {
        value = &value[..value.len() - 1].trim_end();
        Ok(value.parse::<usize>()? * 1024 * 1024 * 1024 * 1024 * 1024 * 1024 * 1024 * 1024)
    } else {
        value.parse()
    }
}

pub fn parse_pak_path(path: &str) -> impl std::iter::Iterator<Item=&str> {
    path.trim_matches('/')
        .split('/')
        .filter(|comp| !comp.is_empty())
}

pub fn make_pak_path(mut components: impl std::iter::Iterator<Item=impl AsRef<str>>) -> String {
    let mut path = String::new();
    if let Some(first) = components.next() {
        path.push_str(first.as_ref());
        for component in components {
            path.push('/');
            path.push_str(component.as_ref());
        }
    } else {
        path.push('/');
    }
    path
}

// Align to power of 2 alignment
pub fn align(val: u64, alignment: u64) -> u64 {
    assert_eq!(alignment & (alignment - 1), 0, "Alignment must be a power of 2");
    // Add alignment        Zero out alignment bits
    (val + alignment - 1) & !(alignment - 1)
}

pub const COMPR_LEVEL_FAST:    NonZeroU32 = unsafe { NonZeroU32::new_unchecked(1) };
pub const COMPR_LEVEL_DEFAULT: NonZeroU32 = unsafe { NonZeroU32::new_unchecked(6) };
pub const COMPR_LEVEL_BEST:    NonZeroU32 = unsafe { NonZeroU32::new_unchecked(9) };

pub fn parse_compression_level(value: &str) -> Result<NonZeroU32> {
    if value.eq_ignore_ascii_case("best") {
        Ok(COMPR_LEVEL_BEST)
    } else if value.eq_ignore_ascii_case("fast") {
        Ok(COMPR_LEVEL_FAST)
    } else if value.eq_ignore_ascii_case("default") {
        Ok(COMPR_LEVEL_DEFAULT)
    } else {
        match value.parse() {
            Ok(level) if level > 0 && level < 10 => {
                Ok(NonZeroU32::new(level).unwrap())
            }
            _ => {
                return Err(Error::new(format!(
                    "illegal compression level: {:?}",
                    value)));
            }
        }
    }
}

pub fn sha1_digest<R: Read>(mut reader: R) -> Result<[u8; 20]> {
    let mut hasher = sha1_smol::Sha1::new();
    let mut buffer = [0; 1024];

    loop {
        let count = reader.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }

    Ok(hasher.digest().bytes())
}
