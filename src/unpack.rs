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

use std::path::Path;
use std::fs::File;

use crate::{Result, pak::COMPR_NONE};
use crate::Record;
use crate::Pak;
use crate::Filter;

#[inline]
fn unpack_iter<'a>(pak: &Pak, in_file: &mut File, outdir: &Path, dirname_from_compression: bool, records_iter: impl Iterator<Item=&'a Record>) -> Result<()> {
    if dirname_from_compression {
        let mut zlib_outdir = outdir.to_path_buf();
        let mut none_outdir = outdir.to_path_buf();

        zlib_outdir.push("zlib");
        none_outdir.push("none");

        for record in records_iter {
            let method = record.compression_method();
            let outdir = if method == COMPR_NONE { &none_outdir } else { &zlib_outdir };

            match pak.unpack(record, in_file, outdir) {
                Ok(()) => {},
                Err(error) => {
                    return Err(error.with_path_if_none(record.filename()));
                }
            }
        }
    } else {
        for record in records_iter {
            match pak.unpack(record, in_file, outdir) {
                Ok(()) => {},
                Err(error) => {
                    return Err(error.with_path_if_none(record.filename()));
                }
            }
        }
    }

    Ok(())
}

pub fn unpack<'a>(pak: &Pak, in_file: &mut File, outdir: impl AsRef<Path>, dirname_from_compression: bool, filter: &Option<Filter<'a>>) -> Result<()> {
    let outdir = outdir.as_ref();

    if let Some(filter) = filter {
        let records = pak.records()
            .iter()
            .filter(|record| filter.contains(record.filename()));

        unpack_iter(pak, in_file, outdir, dirname_from_compression, records)?;
    } else {
        unpack_iter(pak, in_file, outdir, dirname_from_compression, pak.records().iter())?;
    }
    Ok(())
}
