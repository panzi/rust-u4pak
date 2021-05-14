Rust U4Pak
==========

More or less re-implementation of [Python U4Pak](https://github.com/panzi/u4pak)
for fun, ease of use (standalone binary), and speed (multi-threading).

This is a tool to pack, unpack, check, and list the contents of Unreal Engine 4
packages. Under Linux it can also be used to read-only FUSE-mount archives. Note
that only a limited number of pak versions are supported, depending on the kinds
of paks I have seen (version 1, 2, 3, 4, 7).

Encryption is not supported. I haven't seen a pak file that uses encryption and
I have no clue how it would work (e.g. what is the algorithm or where to get the
encrytion key from).

**NOTE:** If you know (cheap) games that use other archive versions please tell me!
Especially if its 5 or 6. There is a change in how certain offsets are handled at
some point, but since I only have an example file of version 7 I don't know if it
happened in version 5, 6, or 7.

Note that sometimes some parts of pak files are zeroed out by games. In that
case sometimes the options `--ignore-magic` and `--force-version=3` (or maybe
another version) may help, but usually too much of the file is zeroed out and
nothing can be read. In particular I've seen the whole footer to be zeroed out
(contains the offset of the file index). My guess would be that this information
is compiled into the game binary somehow, but I have no idea how one would
access that.

Instead of passing arguments you can also put the arguments in a file with the
extension .u4pak and pass the path to that instead. This is useful for Windows
users that aren't used to a terminal. You can even associate the extension with
u4pak.exe so that it will be automatically opened with it when you double click
it. File paths in a .u4pak file are relative to the directory containing the
file. The syntax of these files is not shell syntax. If you don't have any white
space, double quotes (`"`), or hash marks (`#`) in your file names you don't have to
worry about anything. `#` is used to start a comment line (only if it doesn't
touch any non-white space on it's left) and `"` is used to quote arguments
containing white space, `#`, or `"`. In order to write a `"` in a quoted argument you
simply need to double it, meaning an argument that contains nothing but a single
`"` is written as `""""`. Newlines are ignored like any other white space. An
example .u4pak file whould be:

```sh
# This is packing my project:
pack
--version=4
--mount-point=../../..

":rename=/:C:\Users\Alice\My Documents\U4Project\Some Files"
":zlib,rename=/:Some Other Files"
```

If u4pak.exe is run by double clicking or by dropping a .u4pak file on it it
won't immediately close the terminal window, but will instead ask you to press
ENTER. It does this so you have a chance to read the output. Since I don't use
Windows (I cross compile on Linux and test with Wine) I could not test this
particular feature. If it doesn't work please report a bug. In order to force
the "Press ENTER to continue..." message you can pass the argument
`--pause-on-exit=always` (Windows-only).

Usage
-----

```sh
u4pak [--pause-on-exit=<always|never|auto>] [SUBCOMMAND]
```

Or:

```sh
u4pak "C:\Path\to\arguments.u4pak"
```

### Sub-Commands

| Sub-Command | Description                                                        |
| ----------- | ------------------------------------------------------------------ |
| check       | Check consistency of a package                                     |
| help        | Prints general help message or the help of the given subcommand(s) |
| info        | Show summarized information of a package                           |
| list        | List content of a package                                          |
| mount       | Mount package as read-only filesystem (Linux-only)                 |
| pack        | Create a new package                                               |
| unpack      | Unpack content of a package                                        |

For help to the various sub-commands run `u4pak help SUBCOMMAND`.

Related Projects
----------------

* [fezpak](https://github.com/panzi/fezpak): pack, unpack, list and mount FEZ .pak archives
* [psypkg](https://github.com/panzi/psypkg): pack, unpack, list and mount Psychonauts .pkg archives
* [bgebf](https://github.com/panzi/bgebf): unpack, list and mount Beyond Good and Evil .bf archives
* [unvpk](https://github.com/panzi/unvpk): extract, list, check and mount Valve .vpk archives (C++)
* [rust-vpk](https://github.com/panzi/rust-vpk): Rust rewrite of the above (Rust)
* [t2fbq](https://github.com/panzi/t2fbq): unpack, list and mount Trine 2 .fbq archives
* [u4pak](https://github.com/panzi/u4pak): old Python version of this program

GPLv3 License
-------------

Rust U4Pak - pack, unpack, check, list and mount Unreal Engine 4 packages

Copyright (C) 2021 Mathias Panzenböck

[rust-u4pak](https://github.com/panzi/rust-u4pak) is free software: you can
redistribute it and/or modify it under the terms of the GNU General Public
License as published by the Free Software Foundation, either version 3 of the
License, or (at your option) any later version.

rust-u4pak is distributed in the hope that it will be useful, but WITHOUT ANY
WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A
PARTICULAR PURPOSE.  See the GNU General Public License for more details.

You should have received a copy of the GNU General Public License along with
rust-u4pak.  If not, see <https://www.gnu.org/licenses/>.
