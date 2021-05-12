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

use std::{collections::HashMap, ffi::OsStr, fs::File, io::Read, path::Path, time::{Duration, SystemTime, UNIX_EPOCH}};
use std::os::unix::fs::FileExt;
use std::os::linux::fs::MetadataExt;

use cntr_fuse as fuse;
use flate2::bufread::ZlibDecoder;
use fuse::{Filesystem, FileType, Request, ReplyEntry, FileAttr, ReplyAttr, ReplyEmpty, ReplyOpen, ReplyDirectory, ReplyStatfs, ReplyRead, FUSE_ROOT_ID};
use daemonize::{Daemonize, DaemonizeError};
use libc::{ENOENT, EISDIR, EACCES, ENOTDIR, EINVAL, EIO, ENOSYS, O_RDONLY};

use crate::{Error, Pak, Record, Result, pak, record::CompressionBlock, util::{make_pak_path, parse_pak_path}};

#[derive(Debug)]
enum INodeData {
    File {
        offset: u64,
        size: u64,
        uncompressed_size: u64,
        compression_method: u32,
        compression_blocks: Option<Vec<CompressionBlock>>,
        encrypted: bool,
        compression_block_size: u32,
    },
    Dir(HashMap<String, u64>)
}

#[derive(Debug)]
struct INode {
    parent: u64,
    inode: u64,
    data: INodeData,
    stat: FileAttr,
}

impl INode {
    #[inline]
    fn is_dir(&self) -> bool {
        matches!(self.data, INodeData::Dir(_))
    }

    #[allow(unused)]
    #[inline]
    fn is_file(&self) -> bool {
        matches!(self.data, INodeData::File { .. })
    }
}

#[derive(Debug)]
pub struct U4PakFS {
    file: File,
    inodes: Vec<INode>,

    atime:  SystemTime,
    mtime:  SystemTime,
    ctime:  SystemTime,
    crtime: SystemTime,

    uid: u32,
    gid: u32,

    blksize: u64,
    blocks:  u64,
}

impl U4PakFS {
    pub fn new(pak: &Pak, file: File) -> Result<Self> {
        let meta = file.metadata()?;

        let mut u4pakfs = U4PakFS {
            file,
            inodes: Vec::new(),

            atime:  make_time(meta.st_atime(), meta.st_atime_nsec()),
            mtime:  make_time(meta.st_mtime(), meta.st_mtime_nsec()),
            ctime:  make_time(meta.st_ctime(), meta.st_ctime_nsec()),
            crtime: meta.created().unwrap_or(UNIX_EPOCH),

            uid:    meta.st_uid(),
            gid:    meta.st_gid(),

            blksize: meta.st_blksize(),
            blocks:  0,
        };

        u4pakfs.inodes.push(INode {
            parent: FUSE_ROOT_ID,
            inode:  FUSE_ROOT_ID,
            data: INodeData::Dir(HashMap::new()),
            stat: FileAttr {
                ino:    FUSE_ROOT_ID,
                size:   5,
                blocks: 1 + ((5 - 1) / u4pakfs.blksize),
                atime:  u4pakfs.atime,
                mtime:  u4pakfs.mtime,
                ctime:  u4pakfs.ctime,
                crtime: u4pakfs.crtime,
                kind:   FileType::Directory,
                perm:   0o555,
                nlink:  1,
                uid:    u4pakfs.uid,
                gid:    u4pakfs.gid,
                rdev:   0,
                flags:  0,
            },
        });

        let version = pak.version();
        for record in pak.records() {
            u4pakfs.insert(version, record)?;
        }

        Ok(u4pakfs)
    }

    #[inline]
    fn get(&self, inode: u64) -> Option<&INode> {
        self.inodes.get((inode - FUSE_ROOT_ID) as usize)
    }

    fn insert(&mut self, version: u32, record: &Record) -> Result<u64> {
        let mut parent = FUSE_ROOT_ID;
        let path: Vec<_> = parse_pak_path(record.filename()).collect();

        if path.len() > 1 {
            for (index, &name) in path[0..path.len() - 1].iter().enumerate() {
                let new_inode = self.inodes.len() as u64 + FUSE_ROOT_ID;
                let parent_inode = &mut self.inodes[(parent - FUSE_ROOT_ID) as usize];

                if let INodeData::Dir(children) = &mut parent_inode.data {
                    if let Some(&child_inode) = children.get(name) {
                        parent = child_inode;
                    } else {
                        parent_inode.stat.nlink += 1;
                        parent_inode.stat.size += name.len() as u64 + 1;
                        parent_inode.stat.blocks = 1 + ((parent_inode.stat.size - 1) / self.blksize);

                        children.insert(name.to_string(), new_inode);
                        self.inodes.push(INode {
                            parent,
                            inode:  new_inode,
                            data: INodeData::Dir(HashMap::new()),
                            stat: FileAttr {
                                ino:    new_inode,
                                size:   5,
                                blocks: 1 + ((5 - 1) / self.blksize),
                                atime:  self.atime,
                                mtime:  self.mtime,
                                ctime:  self.ctime,
                                crtime: self.crtime,
                                kind:   FileType::Directory,
                                perm:   0o555,
                                nlink:  1,
                                uid:    self.uid,
                                gid:    self.gid,
                                rdev:   0,
                                flags:  0,
                            },
                        });

                        parent = new_inode;
                    }
                } else {
                    return Err(Error::new(format!("{}: not a directory", make_pak_path(path[0..index].iter()))));
                }
            }
        }

        if let Some(&name) = path.last() {
            let new_inode = self.inodes.len() as u64 + FUSE_ROOT_ID;
            let parent_inode = &mut self.inodes[(parent - FUSE_ROOT_ID) as usize];

            if let INodeData::Dir(children) = &mut parent_inode.data {
                if children.contains_key(name) {
                    return Err(Error::new(format!("{}: file already exists", record.filename())));
                }

                parent_inode.stat.nlink += 1;
                parent_inode.stat.size += name.len() as u64 + 1;
                parent_inode.stat.blocks = 1 + ((parent_inode.stat.size - 1) / self.blksize);

                children.insert(name.to_string(), new_inode);

                let atime:  SystemTime;
                let mtime:  SystemTime;
                let ctime:  SystemTime;
                let crtime: SystemTime;
                if let Some(timestamp) = record.timestamp() {
                    atime  = UNIX_EPOCH + Duration::from_secs(timestamp);
                    mtime  = atime;
                    ctime  = atime;
                    crtime = atime;
                } else {
                    atime  = self.atime;
                    mtime  = self.mtime;
                    ctime  = self.ctime;
                    crtime = self.crtime;
                }

                let offset = record.offset();
                let compression_blocks;
                if version < 7 {
                    compression_blocks = (*record.compression_blocks()).clone();
                } else if let Some(blocks) = record.compression_blocks() {
                    compression_blocks = Some(blocks.iter().map(|block| CompressionBlock {
                        start_offset: offset + block.start_offset,
                        end_offset:   offset + block.end_offset,
                    }).collect());
                } else {
                    compression_blocks = None;
                }

                let uncompressed_size = record.uncompressed_size();

                self.inodes.push(INode {
                    parent,
                    inode: new_inode,
                    data: INodeData::File {
                        offset: offset + pak::Pak::header_size(version, record),
                        size: record.size(),
                        uncompressed_size,
                        compression_method: record.compression_method(),
                        compression_blocks,
                        encrypted: record.encrypted(),
                        compression_block_size: record.compression_block_size(),
                    },
                    stat: FileAttr {
                        ino:    new_inode,
                        size:   uncompressed_size,
                        blocks: if uncompressed_size != 0 { 1 + ((uncompressed_size - 1) / self.blksize) } else { 0 },
                        atime,
                        mtime,
                        ctime,
                        crtime,
                        kind:   FileType::RegularFile,
                        perm:   0o444,
                        nlink:  1,
                        uid:    self.uid,
                        gid:    self.gid,
                        rdev:   0,
                        flags:  0,
                    },
                });

            } else {
                return Err(Error::new(format!("{}: not a directory", make_pak_path(path[0..path.len() - 1].iter()))));
            }
        } else {
            return Err(Error::new("empty path".to_string()));
        }

        Ok(0)
    }
}

const TTL: Duration = Duration::from_secs(std::u64::MAX);

impl Filesystem for U4PakFS {
    fn lookup(&mut self, _req: &Request, parent: u64, name: &OsStr, reply: ReplyEntry) {
        if let Some(mut inode_data) = self.get(parent) {
            if "." == name {
                // done
            } else if ".." == name {
                inode_data = if let Some(inode_data) = self.get(inode_data.parent) {
                    inode_data
                } else {
                    return reply.error(ENOENT);
                };
            } else if let INodeData::Dir(children) = &inode_data.data {
                if let Some(name) = name.to_str() {
                    if let Some(&inode) = children.get(name) {
                        inode_data = if let Some(inode_data) = self.get(inode) {
                            inode_data
                        } else {
                            return reply.error(ENOENT);
                        };
                    } else {
                        return reply.error(ENOENT);
                    }
                } else {
                    return reply.error(ENOENT);
                }
            } else {
                return reply.error(ENOTDIR);
            }

            return reply.entry(&TTL, &inode_data.stat, 0);
        } else {
            return reply.error(ENOENT);
        }
    }

    fn getattr(&mut self, _req: &Request, ino: u64, reply: ReplyAttr) {
        if let Some(inode_data) = self.get(ino) {
            return reply.attr(&TTL, &inode_data.stat);
        } else {
            return reply.error(ENOENT);
        }
    }

    fn access(&mut self, _req: &Request, ino: u64, mask: u32, reply: ReplyEmpty) {
        if let Some(inode_data) = self.get(ino) {
            if mask & inode_data.stat.perm as u32 != mask {
                return reply.error(EACCES);
            }
            return reply.ok();
        } else {
            return reply.error(ENOENT);
        }
    }


    fn opendir(&mut self, _req: &Request, ino: u64, _flags: u32, reply: ReplyOpen) {
        if let Some(inode_data) = self.get(ino) {
            if !inode_data.is_dir() {
                return reply.error(ENOTDIR);
            }
            return reply.opened(ino, 0);
        } else {
            return reply.error(ENOENT);
        }
    }

    fn readdir(&mut self, _req: &Request, ino: u64, _fh: u64, offset: i64, mut reply: ReplyDirectory) {
        if let Some(inode_data) = self.get(ino) {
            if let INodeData::Dir(children) = &inode_data.data {
                // Offset will be the last offset FUSE already got, or 0 at the start.
                // Therefore I give the entries offsets starting with 1, so that the
                // start is no special case. The offset 0 is just the last offset FUSE
                // already got, which is not a real entry.
                let mut entry_offset = 1;
                if offset < entry_offset {
                    reply.add(ino, entry_offset, FileType::Directory, ".");
                }
                entry_offset += 1;
                if offset < entry_offset {
                    reply.add(inode_data.parent, entry_offset, FileType::Directory, "..");
                }
                entry_offset += 1;
                for (name, &child_inode) in children {
                    if offset < entry_offset {
                        let child = self.get(child_inode).unwrap();
                        if reply.add(child.inode, entry_offset, if child.is_dir() {
                            FileType::Directory
                        } else {
                            FileType::RegularFile
                        }, name) {
                            break;
                        }
                    }
                    entry_offset += 1;
                }
                return reply.ok();
            } else {
                return reply.error(ENOTDIR);
            }
        } else {
            return reply.error(ENOENT);
        }
    }

    fn statfs(&mut self, _req: &Request, _ino: u64, reply: ReplyStatfs) {
        reply.statfs(
            /* blocks  */ self.blocks,
            /* bfree   */ 0,
            /* bavail  */ 0,
            /* files   */ self.inodes.len() as u64,
            /* ffree   */ 0,
            /* bsize   */ self.blksize as u32,
            /* namelen */ std::u32::MAX,
            /* frsize  */ 0);
    }

    fn open(&mut self, _req: &Request, ino: u64, flags: u32, reply: ReplyOpen) {
        if let Some(inode_data) = self.get(ino) {
            if inode_data.is_dir() {
                return reply.error(EISDIR);
            } else if flags & 3 != O_RDONLY as u32 {
                return reply.error(EACCES);
            }
            return reply.opened(ino, 0);
        } else {
            return reply.error(ENOENT);
        }
    }

    fn read(&mut self, _req: &Request, ino: u64, _fh: u64, read_offset: i64, read_size: u32, reply: ReplyRead) {
        if let Some(inode_data) = self.get(ino) {
            if let INodeData::File {
                    compression_method,
                    compression_block_size,
                    compression_blocks,
                    encrypted,
                    offset,
                    size,
                    uncompressed_size,
            } = &inode_data.data {
                if *encrypted {
                    return reply.error(ENOSYS);
                }

                if read_offset < 0 {
                    return reply.error(EINVAL);
                }

                let uncompressed_size = *uncompressed_size;
                if read_offset as u64 >= uncompressed_size {
                    return reply.data(&[]);
                }

                let offset = *offset;
                match *compression_method {
                    pak::COMPR_NONE => {
                        let read_offset = offset + read_offset as u64;
                        let end_offset = std::cmp::min(offset + uncompressed_size, read_offset + read_size as u64);
                        let read_size = end_offset - read_offset;

                        let mut buffer = vec![0; read_size as usize];
                        if let Err(error) = self.file.read_exact_at(&mut buffer, read_offset) {
                            return reply.error(error.raw_os_error().unwrap_or(EIO));
                        }

                        return reply.data(&buffer);
                    }
                    pak::COMPR_ZLIB => {
                        if let Some(blocks) = compression_blocks {
                            let compression_block_size = *compression_block_size as u64;
                            let end_offset = std::cmp::min(read_offset as u64 + read_size as u64, uncompressed_size);
                            let start_block_index = (read_offset as u64 / compression_block_size) as usize;
                            let end_block_index   = (end_offset         / compression_block_size) as usize;
                            let mut current_offset = compression_block_size * start_block_index as u64;

                            let mut in_buffer = Vec::new();
                            let mut out_buffer = Vec::new();
                            for block in &blocks[start_block_index..end_block_index + 1] {
                                let block_size = block.end_offset - block.start_offset;
                                in_buffer.resize(block_size as usize, 0);
                                if let Err(error) = self.file.read_exact_at(&mut in_buffer, block.start_offset) {
                                    return reply.error(error.raw_os_error().unwrap_or(EIO));
                                }

                                let mut zlib = ZlibDecoder::new(&in_buffer[..]);

                                if current_offset < read_offset as u64 {
                                    out_buffer.resize(std::cmp::min(compression_block_size, end_offset) as usize, 0);
                                    if let Err(error) = zlib.read_exact(&mut out_buffer) { // TODO: maybe not read_exact()?
                                        return reply.error(error.raw_os_error().unwrap_or(EIO));
                                    }
                                    out_buffer.drain(0..read_offset as usize);
                                } else if end_offset < current_offset + compression_block_size {
                                    let remaining = end_offset - current_offset;
                                    let index = out_buffer.len();
                                    out_buffer.resize(index + remaining as usize, 0);
                                    if let Err(error) = zlib.read_exact(&mut out_buffer[index..]) { // TODO: maybe not read_exact()?
                                        return reply.error(error.raw_os_error().unwrap_or(EIO));
                                    }
                                } else if let Err(error) = zlib.read_to_end(&mut out_buffer) {
                                    return reply.error(error.raw_os_error().unwrap_or(EIO));
                                }
                                current_offset += compression_block_size;
                            }

                            return reply.data(&out_buffer);
                        } else {
                            // version 2 has compression support, but not compression blocks
                            let size = *size;
                            let mut in_buffer = vec![0u8; size as usize];
                            let mut out_buffer = Vec::with_capacity(uncompressed_size as usize);
                            if let Err(error) = self.file.read_exact_at(&mut in_buffer, offset) {
                                return reply.error(error.raw_os_error().unwrap_or(EIO));
                            }

                            let mut zlib = ZlibDecoder::new(&in_buffer[..]);
                            if let Err(error) = zlib.read_to_end(&mut out_buffer) {
                                return reply.error(error.raw_os_error().unwrap_or(EIO));
                            }

                            if (read_size as usize) < out_buffer.len() {
                                return reply.data(&out_buffer[0..read_size as usize]);
                            }
                            return reply.data(&out_buffer);
                        }
                    }
                    _ => return reply.error(ENOSYS)
                }
            } else {
                return reply.error(EISDIR);
            }
        } else {
            return reply.error(ENOENT);
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct MountOptions {
    pub foreground: bool,
    pub debug: bool,
}

impl Default for MountOptions {
    fn default() -> Self {
        Self {
            foreground: false,
            debug: false,
        }
    }
}

impl From<DaemonizeError> for Error {
    fn from(error: DaemonizeError) -> Self {
        Error::new(error.to_string())
    }
}

pub fn mount(pak: Pak, file: File, mountpt: impl AsRef<Path>, options: MountOptions) -> Result<()> {
    let mountpt = match mountpt.as_ref().canonicalize() {
        Ok(mountpt) => mountpt,
        Err(error) => return Err(Error::io_with_path(error, mountpt))
    };

    let mut fuse_options = vec![
        OsStr::new("fsname=u4pakfs"),
        OsStr::new("subtype=u4pakfs"),
        OsStr::new("ro")
    ];

    let foreground;
    if options.debug {
        foreground = true;
        fuse_options.push(OsStr::new("debug"));
    } else {
        foreground = options.foreground;
    }

    let fs = U4PakFS::new(&pak, file)?;

    drop(pak);

    if !foreground {
        let daemonize = Daemonize::new()
            .working_directory("/")
            .umask(0);

        daemonize.start()?;
    }

    fuse::mount(fs, mountpt, &fuse_options)?;

    Ok(())
}

fn make_time(mut time: i64, mut nsec: i64) -> SystemTime {
    if time <= 0 {
        time = -time;
        if nsec < 0 {
            nsec = -nsec;
        } else {
            time += 1;
            nsec = 1_000_000_000 - nsec;
        }

        return UNIX_EPOCH - Duration::new(time as u64, nsec as u32);
    } else {
        if nsec < 0 {
            time -= 1;
            nsec += 1_000_000_000;
        }
        return UNIX_EPOCH + Duration::new(time as u64, nsec as u32);
    }
}
