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

use std::str::FromStr;

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
    let mut first = true;
    for ((cell, len), align) in row.iter().zip(lens.iter()).zip(align.iter()) {
        if first {
            first = false;
        } else {
            print!("  "); // cell spacing
        }

        if align.is_right() {
            print!("{:>1$}", cell.as_ref(), *len);
        } else {
            print!("{:<1$}", cell.as_ref(), *len);
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
    let mut first = true;
    for len in lens.iter() {
        let mut len = *len;
        if first {
            first = false;
        } else {
            len += 2; // cell spacing
        }

        while len > 0 {
            print!("-");
            len -= 1;
        }
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

    if value.ends_with("B") {
        value = &value[..value.len() - 1];
    }

    if value.ends_with("K") {
        value = &value[..value.len() - 1].trim_end();
        Ok(value.parse::<usize>()? * 1024)
    } else if value.ends_with("M") {
        value = &value[..value.len() - 1].trim_end();
        Ok(value.parse::<usize>()? * 1024 * 1024)
    } else if value.ends_with("G") {
        value = &value[..value.len() - 1].trim_end();
        Ok(value.parse::<usize>()? * 1024 * 1024 * 1024)
    } else if value.ends_with("T") {
        value = &value[..value.len() - 1].trim_end();
        Ok(value.parse::<usize>()? * 1024 * 1024 * 1024 * 1024)
    } else if value.ends_with("P") {
        value = &value[..value.len() - 1].trim_end();
        Ok(value.parse::<usize>()? * 1024 * 1024 * 1024 * 1024 * 1024)
    } else if value.ends_with("E") {
        value = &value[..value.len() - 1].trim_end();
        Ok(value.parse::<usize>()? * 1024 * 1024 * 1024 * 1024 * 1024 * 1024)
    } else if value.ends_with("Z") {
        value = &value[..value.len() - 1].trim_end();
        Ok(value.parse::<usize>()? * 1024 * 1024 * 1024 * 1024 * 1024 * 1024 * 1024)
    } else if value.ends_with("Y") {
        value = &value[..value.len() - 1].trim_end();
        Ok(value.parse::<usize>()? * 1024 * 1024 * 1024 * 1024 * 1024 * 1024 * 1024 * 1024)
    } else {
        value.parse()
    }
}
