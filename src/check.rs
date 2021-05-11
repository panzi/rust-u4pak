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

use std::{collections::HashSet, fs::File, io::{BufReader, Read, Seek, SeekFrom, stderr}, num::NonZeroUsize};

use crossbeam_channel::{Sender, unbounded};
use crossbeam_utils::thread;
use openssl::sha::Sha1 as OpenSSLSha1;

use crate::{Error, Filter, Pak, pak::{BUFFER_SIZE, COMPR_METHODS, COMPR_NONE, HexDisplay, Sha1}};
use crate::reopen::Reopen;
use crate::{Record, Result};

pub const NULL_SHA1: Sha1 = [0u8; 20];

#[derive(Debug)]
pub struct CheckOptions<'a> {
    pub abort_on_error: bool,
    pub ignore_null_checksums: bool,
    pub null_separated: bool,
    pub verbose: bool,
    pub paths: Option<&'a [&'a str]>,
    pub thread_count: NonZeroUsize,
}

impl Default for CheckOptions<'_> {
    fn default() -> Self {
        Self {
            abort_on_error: false,
            ignore_null_checksums: false,
            null_separated: false,
            verbose: false,
            paths: None,
            thread_count: NonZeroUsize::new(num_cpus::get()).unwrap_or(NonZeroUsize::new(1).unwrap()),
        }
    }
}

macro_rules! check_error {
    ($ok:expr, $result_sender:expr, $abort_on_error:expr, $error:expr) => {
        {
            if let Err(_) = $result_sender.send(Err($error)) {
                return;
            }

            if $abort_on_error {
                return;
            }

            $ok = false;
        }
    };
}

macro_rules! io {
    () => { Ok(()) };
    ($expr:expr $(,)?) => { $expr };
    ($expr:expr, $($tail:expr),* $(,)?) => {
        if let Err(_error) = ($expr) {
            Err(_error)
        } else {
            io!($($tail),*)
        }
    };
}

fn check_data<R>(reader: &mut R, filename: &str, offset: u64, size: u64, checksum: &Sha1, ignore_null_checksums: bool, buffer: &mut Vec<u8>) -> Result<()>
where R: Read, R: Seek {
    if ignore_null_checksums && checksum == &NULL_SHA1 {
        return Ok(());
    }
    reader.seek(SeekFrom::Start(offset))?;
    let mut hasher = OpenSSLSha1::new();
    let mut remaining = size;
    buffer.resize(BUFFER_SIZE, 0);
    loop {
        if remaining >= BUFFER_SIZE as u64 {
            reader.read_exact(buffer)?;
            hasher.update(&buffer);
            remaining -= BUFFER_SIZE as u64;
        } else {
            let buffer = &mut buffer[..remaining as usize];
            reader.read_exact(buffer)?;
            hasher.update(&buffer);
            break;
        }
    }
    let actual_digest = hasher.finish();
    if &actual_digest != checksum {
        return Err(Error::new(format!(
            "checksum missmatch:\n\
             \texpected: {}\n\
             \tactual:   {}",
             HexDisplay::new(checksum),
             HexDisplay::new(&actual_digest)
        )).with_path(filename));
    }
    Ok(())
}


pub fn check<'a>(pak: &'a Pak, in_file: &mut File, options: CheckOptions) -> Result<usize> {
    let CheckOptions { abort_on_error, ignore_null_checksums, null_separated, verbose, thread_count, paths } = options;
    let mut error_count = 0usize;
    let pak_path = in_file.path()?;
    let index_offset = pak.index_offset();
    let version = pak.version();
    let mut filter: Option<Filter> = paths.map(|paths| paths.into());
    let mut stderr = stderr();

    if let Err(error) = check_data(&mut BufReader::new(in_file), "<archive index>", index_offset, pak.index_size(), pak.index_sha1(), ignore_null_checksums, &mut vec![0u8; BUFFER_SIZE]) {
        error_count += 1;
        if abort_on_error {
            return Err(error);
        } else {
            let _ = error.write_to(&mut stderr, null_separated);
        }
    }

    let read_record = match version {
        1 => Record::read_v1,
        2 => Record::read_v2,
        _ if version <= 4 || version == 7 => Record::read_v3,
        _ => {
            return Err(Error::new(format!("unsupported version: {}", version)));
        }
    };

    let thread_result = thread::scope::<_, Result<usize>>(|scope| {
        let (work_sender, work_receiver) = unbounded::<&Record>();
        let (result_sender, result_receiver) = unbounded::<Result<&str>>();

        for _ in 0..thread_count.get() {
            let work_receiver = work_receiver.clone();
            let result_sender = result_sender.clone();
            let in_file = File::open(&pak_path)?;

            scope.spawn(move |_| {
                let mut reader = BufReader::new(in_file);
                let mut buffer = vec![0u8; BUFFER_SIZE];

                while let Ok(record) = work_receiver.recv() {
                    let mut ok = true;

                    if !COMPR_METHODS.contains(&record.compression_method()) {
                        check_error!(ok, result_sender, abort_on_error, Error::new(format!(
                            "unknown compression method: 0x{:02x}",
                            record.compression_method(),
                        )).with_path(record.filename()));
                    }

                    if record.compression_method() == COMPR_NONE && record.size() != record.uncompressed_size() {
                        check_error!(ok, result_sender, abort_on_error, Error::new(format!(
                            "file is not compressed but compressed size ({}) differes from uncompressed size ({})",
                            record.size(),
                            record.uncompressed_size(),
                        )).with_path(record.filename()));
                    }

                    let offset = record.offset() + Pak::header_size(version, record);
                    if offset + record.size() > index_offset {
                        check_error!(ok, result_sender, abort_on_error, Error::new(
                            "data bleeds into index".to_string()
                        ).with_path(record.filename()));
                    }

                    if let Err(error) = reader.seek(SeekFrom::Start(record.offset())) {
                        check_error!(ok, result_sender, abort_on_error,
                            Error::io_with_path(error, record.filename()));
                    } else {
                        match read_record(&mut reader, record.filename().to_string()) {
                            Ok(other_record) => {
                                if other_record.offset() != 0 {
                                    check_error!(ok, result_sender, abort_on_error,
                                        Error::new(format!("data record offset field is not 0 but {}",
                                                other_record.offset()))
                                            .with_path(other_record.filename()));
                                }

                                if !record.same_metadata(&other_record) {
                                    check_error!(ok, result_sender, abort_on_error,
                                        Error::new(format!("metadata missmatch:\n{}",
                                                record.metadata_diff(&other_record)))
                                            .with_path(other_record.filename()));
                                }
                            }
                            Err(error) => {
                                check_error!(ok, result_sender, abort_on_error, error);
                            }
                        };
                    }

                    if let Some(blocks) = record.compression_blocks() {
                        if !ignore_null_checksums || record.sha1() != &NULL_SHA1 {
                            let base_offset = if version >= 7 { record.offset() } else { 0 };
                            let mut hasher = OpenSSLSha1::new();

                            for block in blocks {
                                if block.start_offset > block.end_offset {
                                    check_error!(ok, result_sender, abort_on_error,
                                        Error::new(format!(
                                            "compression block start offset is bigger than end offset: {} > {}",
                                            block.start_offset, block.end_offset,
                                        )));
                                } else {
                                    let block_size = block.end_offset - block.start_offset;

                                    buffer.resize(block_size as usize, 0);
                                    if let Err(error) = io!{
                                        reader.seek(SeekFrom::Start(base_offset + block.start_offset)),
                                        reader.read_exact(&mut buffer)
                                    } {
                                        let _ = result_sender.send(Err(Error::io_with_path(error, record.filename())));
                                        return;
                                    }
                                    hasher.update(&buffer);
                                }
                            }

                            let actual_digest = hasher.finish();
                            if &actual_digest != record.sha1() {
                                check_error!(ok, result_sender, abort_on_error, Error::new(format!(
                                    "checksum missmatch:\n\
                                    \texpected: {}\n\
                                    \tactual:   {}",
                                    HexDisplay::new(record.sha1()),
                                    HexDisplay::new(&actual_digest)
                                )).with_path(record.filename()));
                            }
                        }
                    } else if let Err(error) = check_data(&mut reader, record.filename(), offset,
                            record.size(), record.sha1(), ignore_null_checksums, &mut buffer) {
                        check_error!(ok, result_sender, abort_on_error, error);
                    }

                    if ok {
                        let _ = result_sender.send(Ok(record.filename()));
                    }
                }
            });
        }

        drop(work_receiver);
        drop(result_sender);

        if let Some(filter) = &mut filter {
            let records = pak.records()
                .iter()
                .filter(|&record| filter.visit(record.filename()));

            error_count += enqueue(records, work_sender, abort_on_error, null_separated)?;
        } else {
            error_count += enqueue(pak.records().iter(), work_sender, abort_on_error, null_separated)?;
        }

        let linesep = if options.null_separated { '\0' } else { '\n' };

        while let Ok(result) = result_receiver.recv() {
            match result {
                Ok(filename) => {
                    if verbose {
                        print!("{}: OK{}", filename, linesep);
                    }
                }
                Err(error) => {
                    if abort_on_error {
                        return Err(error);
                    }
                    let _ = error.write_to(&mut stderr, null_separated);
                }
            }
        }

        if let Some(filter) = &filter {
            let mut iter = filter.non_visited_paths();
            if let Some(filename) = iter.next() {
                let mut message = format!("Paths not found in pak:\n* {}", filename);
                error_count += 1;
                for filename in iter {
                    message.push_str("\n* ");
                    message.push_str(&filename);
                    error_count += 1;
                }
                let error = Error::new(message);
                if abort_on_error {
                    return Err(error);
                }
                let _ = error.write_to(&mut stderr, null_separated);
            }
        }

        Ok(error_count)
    });

    match thread_result {
        Err(error) => {
            return Err(Error::new(format!("threading error: {:?}", error)));
        }
        Ok(result) => result
    }
}

fn enqueue<'a>(records: impl std::iter::Iterator<Item=&'a Record>, work_sender: Sender<&'a Record>, abort_on_error: bool, null_separated: bool) -> Result<usize> {
    let mut filenames: HashSet<&str> = HashSet::new();
    let mut error_count = 0usize;
    for record in records {
        if !filenames.insert(record.filename()) {
            let error = Error::new(
                "filename not unique in archive".to_string()
            ).with_path(record.filename());

            error_count += 1;
            if abort_on_error {
                return Err(error);
            } else {
                let _ = error.write_to(&mut stderr(), null_separated);
            }
        }

        match work_sender.send(record) {
            Ok(()) => {}
            Err(error) =>
                return Err(Error::new(error.to_string()).with_path(record.filename()))
        }
    }
    Ok(error_count)
}
