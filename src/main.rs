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

use clap::{App, AppSettings, Arg, ArgMatches, SubCommand};
use pak::COMPR_NONE;
use std::{convert::TryInto, io::stderr, num::{NonZeroU32, NonZeroUsize}};
use std::io::BufReader;
use std::fs::File;

#[cfg(target_family="windows")]
use std::convert::TryFrom;

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
pub mod encode;

pub mod filter;
pub use filter::Filter;

pub mod unpack;
pub use unpack::unpack;

pub mod pack;
pub use pack::{pack, PackOptions};

pub mod check;
pub use check::check;

pub mod walkdir;

use crate::{check::CheckOptions, pack::PackPath, pak::{COMPR_ZLIB, DEFAULT_BLOCK_SIZE}, unpack::UnpackOptions, util::parse_size};

pub mod io;

pub mod reopen;

fn get_paths<'a>(args: &'a clap::ArgMatches) -> Result<Option<Vec<&'a str>>> {
    if let Some(arg_paths) = args.values_of("paths") {
        let count = arg_paths.len();
        if count == 0 {
            Ok(None)
        } else {
            let mut paths: Vec<&str> = Vec::with_capacity(count);
            for path in arg_paths {
                if path.is_empty() {
                    return Err(Error::new(
                        "Path may not be empty. Use \"/\" to reference the root directory of a pak archive."
                        .to_string()));
                }
                paths.push(path);
            }
            Ok(Some(paths))
        }
    } else {
        Ok(None)
    }
}

fn get_threads(args: &clap::ArgMatches) -> Result<NonZeroUsize> {
    let threads = if let Some(threads) = args.value_of("threads") {
        if threads.eq_ignore_ascii_case("auto") {
            NonZeroUsize::new(num_cpus::get())
        } else {
            let threads = threads.parse()?;
            if threads == 0 {
                return Err(Error::new("thread count may not be 0".to_string()));
            }
            NonZeroUsize::new(threads)
        }
    } else {
        NonZeroUsize::new(num_cpus::get())
    };

    Ok(threads.unwrap_or_else(|| NonZeroUsize::new(1).unwrap()))
}

pub fn parse_compression_method(value: &str) -> Result<u32> {
    if value.eq_ignore_ascii_case("none") {
        Ok(COMPR_NONE)
    } else if value.eq_ignore_ascii_case("zlib") {
        Ok(COMPR_ZLIB)
    } else {
        Err(Error::new(format!("compression method not supported: {:?}", value)))
    }
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

fn arg_verbose<'a, 'b>() -> Arg<'a, 'b> {
    Arg::with_name("verbose")
        .long("verbose")
        .short("v")
        .takes_value(false)
        .help("Verbose output.")
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

fn arg_threads<'a, 'b>() -> Arg<'a, 'b> {
    Arg::with_name("threads")
        .long("threads")
        .short("t")
        .takes_value(true)
        .default_value("auto")
        .value_name("COUNT")
        .help("Number of threads to use.")
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

#[cfg(target_family="windows")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Pause {
    Always,
    Never,
    Auto,
}

#[cfg(target_family="windows")]
impl Default for Pause {
    fn default() -> Self {
        Self::Auto
    }
}

#[cfg(target_family="windows")]
impl TryFrom<&str> for Pause {
    type Error = crate::Error;

    fn try_from(value: &str) -> std::result::Result<Self, Self::Error> {
        let trim_value = value.trim();

        if trim_value.eq_ignore_ascii_case("auto") {
            Ok(Pause::Auto)
        } else if trim_value.eq_ignore_ascii_case("never") {
            Ok(Pause::Never)
        } else if trim_value.eq_ignore_ascii_case("always") {
            Ok(Pause::Always)
        } else {
            Err(Error::new(format!("illegal value for --pause: {:?}", value)))
        }
    }
}

fn main() {
    let default_block_size_str = format!("{}", DEFAULT_BLOCK_SIZE);

    let app = App::new("VPK - Valve Packages")
        .version("1.0.0")
        .global_setting(AppSettings::VersionlessSubcommands)
        .author("Mathias Panzenböck <grosser.meister.morti@gmx.net>");

    #[cfg(target_family="windows")]
    let app = app
        .arg(Arg::with_name("pause")
            .long("pause")
            .default_value("auto")
            .takes_value(true)
            .help("Wait for user to press ENTER at exit. Possible values: always, never, auto."));

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
            .arg(arg_print0().requires("only-names"))
            .arg(arg_ignore_magic())
            .arg(arg_encoding())
            .arg(arg_force_version())
            .arg(arg_human_readable())
            .arg(arg_threads())
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
            .arg(arg_threads())
            .arg(arg_verbose())
            .arg(arg_package())
            .arg(arg_paths()))
        .subcommand(SubCommand::with_name("unpack")
            .alias("u")
            .about("Unpack content of a package.")
            .arg(arg_print0())
            .arg(arg_ignore_magic())
            .arg(arg_encoding())
            .arg(arg_force_version())
            .arg(arg_threads())
            .arg(arg_verbose())
            .arg(Arg::with_name("dirname-from-compression")
                .long("dirname-from-compression")
                .short("d")
                .takes_value(false)
                .help("Put files that where compressed into separate folders."))
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
            .arg(Arg::with_name("mount-point")
                .long("mount-point")
                .short("m")
                .takes_value(true))
            .arg(Arg::with_name("compression-method")
                .long("compression-method")
                .short("c")
                .takes_value(true)
                .default_value("none"))
            .arg(Arg::with_name("compression-block-size")
                .long("compression-block-size")
                .short("b")
                .takes_value(true)
                .default_value(&default_block_size_str))
            .arg(Arg::with_name("compression-level")
                .long("compression-level")
                .short("l")
                .takes_value(true)
                .default_value("default"))
            .arg(arg_encoding())
            .arg(arg_print0())
            .arg(arg_threads())
            .arg(arg_verbose())
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

    let matches = match app.get_matches_safe() {
        Ok(matches) => matches,
        Err(error) => {
            if error.use_stderr() {
                eprintln!("{}", error);
                #[cfg(target_family="windows")] { windows::pause_if_owns_terminal(); }
                std::process::exit(1);
            } else {
                println!("{}", error);
                #[cfg(target_family="windows")] { windows::pause_if_owns_terminal(); }
                return;
            }
        }
    };

    #[cfg(target_family="windows")]
    let pause: Pause = match matches.value_of("pause").unwrap().try_into() {
        Ok(pause) => pause,
        Err(error) => {
            eprintln!("{}", error);
            windows::pause_if_owns_terminal();
            std::process::exit(1);
        }
    };

    if let Err(error) = run(&matches) {
        let _ = error.write_to(&mut stderr(), false);
    }

    #[cfg(target_family="windows")]
    match pause {
        Pause::Always => windows::pause(),
        Pause::Never  => {},
        Pause::Auto   => windows::pause_if_owns_terminal(),
    }
}

fn run(matches: &ArgMatches) -> Result<()> {
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
            let ignore_magic          = args.is_present("ignore-magic");
            let encoding = args.value_of("encoding").unwrap().try_into()?;
            let path = args.value_of("package").unwrap();
            let paths = get_paths(args)?;
            let paths: Option<&[&str]> = if let Some(paths) = &paths {
                Some(paths)
            } else {
                None
            };

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

            drop(reader);

            list(pak, ListOptions {
                order,
                style: if only_names {
                    ListStyle::OnlyNames { null_separated }
                } else {
                    ListStyle::Table { human_readable }
                },
                paths,
            })?;
        }
        ("check", Some(args)) => {
            let null_separated        = args.is_present("print0");
            let ignore_magic          = args.is_present("ignore-magic");
            let ignore_null_checksums = args.is_present("ignore-null-checksums");
            let verbose               = args.is_present("verbose");
            let encoding = args.value_of("encoding").unwrap().try_into()?;
            let path = args.value_of("package").unwrap();
            let paths = get_paths(args)?;
            let paths: Option<&[&str]> = if let Some(paths) = &paths {
                Some(paths)
            } else {
                None
            };

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

            let options = CheckOptions {
                abort_on_error: true,
                ignore_null_checksums,
                null_separated,
                verbose,
                thread_count: get_threads(args)?,
                paths,
            };

            let error_count = check(&pak, &mut file, options)?;

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
            let null_separated           = args.is_present("print0");
            let verbose                  = args.is_present("verbose");
            let ignore_magic             = args.is_present("ignore-magic");
            let dirname_from_compression = args.is_present("dirname-from-compression");
            let encoding = args.value_of("encoding").unwrap().try_into()?;
            let thread_count = get_threads(args)?;
            let path = args.value_of("package").unwrap();
            let paths = get_paths(args)?;
            let paths: Option<&[&str]> = if let Some(paths) = &paths {
                Some(paths)
            } else {
                None
            };

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

            drop(reader);

            unpack(&pak, &mut file, outdir, UnpackOptions {
                dirname_from_compression,
                verbose,
                null_separated,
                paths,
                thread_count,
            })?;
        }
        ("pack", Some(args)) => {
            let thread_count = get_threads(args)?;
            let null_separated = args.is_present("print0");
            let verbose        = args.is_present("verbose");
            let mount_point = args.value_of("mount-point");
            let encoding = args.value_of("encoding").unwrap().try_into()?;
            let version = args.value_of("version").unwrap().parse()?;
            let compression_block_size = parse_size(args.value_of("compression-block-size").unwrap())?;
            if compression_block_size > u32::MAX as usize {
                return Err(Error::new(format!("--compression-block-size too big: {}", compression_block_size)));
            }
            let compression_block_size = if let Some(value) = NonZeroU32::new(compression_block_size as u32) {
                value
            } else {
                return Err(Error::new("--compression-block-size cannot be 0".to_string()));
            };
            let compression_method = parse_compression_method(args.value_of("compression-method").unwrap())?;
            let compression_level = parse_compression_level(args.value_of("compression-level").unwrap())?;
            let path = args.value_of("package").unwrap();
            let paths = if let Some(path_strs) = args.values_of("paths") {
                let mut paths = Vec::<PackPath>::new();

                for path in path_strs {
                    if path.starts_with('@') {
                        // TODO: maybe also read other arguments from file? (in particular mount_point)
                        let path = &path[1..];
                        paths.extend(PackPath::read_from_path(path)?.into_iter());
                    } else {
                        paths.push(path.try_into()?);
                    }
                }

                paths
            } else {
                return Err(Error::new("missing argument: PATH".to_string()));
            };

            pack(path, &paths, PackOptions {
                version,
                mount_point,
                encoding,
                compression_method,
                compression_block_size,
                compression_level,
                verbose,
                null_separated,
                thread_count,
            })?;
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

#[allow(unused)]
#[allow(non_camel_case_types)]
#[allow(non_snake_case)]
#[cfg(target_family="windows")]
mod windows {
    use std::io::Read;

    pub(crate) type DWORD = u32;

    #[link(name = "user32")]
    extern "stdcall" {
        pub(crate) fn GetConsoleProcessList(lpdwProcessList: *mut DWORD, dwProcessCount: DWORD) -> DWORD;
    }

    pub fn pause() {
        println!("Press ENTER to continue...");
        let mut buf = [0];
        let _ = std::io::stdin().read(&mut buf);
    }

    pub fn pause_if_owns_terminal() {
        let mut process_list = [0, 0];
        let count = unsafe { GetConsoleProcessList(process_list.as_mut_ptr() as *mut DWORD, process_list.len() as DWORD) };

        if count == 1 {
            pause();
        }
    }
}
