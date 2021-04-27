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

use crate::Result;
use crate::Pak;
use crate::Filter;

pub fn unpack<'a, R>(pak: &Pak, input: &mut File, outdir: impl AsRef<Path>, filter: &Option<Filter<'a>>) -> Result<()> {
    let outdir = outdir.as_ref();

    if !outdir.metadata()?.is_dir() {
        std::fs::create_dir_all(outdir)?;
    }

    if let Some(filter) = filter {
        let records = pak.records()
            .iter()
            .filter(|record| filter.contains(record.filename()));

        for record in records {
            pak.unpack(record, input, outdir)?;
        }
    } else {
        for record in pak.records() {
            pak.unpack(record, input, outdir)?;
        }
    }
    Ok(())
}
