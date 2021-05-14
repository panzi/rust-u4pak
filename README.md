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

File Format
-----------

Byte order is little endian and the character encoding of file names seems to be
ASCII (or ISO-8859-1/UTF-8 that coincidentally only uses ASCII compatiple
characters).

Offsets and sizes seem to be 64bit or at least unsigned 32bit integers. If
interpreted as 32bit integers all sizes (except the size of file names) and offsets
are followed by another 32bit integer of the value 0, which makes me guess these
are 64bit values. Also some values exceed the range of signed 32bit integers, so
they have to be at least unsigned 32bit integers. This information was reverse
engineered from the Elemental [Demo](https://wiki.unrealengine.com/Linux_Demos)
for Linux (which contains a 2.5 GB .pak file).

Basic layout:

* Data Records
* Index
  * Index Header
  * Index Records
* Footer

In order to parse a file you need to read the footer first. The footer contains
an offset pointer to the start of the index records. The index records then
contain offset pointers to the data records.

Some games seem to zero out parts of the file. In particular the footer, which
makes it pretty much impossible to read the file without manual analysis and
guessing. I suspect these games have the footer included somewhere in the game
binary. If it's not obfuscated one might be able to find it using the file
magic (given that the file magic is even included)?

### Record

    Offset  Size  Type         Description
         0     8  uint64_t     offset
         8     8  uint64_t     size (N)
        16     8  uint64_t     uncompressed size
        24     4  uint32_t     compression method:
                                  0x00 ... none
                                  0x01 ... zlib
                                  0x10 ... bias memory
                                  0x20 ... bias speed
    if version <= 1
        28     8  uint64_t     timestamp
    end
         ?    20  uint8_t[20]  data sha1 hash
    if version >= 3
     if compression method != 0x00
      ?+20     4  uint32_t     block count (M)
      ?+24  M*16  CB[M]        compression blocks
     end
         ?     1  uint8_t      is encrypted
       ?+1     4  uint32_t     The uncompressed size of each compression block.
    end                        The last block can be smaller, of course.

### Compression Block (CB)

Size: 16 bytes

    Offset  Size  Type         Description
         0     8  uint64_t     compressed data block start offset.
                               version <= 4: offset is absolute to the file
                               version 7: offset is relative to the offset
                                          field in the corresponding Record
         8     8  uint64_t     compressed data block end offset.
                               There may or may not be a gap between blocks.
                               version <= 4: offset is absolute to the file
                               version 7: offset is relative to the offset
                                          field in the corresponding Record

### Data Record

    Offset  Size  Type            Description
         0     ?  Record          file metadata (offset field is 0, N = compressed_size)
         ?     N  uint8_t[N]      file data

### Index Record

    Offset  Size  Type            Description
         0     4  uint32_t        file name size (S)
         4     S  char[S]         file name (includes terminating null byte)
       4+S     ?  Record          file metadata

### Index

    Offset  Size  Type            Description
         0     4  uint32_t        mount point size (S)
         4     S  char[S]         mount point (includes terminating null byte)
       S+4     4  uint32_t        record count (N)
       S+8     ?  IndexRecord[N]  records

### Footer

Size: 44 bytes

    Offset  Size  Type         Description
         0     4  uint32_t     magic: 0x5A6F12E1
         4     4  uint32_t     version: 1, 2, 3, 4, or 7
         8     8  uint64_t     index offset
        16     8  uint64_t     index size
        24    20  uint8_t[20]  index sha1 hash

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
