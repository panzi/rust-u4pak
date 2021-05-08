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

use std::{fs::OpenOptions, io::{BufReader, BufWriter, Read, Seek, SeekFrom, Write}, path::{Path, PathBuf}};
use std::fs::File;

use crossbeam_channel::{Receiver, Sender, unbounded};
use crossbeam_utils::thread;
use flate2::bufread::ZlibDecoder;

use crate::{Error, Result, io::transfer, pak::{self, COMPR_NONE, compression_method_name}, util::parse_pak_path};
use crate::Record;
use crate::Pak;
use crate::Filter;
use crate::reopen::Reopen;

#[derive(Debug)]
pub struct UnpackOptions<'a> {
    pub dirname_from_compression: bool,
    pub verbose: bool,
    pub null_separated: bool,
    pub filter: Option<Filter<'a>>,
}

impl Default for UnpackOptions<'_> {
    fn default() -> Self {
        Self {
            dirname_from_compression: false,
            verbose: false,
            null_separated: false,
            filter: None,
        }
    }
}

#[inline]
fn unpack_iter<'a>(pak: &Pak, in_file: &mut File, outdir: &Path, options: &'a UnpackOptions<'a>, records_iter: impl Iterator<Item=&'a Record>) -> Result<()> {
    let thread_count = num_cpus::get(); // TODO: also get from arguments
    let version = pak.version();

    let dirnames = if options.dirname_from_compression {
        let mut zlib_outdir = outdir.to_path_buf();
        let mut none_outdir = outdir.to_path_buf();

        zlib_outdir.push("zlib");
        none_outdir.push("none");

        Some((zlib_outdir, none_outdir))
    } else {
        None
    };

    let pak_path = in_file.path()?;

    let thread_result = thread::scope::<_, Result<()>>(|scope| {
        let (work_sender, work_receiver) = unbounded();
        let (result_sender, result_receiver) = unbounded();

        for _ in 0..thread_count {
            let work_receiver = work_receiver.clone();
            let result_sender = result_sender.clone();
            let mut in_file = File::open(&pak_path)?;

            scope.spawn(move |_| {
                let in_file = &mut in_file;
                if let Err(error) = worker_proc(in_file, version, work_receiver, result_sender) {
                    if !error.error_type().is_channel_disconnected() {
                        eprintln!("error in worker thread: {}", error);
                    }
                }
            });
        }

        drop(work_receiver);
        drop(result_sender);

        if let Some((zlib_outdir, none_outdir)) = &dirnames {
            for record in records_iter {
                let method = record.compression_method();
                let outdir = if method == COMPR_NONE { &none_outdir } else { &zlib_outdir };

                match work_sender.send(Work { record, outdir }) {
                    Ok(()) => {}
                    Err(error) =>
                        return Err(Error::new(error.to_string()).with_path(record.filename()))
                }
            }
        } else {
            for record in records_iter {
                match work_sender.send(Work { record, outdir }) {
                    Ok(()) => {}
                    Err(error) =>
                        return Err(Error::new(error.to_string()).with_path(record.filename()))
                }
            }
        }

        drop(work_sender);

        #[cfg(target_family="unix")]
        let mut stdout = std::io::stdout();

        let linesep = if options.null_separated { '\0' } else { '\n' };

        while let Ok(result) = result_receiver.recv() {
            let path = result?;
            if options.verbose {
                #[cfg(target_family="unix")]
                {
                    use std::os::unix::ffi::OsStrExt;
                    let _ = stdout.write_all(path.as_os_str().as_bytes());
                    let _ = stdout.write_all(&[linesep as u8]);
                }

                #[cfg(not(target_family="unix"))]
                {
                    print!("{}{}", path.to_string_lossy(), linesep);
                }
            }
        }

        drop(result_receiver);

        Ok(())
    });

    match thread_result {
        Err(error) => {
            return Err(Error::new(format!("threading error: {:?}", error)));
        }
        Ok(result) => result
    }
}

pub fn unpack<'a>(pak: &Pak, in_file: &mut File, outdir: impl AsRef<Path>, options: UnpackOptions<'a>) -> Result<()> {
    let outdir = outdir.as_ref();

    if let Some(filter) = &options.filter {
        let records = pak.records()
            .iter()
            .filter(|record| filter.contains(record.filename()));

        unpack_iter(pak, in_file, outdir, &options, records)?;
    } else {
        unpack_iter(pak, in_file, outdir, &options, pak.records().iter())?;
    }
    Ok(())
}

pub fn unpack_record(record: &Record, version: u32, in_file: &mut File, outdir: impl AsRef<Path>) -> Result<PathBuf> {
    if record.encrypted() {
        return Err(Error::new("encryption is not supported".to_string())
            .with_path(record.filename()));
    }

    let mut path = outdir.as_ref().to_path_buf();
    for component in parse_pak_path(record.filename()) {
        path.push(component);
    }

    let mut out_file = match OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&path) {
        Ok(file) => file,
        Err(error) => {
            if error.kind() == std::io::ErrorKind::NotFound {
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent)?;
                    OpenOptions::new().write(true).create(true).open(&path)?
                } else {
                    return Err(Error::io_with_path(error, path));
                }
            } else {
                return Err(Error::io_with_path(error, path));
            }
        }
    };

    match record.compression_method() {
        pak::COMPR_NONE => {
            in_file.seek(SeekFrom::Start(record.offset() + pak::Pak::header_size(version, record)))?;
            transfer(in_file, &mut out_file, record.size() as usize)?;
            out_file.flush()?;
        }
        pak::COMPR_ZLIB => {
            if let Some(blocks) = record.compression_blocks() {
                let base_offset = if version >= 7 { record.offset() } else { 0 };

                let mut in_file = BufReader::new(in_file);
                let mut out_file = BufWriter::new(out_file);

                let mut in_buffer = Vec::new();
                let mut out_buffer = Vec::with_capacity(record.compression_block_size() as usize);

                for block in blocks {
                    let block_size = block.end_offset - block.start_offset;
                    in_buffer.resize(block_size as usize, 0);
                    in_file.seek(SeekFrom::Start(base_offset + block.start_offset))?;
                    in_file.read_exact(&mut in_buffer)?;

                    let mut zlib = ZlibDecoder::new(&in_buffer[..]);
                    out_buffer.clear();
                    zlib.read_to_end(&mut out_buffer)?;
                    out_file.write_all(&out_buffer)?;
                }
                out_file.flush()?;
            } else {
                // version 2 has compression support, but not compression blocks
                in_file.seek(SeekFrom::Start(record.offset() + pak::Pak::header_size(version, record)))?;

                let mut in_buffer = vec![0u8; record.size() as usize];
                let mut out_buffer = Vec::new();
                in_file.read_exact(&mut in_buffer)?;

                let mut zlib = ZlibDecoder::new(&in_buffer[..]);
                zlib.read_to_end(&mut out_buffer)?;
                out_file.write_all(&out_buffer)?;
                out_file.flush()?;
            }
        }
        _ => {
            return Err(Error::new(format!(
                    "unsupported compression method: {}",
                    compression_method_name(record.compression_method())))
                .with_path(record.filename()));
        }
    }

    Ok(path)
}

#[derive(Debug)]
struct Work<'a> {
    record: &'a Record,
    outdir: &'a Path,
}

fn worker_proc(in_file: &mut File, version: u32, work_channel: Receiver<Work>, result_channel: Sender<Result<PathBuf>>) -> Result<()> {
    while let Ok(Work { record, outdir }) = work_channel.recv() {
        let result = unpack_record(record, version, in_file, outdir)
            .map_err(|error| error
                .with_path_if_none(record.filename()));

        result_channel.send(result)?;
    }

    Ok(())
}
