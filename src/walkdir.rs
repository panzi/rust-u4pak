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
