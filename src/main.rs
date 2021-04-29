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

use clap::{Arg, App, SubCommand};
use std::convert::TryInto;
use std::io::BufReader;
use std::fs::File;

pub mod pak;
pub use pak::{Pak, Options};

pub mod result;
pub use result::{Error, Result};

pub mod sort;
pub use sort::{DEFAULT_ORDER, SortKey, parse_order};

pub mod record;
pub use record::Record;

pub mod info;
pub use info::info;

pub mod list;
pub use list::{list, ListOptions, ListStyle};

pub mod util;

pub mod decode;

pub mod filter;
pub use filter::Filter;

pub mod unpack;
pub use unpack::unpack;

pub mod pack;
pub use pack::pack;

pub mod io;

fn get_filter<'a>(args: &'a clap::ArgMatches) -> Option<Filter<'a>> {
    if let Some(paths) = args.values_of("paths") {
        if paths.len() == 0 {
            None
        } else {
            Some(Filter::from_paths(paths))
        }
    } else {
        None
    }
}

fn arg_human_readable<'a, 'b>() -> Arg<'a, 'b> {
    Arg::with_name("human-readable")
        .long("human-readable")
        .short("h")
        .takes_value(false)
        .help("Print sizes like 1.0 K, 2.2 M, 4.1 G etc.")
}

fn arg_package<'a, 'b>() -> Arg<'a, 'b> {
    Arg::with_name("package")
        .index(1)
        .required(true)
        .value_name("PACKAGE")
        .help("A file ending in _dir.vpk (e.g. pak01_dir.vpk)")
}

fn arg_paths<'a, 'b>() -> Arg<'a, 'b> {
    Arg::with_name("paths")
        .index(2)
        .multiple(true)
        .value_name("PATH")
        .help("If given, only consider these files from the package.")
}

fn arg_check_integrity<'a, 'b>() -> Arg<'a, 'b> {
    Arg::with_name("check-integrity")
        .long("check-integrity")
        .short("c")
        .takes_value(false)
        .help("Check integrity of package")
}

fn arg_ignore_magic<'a, 'b>() -> Arg<'a, 'b> {
    Arg::with_name("ignore-magic")
        .long("ignore-magic")
        .takes_value(false)
        .help("Ignore file magic")
}

fn arg_encoding<'a, 'b>() -> Arg<'a, 'b> {
    Arg::with_name("encoding")
        .long("encoding")
        .short("e")
        .takes_value(true)
        .default_value("UTF-8")
        .value_name("ENCODING")
        .help("Use ENCODING to decode strings. Supported encodings: UTF-8, Latin1, ASCII")
}

fn arg_force_version<'a, 'b>() -> Arg<'a, 'b> {
    Arg::with_name("force-version")
        .long("force-version")
        .takes_value(true)
        .value_name("VERSION")
        .help("Assume package to be of given version.")
}

fn arg_ignore_null_checksums<'a, 'b>() -> Arg<'a, 'b> {
    Arg::with_name("ignore-null-checksums")
        .long("ignore-null-checksums")
        .takes_value(false)
        .help("Ignore checksums that are all zeros.")
}

fn arg_print0<'a, 'b>() -> Arg<'a, 'b> {
    Arg::with_name("print0")
        .long("print0")
        .short("0")
        .takes_value(false)
        .help(
            "Separate file names with NULL bytes. \
            This is useful for use with xargs --null, to be sure that \
            possible new lines in file names aren't interpreted as \
            file name separators.")
}

fn run() -> Result<()> {
    let app = App::new("VPK - Valve Packages")
        .version("1.0.0")
        .author("Mathias Panzenböck <grosser.meister.morti@gmx.net>");

    let app = app
        .subcommand(SubCommand::with_name("info")
            .alias("i")
            .about("Show summarized information of a package.")
            .arg(arg_human_readable())
            .arg(arg_ignore_magic())
            .arg(arg_encoding())
            .arg(arg_force_version())
            .arg(arg_package()))
        .subcommand(SubCommand::with_name("list")
            .alias("l")
            .about("List content of a package.")
            .arg(Arg::with_name("only-names")
                .long("only-names")
                .short("n")
                .takes_value(false)
                .help(
                    "Only print file names. \
                     This is useful for use with xargs and the like."))
            .arg(Arg::with_name("sort")
                .long("sort")
                .short("s")
                .takes_value(true)
                .value_name("ORDER")
                .help(
                    "Sort order of list as comma separated keys:\n\
                    \n\
                    * path               - path of the file inside the package\n\
                    * size               - size of the data embedded in the package\n\
                    * uncompressed-size  - sum of the other two sizes\n\
                    * offset             - offset inside of the package\n\
                    * compression-method - the compression method\n\
                    \n\
                    If you prepend the order with - you invert the sort order for that key. E.g.:\n\
                    \n\
                    u4pak list --sort=-size,name")
            )
            .arg(arg_check_integrity())
            .arg(arg_print0().requires("only-names"))
            .arg(arg_ignore_magic())
            .arg(arg_encoding())
            .arg(arg_force_version())
            .arg(arg_ignore_null_checksums())
            .arg(arg_human_readable())
            .arg(arg_package())
            .arg(arg_paths()))
        .subcommand(SubCommand::with_name("check")
            .alias("c")
            .about("Check concistency of a package.")
            .arg(arg_print0())
            .arg(arg_ignore_magic())
            .arg(arg_encoding())
            .arg(arg_force_version())
            .arg(arg_ignore_null_checksums())
            .arg(arg_package())
            .arg(arg_paths()))
        .subcommand(SubCommand::with_name("unpack")
            .alias("u")
            .about("Unpack content of a package.")
            .arg(arg_print0())
            .arg(arg_check_integrity())
            .arg(arg_ignore_magic())
            .arg(arg_encoding())
            .arg(arg_force_version())
            .arg(arg_ignore_null_checksums())
            .arg(Arg::with_name("outdir")
                .long("outdir")
                .short("o")
                .takes_value(true)
                .value_name("DIR")
                .default_value(".")
                .help("Write unpacked files to DIR."))
            .arg(arg_package())
            .arg(arg_paths()))
        .subcommand(SubCommand::with_name("pack")
            .alias("p")
            .about("Create a new package.")
            .arg(Arg::with_name("version")
                .long("version")
                .short("v")
                .takes_value(true)
                .default_value("3")
                .help("Create package of given VERSION. Supported versions are: 1, 2, and 3"))
            .arg(arg_package())
            .arg(Arg::with_name("paths")
                .index(2)
                .multiple(true)
                .value_name("PATH")
                .help("Pack these files.")));

    #[cfg(target_os = "linux")]
    let app = app.subcommand(SubCommand::with_name("mount")
        .alias("m")
        .about("Mount package as read-only filesystem.")
        .arg(arg_check_integrity())
        .arg(arg_ignore_magic())
        .arg(arg_encoding())
        .arg(arg_force_version())
        .arg(arg_ignore_null_checksums())
        .arg(Arg::with_name("foregound")
            .long("foreground")
            .short("f")
            .takes_value(false)
            .help("Keep process in foreground."))
        .arg(arg_package())
        .arg(Arg::with_name("mountpt")
            .index(2)
            .required(true)
            .value_name("MOUNTPT")));

    let matches = app.get_matches();

    match matches.subcommand() {
        ("info", Some(args)) => {
            let human_readable        = args.is_present("human-readable");
            let ignore_magic          = args.is_present("ignore-magic");
            let encoding = args.value_of("encoding").unwrap().try_into()?;
            let path = args.value_of("package").unwrap();

            let force_version = if let Some(version) = args.value_of("force-version") {
                Some(version.parse()?)
            } else {
                None
            };

            let pak = Pak::from_path(&path, Options {
                ignore_magic,
                encoding,
                force_version,
            })?;

            info(&pak, human_readable)?;
        }
        ("list", Some(args)) => {
            let order = if let Some(order) = args.value_of("sort") {
                Some(parse_order(order)?)
            } else {
                None
            };
            let order = if let Some(order) = &order {
                Some(&order[..])
            } else {
                None
            };

            let human_readable        = args.is_present("human-readable");
            let null_separated        = args.is_present("print0");
            let only_names            = args.is_present("only-names");
            let check_integrity       = args.is_present("check-integrity");
            let ignore_magic          = args.is_present("ignore-magic");
            let ignore_null_checksums = args.is_present("ignore-null-checksums");
            let encoding = args.value_of("encoding").unwrap().try_into()?;
            let path = args.value_of("package").unwrap();
            let filter = get_filter(args);

            let force_version = if let Some(version) = args.value_of("force-version") {
                Some(version.parse()?)
            } else {
                None
            };

            let file = match File::open(path) {
                Ok(file) => file,
                Err(error) => return Err(Error::io_with_path(error, path))
            };
            let mut reader = BufReader::new(file);

            let pak = Pak::from_reader(&mut reader, Options {
                ignore_magic,
                encoding,
                force_version,
            })?;

            if check_integrity {
                if let Some(filter) = &filter {
                    let records = pak.records()
                        .iter()
                        .filter(|record| filter.contains(record.filename()));
                    pak.check_integrity_of(records, &mut reader, true, ignore_null_checksums, null_separated)?;
                } else {
                    pak.check_integrity(&mut reader, true, ignore_null_checksums, null_separated)?;
                }
            }

            list(pak, ListOptions {
                order,
                style: if only_names {
                    ListStyle::OnlyNames { null_separated }
                } else {
                    ListStyle::Table { human_readable }
                },
                filter,
            })?;
        }
        ("check", Some(args)) => {
            let null_separated        = args.is_present("print0");
            let ignore_magic          = args.is_present("ignore-magic");
            let ignore_null_checksums = args.is_present("ignore-null-checksums");
            let encoding = args.value_of("encoding").unwrap().try_into()?;
            let path = args.value_of("package").unwrap();
            let filter = get_filter(args);

            let force_version = if let Some(version) = args.value_of("force-version") {
                Some(version.parse()?)
            } else {
                None
            };

            let file = match File::open(path) {
                Ok(file) => file,
                Err(error) => return Err(Error::io_with_path(error, path))
            };
            let mut reader = BufReader::new(file);

            let pak = Pak::from_reader(&mut reader, Options {
                ignore_magic,
                encoding,
                force_version,
            })?;

            let error_count = if let Some(filter) = &filter {
                let records = pak.records()
                    .iter()
                    .filter(|record| filter.contains(record.filename()));
                pak.check_integrity_of(records, &mut reader, false, ignore_null_checksums, null_separated)?
            } else {
                pak.check_integrity(&mut reader, false, ignore_null_checksums, null_separated)?
            };

            let sep = if null_separated { '\0' } else { '\n' };
            if error_count == 0 {
                print!("All ok{}", sep);
            } else {
                print!("Found {} error(s){}", error_count, sep);
                std::process::exit(1);
            }
        }
        ("unpack", Some(args)) => {
            let outdir = args.value_of("outdir").unwrap();
            let null_separated        = args.is_present("print0");
            let ignore_magic          = args.is_present("ignore-magic");
            let check_integrity       = args.is_present("check-integrity");
            let ignore_null_checksums = args.is_present("ignore-null-checksums");
            let encoding = args.value_of("encoding").unwrap().try_into()?;
            let path = args.value_of("package").unwrap();
            let filter = get_filter(args);

            let force_version = if let Some(version) = args.value_of("force-version") {
                Some(version.parse()?)
            } else {
                None
            };

            let mut file = match File::open(path) {
                Ok(file) => file,
                Err(error) => return Err(Error::io_with_path(error, path))
            };
            let mut reader = BufReader::new(&mut file);

            let pak = Pak::from_reader(&mut reader, Options {
                ignore_magic,
                encoding,
                force_version,
            })?;

            if check_integrity {
                if let Some(filter) = &filter {
                    let records = pak.records()
                        .iter()
                        .filter(|record| filter.contains(record.filename()));
                    pak.check_integrity_of(records, &mut reader, true, ignore_null_checksums, null_separated)?;
                } else {
                    pak.check_integrity(&mut reader, true, ignore_null_checksums, null_separated)?;
                }
            }

            drop(reader);

            unpack(&pak, &mut file, outdir, &filter)?;
        }
        ("pack", Some(_args)) => {
            panic!("pack is not implemented yet");
        }
        #[cfg(target_os = "linux")]
        ("mount", Some(_args)) => {
            panic!("mount is not implemented yet");
        }
        (cmd, _) => {
            return Err(Error::new(format!(
                "unknown subcommand: {}\n\
                 For more information try --help",
                 cmd
            )));
        }
    }

    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{}", error);
        std::process::exit(1);
    }
}
