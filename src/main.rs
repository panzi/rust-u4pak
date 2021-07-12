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
use terminal_size::{terminal_size, Width};

use pak::{COMPR_NONE, Variant};
use std::{convert::TryInto, io::stderr, num::{NonZeroU32, NonZeroUsize}};
use std::io::BufReader;
use std::fs::File;

#[cfg(target_family="windows")]
use std::convert::TryFrom;

pub mod pak;
pub use pak::{Pak, Options, COMPR_ZLIB};

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
pub use util::parse_size;

pub mod decode;
pub mod encode;

pub mod filter;
pub use filter::Filter;

pub mod unpack;
pub use unpack::{unpack, UnpackOptions};

pub mod pack;
pub use pack::{pack, PackOptions, PackPath};

pub mod check;
pub use check::{check, CheckOptions};

pub mod walkdir;
pub mod io;
pub mod reopen;
pub mod args;

#[cfg(target_os="linux")]
pub mod mount;
#[cfg(target_os="linux")]
pub use mount::{mount, MountOptions};

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
        .help("An Unreal Engine 4 pak file")
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

fn arg_variant<'a, 'b>() -> Arg<'a, 'b> {
    Arg::with_name("variant")
        .long("variant")
        .short("a")
        .takes_value(true)
        .default_value("standard")
        .help("Pak variant: 'standard' or 'conan_exiles'.")
}

fn arg_ignore_magic<'a, 'b>() -> Arg<'a, 'b> {
    Arg::with_name("ignore-magic")
        .long("ignore-magic")
        .takes_value(false)
        .help("Ignore file magic.")
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
        .help(
            "Number of threads to use for the operation. \
            'auto' means use the number of logical cores on your computer.")
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

const DEFAULT_BLOCK_SIZE_STR: &str = "65536";

fn make_app<'a, 'b>() -> App<'a, 'b> {
    let width = if let Some((Width(width), _)) = terminal_size() {
        width as usize
    } else {
        120
    };

    let app = App::new("Rust U4Pak")
        .set_term_width(width)
        .about("\n\
                This is a tool to pack, unpack, check, and list the contents of Unreal Engine 4 packages. \
                Note that only a limited number of pak versions are supported, depending on the kinds of \
                paks I have seen.For reading that is version 1, 2, 3, 4, 7, for writing it is 1, 2, 3.\n\
                \n\
                Encryption is not supported. I haven't seen a pak file that uses encryption and I have \
                no clue how it would work (e.g. what is the algorithm or where to get the encrytion key \
                from).\n\
                \n\
                Note that sometimes some parts of pak files are zeroed out by games. In that case \
                sometimes the options --ignore-magic and --force-version=3 (or maybe another version) \
                may help, but usually too much of the file is zeroed out and nothing can be read. \
                In particular I've seen the whole footer to be zeroed out (contains the offset of the \
                file index). My guess would be that this information is compiled into the game binary \
                somehow, but I have no idea how one would access that.\n\
                \n\
                Instead of passing arguments you can also put the arguments in a file with the extension \
                .u4pak and pass the path to that instead. This is useful for Windows users that aren't \
                used to a terminal. You can even associate the extension with u4pak.exe so that it will \
                be automatically opened with it when you double click it. File paths in a .u4pak file are \
                relative to the directory containing the file. The syntax of these files is not shell \
                syntax. If you don't have any white space, double quotes (\"), or hash marks (#) in your \
                file names you don't have to worry about anything. # is used to start a comment line (only \
                if it doesn't touch any non-white space on it's left) and \" is used to quote arguments \
                containing white space, #, or \". In order to write a \" in an quoted argument you simply \
                need to double it, meaning an argument that contains nothing but a single \" is written \
                as \"\"\"\". Newlines are ignored like any other white space. An example .u4pak file \
                whould be:\n\
                \n\
                \t# This is packing my project:\n\
                \tpack\n\
                \t--version=4\n\
                \t--mount-point=../../..\n\
                \t\n\
                \t\":rename=/:C:\\Users\\Alice\\My Documents\\U4Project\\Some Files\"\n\
                \t\":zlib,rename=/:Some Other Files\"\n\
                \n\
                If u4pak.exe is run by double clicking or by dropping a .u4pak file on it it won't \
                immediately close the terminal window, but will instead ask you to press ENTER. It does \
                this so you have a chance to read the output. Since I don't use Windows (I cross compile \
                on Linux and test with Wine) I could not test this particular feature. It it doesn't work \
                please report a bug. In order to force the \"Press ENTER to continue...\" message you can \
                pass the argument --pause-on-exit=always (Windows-only).\n\
                \n\
                Homepage: https://github.com/panzi/rust-u4pak\n\
                Report issues to: https://github.com/panzi/rust-u4pak/issues")
        .version("1.3.0")
        .global_setting(AppSettings::VersionlessSubcommands)
        .author("Mathias Panzenböck <grosser.meister.morti@gmx.net>");

    #[cfg(target_family="windows")]
    let app = app
        .arg(Arg::with_name("pause-on-exit")
            .long("pause-on-exit")
            .default_value("auto")
            .takes_value(true)
            .help("Wait for user to press ENTER on exit. Possible values: always, never, auto."));

    let app = app
        .subcommand(SubCommand::with_name("info")
            .alias("i")
            .about("Show summarized information of a package")
            .arg(arg_variant())
            .arg(arg_human_readable())
            .arg(arg_ignore_magic())
            .arg(arg_encoding())
            .arg(arg_force_version())
            .arg(arg_package()))
        .subcommand(SubCommand::with_name("list")
            .alias("l")
            .about("List content of a package")
            .arg(arg_variant())
            .arg(Arg::with_name("only-names")
                .long("only-names")
                .short("n")
                .takes_value(false)
                .help(
                    "Only print file names. \
                    This is useful for use with xargs and the like."))
            .arg(Arg::with_name("no-header")
                .long("no-header")
                .short("H")
                .takes_value(false)
                .conflicts_with("only-names")
                .help("Don't print table header"))
            .arg(Arg::with_name("sort")
                .long("sort")
                .short("s")
                .takes_value(true)
                .value_name("ORDER")
                .help(
                    "Sort order of list as comma separated keys:\n\
                    \n\
                    * p, path                   - path of the file inside the package\n\
                    * o, offset                 - offset inside of the package\n\
                    * s, size                   - size of the data embedded in the package\n\
                    * u, uncompressed-size      - size of the data when uncompressed\n\
                    * c, compression-method     - the compression method (zlib or none)\n\
                    * b, compression-block-size - size of blocks a compressed file is split into\n\
                    * t, timestamp              - timestamp of a file (only in pak version 1)\n\
                    * e, encrypted              - whether the file is encrypted\n\
                    \n\
                    You can invert the sort order by prepending - to the key. E.g.:\n\
                    \n\
                    u4pak list --sort=-size,-timestamp,name")
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
            .about("Check consistency of a package")
            .arg(Arg::with_name("abort-on-error")
                .long("abort-on-error")
                .takes_value(false)
                .help("Stop on the first found error."))
            .arg(arg_variant())
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
            .about("Unpack content of a package")
            .arg(arg_variant())
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
                .help(
                    "Put files that where compressed into separate folders. \
                     The folder names will be 'none' and 'zlib'."))
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
            .about("Create a new package")
            .arg(arg_variant())
            .arg(Arg::with_name("version")
                .long("version")
                .short("V")
                .takes_value(true)
                .help(
                    "Create package of given VERSION. Supported versions are: 1, 2, and 3 \
                    [default: 3 when --variant=standard, 4 when --variant=conan_exiles]"))
            .arg(Arg::with_name("mount-point")
                .long("mount-point")
                .short("m")
                .takes_value(true)
                .help("Mount-point field of the package."))
            .arg(Arg::with_name("compression-method")
                .long("compression-method")
                .short("c")
                .takes_value(true)
                .default_value("none")
                .help(
                    "Default compression method. Note that files <= 100 bytes are never \
                    compressed because the compression overhead would make them actually \
                    bigger. Maybe this limit might be even raised."))
            .arg(Arg::with_name("compression-block-size")
                .long("compression-block-size")
                .short("b")
                .takes_value(true)
                .default_value(DEFAULT_BLOCK_SIZE_STR)
                .help("Default compresison block size."))
            .arg(Arg::with_name("compression-level")
                .long("compression-level")
                .short("l")
                .takes_value(true)
                .default_value("default")
                .help(
                    "Default compression level. Allowed values are the integers from 1 to 9, \
                    or the strings 'fast' (=1), 'best' (=9), and 'default' (=6)."))
            .arg(arg_encoding())
            .arg(arg_print0())
            .arg(arg_threads())
            .arg(arg_verbose())
            .arg(arg_package())
            .arg(Arg::with_name("paths")
                .index(2)
                .multiple(true)
                .value_name("PATH")
                .help(
                    "Pack these files or directories. You can overload certain settings for a path using a special syntax, e.g.:\n\
                    \n\
                    Linux/Unix:\n\
                    \tu4pak pack Archive.pak :zlib,level=7,block_size=65536,rename=/Foo/Bar:/Some/Folder\n\
                    \n\
                    Windows:\n\
                    \tu4pak pack Archive.pak :zlib,level=7,block_size=65536,rename=/Foo/Bar:C:\\Some\\Folder\n\
                    \n\
                    This means add the fiels from the folder '/Some/Folder' ('C:\\Some\\Folder') \
                    from your hard disk, use zlib compression at compression level 7 with a \
                    compression block size of 65536 bytes, and rename the folder to be 'Foo/Bar' \
                    inside of the pak archive file.\n\
                    \n\
                    Instead of 'zlib' you can also write 'none' to not compress the files from the \
                    given path. If you don't say any of either the default value provided by \
                    --compression-method is used. Same goes for all the other parameters. \
                    If you don't specify 'rename' then the same path is used for the folder inside \
                    of the pak archive as the files on your hard disk have.\n\
                    \n\
                    This is handy if you want to compress some files, but not all:\n\
                    \tu4pak pack Archive.pak :zlib,rename=/:ZlibFiles :none,rename=/:UncompressedFiles
                    \n\
                    If the default parameters are all you need (and you provide relative paths \
                    that don't need renaming) you can simply say e.g.:\n\
                    \n\
                    Linux/Unix:\n\
                    \tu4pak pack Archive.pak Some/Folder\n\
                    \n\
                    Windows:\n\
                    \tu4pak pack Archive.pak Some\\Folder\n\
                    ")));

    #[cfg(target_os = "linux")]
    let app = app.subcommand(SubCommand::with_name("mount")
        .alias("m")
        .about("Mount package as read-only filesystem")
        .arg(arg_variant())
        .arg(arg_ignore_magic())
        .arg(arg_encoding())
        .arg(arg_force_version())
        .arg(Arg::with_name("foregound")
            .long("foreground")
            .short("f")
            .takes_value(false)
            .help("Keep process in foreground."))
        .arg(Arg::with_name("debug")
            .long("debug")
            .short("g")
            .takes_value(false)
            .help("Debug mode. Implies --foreground."))
        .arg(arg_package())
        .arg(Arg::with_name("mountpt")
            .index(2)
            .required(true)
            .value_name("MOUNTPT")));

    app
}

fn main() {
    let args_from_file = match args::get_args_from_file() {
        Ok(args_from_file) => args_from_file,
        Err(error) => {
            let _ = error.write_to(&mut stderr(), false);
            #[cfg(target_family="windows")] { windows::pause_if_owns_terminal(); }
            return;
        }
    };

    let matches = if let Some(args) = args_from_file {
        make_app().get_matches_from_safe_borrow(args.iter())
    } else {
        make_app().get_matches_safe()
    };

    let matches = match matches {
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
    let pause: Pause = match matches.value_of("pause-on-exit").unwrap().try_into() {
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
            let variant = args.value_of("variant").unwrap().try_into()?;
            let human_readable = args.is_present("human-readable");
            let ignore_magic   = args.is_present("ignore-magic");
            let encoding = args.value_of("encoding").unwrap().try_into()?;
            let path = args.value_of("package").unwrap();

            let force_version = if let Some(version) = args.value_of("force-version") {
                Some(version.parse()?)
            } else {
                None
            };

            let pak = Pak::from_path(&path, Options {
                variant,
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
            let order = order.as_ref().map(|order| &order[..]);

            let variant = args.value_of("variant").unwrap().try_into()?;
            let human_readable = args.is_present("human-readable");
            let null_separated = args.is_present("print0");
            let only_names     = args.is_present("only-names");
            let ignore_magic   = args.is_present("ignore-magic");
            let no_header      = args.is_present("no-header");
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
                variant,
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
                    ListStyle::Table { human_readable, no_header }
                },
                paths,
            })?;
        }
        ("check", Some(args)) => {
            let null_separated        = args.is_present("print0");
            let ignore_magic          = args.is_present("ignore-magic");
            let ignore_null_checksums = args.is_present("ignore-null-checksums");
            let abort_on_error        = args.is_present("abort-on-error");
            let verbose               = args.is_present("verbose");
            let variant = args.value_of("variant").unwrap().try_into()?;
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
                variant,
                ignore_magic,
                encoding,
                force_version,
            })?;

            let options = CheckOptions {
                variant,
                abort_on_error,
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
            let variant = args.value_of("variant").unwrap().try_into()?;
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
                variant,
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
            let variant = args.value_of("variant").unwrap().try_into()?;
            let thread_count = get_threads(args)?;
            let null_separated = args.is_present("print0");
            let verbose        = args.is_present("verbose");
            let mount_point = args.value_of("mount-point");
            let encoding = args.value_of("encoding").unwrap().try_into()?;
            let version = if let Some(version) = args.value_of("version") {
                version.parse()?
            } else {
                match variant {
                    Variant::Standard => 3,
                    Variant::ConanExiles => 4,
                }
            };
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
                    paths.push(path.try_into()?);
                }

                paths
            } else {
                return Err(Error::new("missing argument: PATH".to_string()));
            };

            pack(path, &paths, PackOptions {
                variant,
                version,
                mount_point,
                compression_method,
                compression_block_size,
                compression_level,
                encoding,
                verbose,
                null_separated,
                thread_count,
            })?;
        }
        #[cfg(target_os = "linux")]
        ("mount", Some(args)) => {
            let foreground   = args.is_present("foreground");
            let debug        = args.is_present("debug");
            let ignore_magic = args.is_present("ignore-magic");
            let variant = args.value_of("variant").unwrap().try_into()?;
            let encoding = args.value_of("encoding").unwrap().try_into()?;
            let path = args.value_of("package").unwrap();
            let mountpt = args.value_of("mountpt").unwrap();

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
                variant,
                ignore_magic,
                encoding,
                force_version,
            })?;

            drop(reader);

            mount(pak, file, mountpt, MountOptions {
                foreground,
                debug,
            }).map_err(|error| error.with_path_if_none(path))?;
        }
        ("", _) => {
            let mut buf = Vec::new();
            make_app().write_long_help(&mut buf)?;
            let message = std::str::from_utf8(&buf)?;

            return Err(Error::new(format!(
                "Error: Missing sub-command!\n\
                 \n\
                 {}", message
            )));
        }
        (cmd, _) => {
            let mut buf = Vec::new();
            make_app().write_long_help(&mut buf)?;
            let message = std::str::from_utf8(&buf)?;

            return Err(Error::new(format!(
                "Error: Unknown subcommand: {}\n\
                 \n\
                 {}",
                 cmd, message
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
