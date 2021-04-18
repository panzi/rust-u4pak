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

pub mod pak;
pub use pak::Pak;

pub mod result;
pub use result::{Error, Result};

pub mod sort;
pub use sort::{DEFAULT_ORDER, SortKey, parse_order};

pub mod record;
pub use record::Record;

pub enum Filter<'a> {
    None,
    Paths(Vec<&'a str>),
}

impl<'a> Filter<'a> {
    pub fn new(args: &'a clap::ArgMatches) -> Self {
        if let Some(paths) = args.values_of("paths") {
            if paths.len() == 0 {
                Filter::None
            } else {
                Filter::Paths(paths.collect())
            }
        } else {
            Filter::None
        }
    }

    pub fn as_ref(&self) -> Option<&[&'a str]> {
        match self {
            Filter::None => None,
            Filter::Paths(paths) => Some(&paths[..])
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
        .help("Print verbose output.")
}

fn run() -> Result<()> {
    let app = App::new("VPK - Valve Packages")
        .version("1.0.0")
        .author("Mathias Panzenböck <grosser.meister.morti@gmx.net>");

    let app = app
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
            .arg(Arg::with_name("null")
                .long("null")
                .short("z")
                .requires("only-names")
                .takes_value(false)
                .help(
                    "Separate file names with NULL bytes. \
                     This is useful for use with xargs --null, to be sure that \
                     possible new lines in file names aren't interpreted as \
                     file name separators."))
            .arg(arg_human_readable())
            .arg(arg_package())
            .arg(arg_paths()));

    let matches = app.get_matches();

    match matches.subcommand() {
        ("list", Some(args)) => {
            let order = if let Some(order) = args.value_of("sort") {
                Some(parse_order(order)?)
            } else {
                None
            };
            let order = match &order {
                Some(order) => &order[..],
                None => &DEFAULT_ORDER[..],
            };

            let human_readable = args.is_present("human-readable");
            let null_separated = args.is_present("null");
            let only_names     = args.is_present("only-names");
            let path           = args.value_of("package").unwrap();
/*
            let pak = Pak::from_path(path)?;

            list(&pak, ListOptions {
                order,
                style: if only_names {
                    ListStyle::OnlyNames { null_separated }
                } else {
                    ListStyle::Table { human_readable }
                },
                filter: filter.as_ref(),
            })?;
            */
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
