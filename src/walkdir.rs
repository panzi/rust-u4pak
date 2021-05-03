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

use std::{fs::DirEntry, path::Path};

#[derive(Debug)]
pub struct WalkDir {
    stack: Vec<std::fs::ReadDir>,
    follow_links: bool,
    only_files: bool,
}

impl WalkDir {
    #[inline]
    pub fn new(path: impl AsRef<Path>, follow_links: bool, only_files: bool) -> std::io::Result<Self> {
        Ok(Self {
            stack: vec![std::fs::read_dir(path)?],
            follow_links,
            only_files,
        })
    }

    #[inline]
    pub fn follow_links(&self) -> bool {
        self.follow_links
    }

    #[inline]
    pub fn only_files(&self) -> bool {
        self.only_files
    }
}

impl Iterator for WalkDir {
    type Item = std::io::Result<DirEntry>;

    fn next(&mut self) -> Option<std::io::Result<DirEntry>> {
        while let Some(iter) = self.stack.last_mut() {
            if let Some(entry) = iter.next() {
                match entry {
                    Ok(entry) => {
                        match entry.metadata() {
                            Ok(metadata) => {
                                if (!self.follow_links && metadata.file_type().is_symlink()) || !metadata.is_dir() {
                                    return Some(Ok(entry));
                                } else {
                                    // is dir
                                    match std::fs::read_dir(entry.path()) {
                                        Ok(iter) => {
                                            self.stack.push(iter);
                                            if !self.only_files {
                                                return Some(Ok(entry));
                                            }
                                        }
                                        Err(error) => {
                                            return Some(Err(error));
                                        }
                                    }
                                }
                            }
                            Err(error) => {
                                return Some(Err(error));
                            }
                        }
                    }
                    Err(error) => {
                        return Some(Err(error));
                    }
                }
            } else {
                self.stack.pop();
            }
        }
        return None;
    }
}

#[inline]
pub fn walkdir(path: impl AsRef<Path>) -> std::io::Result<WalkDir> {
    WalkDir::new(path, true, true)
}
