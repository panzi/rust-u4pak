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

use crate::pak::{Pak, COMPR_NONE, COMPR_ZLIB, COMPR_BIAS_MEMORY, COMPR_BIAS_SPEED};
use crate::result::Result;
use crate::util::{format_size, print_headless_table, Align};

pub fn info(pak: &Pak, human_readable: bool) -> Result<()> {
    let fmt_size = if human_readable {
        |size: u64| format_size(size)
    } else {
        |size: u64| format!("{}", size)
    };

    let mut sum_size = 0;
    let mut sum_uncompressed_size = 0;
    let mut uncompr_count = 0;
    let mut zlib_count = 0;
    let mut bias_speed_count = 0;
    let mut bias_memory_count = 0;
    let mut other_count = 0;
    let mut encrypted_count = 0;

    for record in pak.records() {
        sum_size += record.size();
        sum_uncompressed_size += record.uncompressed_size();
        if record.encrypted() {
            encrypted_count += 1;
        }
        match record.compression_method() {
            self::COMPR_NONE        => uncompr_count     += 1,
            self::COMPR_ZLIB        => zlib_count        += 1,
            self::COMPR_BIAS_SPEED  => bias_speed_count  += 1,
            self::COMPR_BIAS_MEMORY => bias_memory_count += 1,
            _                       => other_count       += 1,
        }
    }

    print_headless_table(
        &[
            vec!["Version:",                   &format!("{}", pak.version())],
            vec!["Sum Compr. Size:",           &fmt_size(sum_size)],
            vec!["Sum Uncompr. Size:",         &fmt_size(sum_uncompressed_size)],
            vec!["File Count:",                &format!("{}", pak.records().len())],
            vec!["Uncompr. Count:",            &format!("{}", uncompr_count)],
            vec!["ZLIB Compr. Count:",         &format!("{}", zlib_count)],
            vec!["Bias Speed Compr. Count:",   &format!("{}", bias_speed_count)],
            vec!["Bias Memory Compr. Count:",  &format!("{}", bias_memory_count)],
            vec!["Unknown Compression Count:", &format!("{}", other_count)],
            vec!["Encrypted Count:",           &format!("{}", encrypted_count)],
        ],
        &[Align::Left, Align::Right]
    );

    Ok(())
}
