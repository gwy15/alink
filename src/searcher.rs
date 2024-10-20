//! search path

use std::{
    collections::{HashMap, HashSet},
    fs, io,
    path::{Path, PathBuf},
};

#[cfg_attr(not(unix), allow(unused))]
pub struct PathSearcher<'s> {
    last_visit: Option<FileYielder<'s>>,
    ignore: &'s HashSet<String>,
    cache: HashMap<u64, PathBuf>,
}
#[cfg_attr(not(unix), allow(unused))]
impl<'s> PathSearcher<'s> {
    pub fn new(ignore: &'s HashSet<String>) -> Self {
        Self {
            last_visit: None,
            cache: Default::default(),
            ignore,
        }
    }
    #[cfg(unix)]
    pub fn search(&mut self, inode: u64, dir: &Path) -> io::Result<Option<PathBuf>> {
        if let Some(path) = self.cache.get(&inode) {
            if path.exists() && path.metadata()?.ino() == inode {
                return Ok(Some(path.clone()));
            } else {
                self.cache.remove(&inode);
            }
        }
        if let Some(last) = self.last_visit.as_mut() {
            for item in last {
                let item = item?;
                let item_inode = item.metadata()?.ino();
                self.cache.insert(item_inode, item.clone());
                if item_inode == inode {
                    return Ok(Some(item));
                }
            }
        }
        // 再搜一遍
        let yielder = FileYielder::new(dir)?.with_ignore(self.ignore);
        self.last_visit = Some(yielder);
        if let Some(last) = self.last_visit.as_mut() {
            for item in last {
                let item = item?;
                let item_inode = item.metadata()?.ino();
                self.cache.insert(item_inode, item.clone());
                if item_inode == inode {
                    return Ok(Some(item));
                }
            }
        }

        Ok(None)
    }
}

struct FileYielder<'s> {
    stack: Vec<fs::ReadDir>,
    ignore: Option<&'s HashSet<String>>,
}
#[cfg_attr(not(unix), allow(unused))]
impl<'s> FileYielder<'s> {
    pub fn new(directory: &Path) -> io::Result<Self> {
        let read_dir = fs::read_dir(directory)?;
        let this = Self {
            stack: vec![read_dir],
            ignore: None,
        };
        Ok(this)
    }
    /// dir name to ignore
    pub fn with_ignore(mut self, ignore: &'s HashSet<String>) -> Self {
        self.ignore = Some(ignore);
        self
    }
    /// yield (file) in directory (recursively)
    fn _next(&mut self) -> Option<io::Result<PathBuf>> {
        let entry = loop {
            let last = self.stack.last_mut()?;
            match last.next() {
                Some(entry) => break entry,
                None => {
                    // read dir is empty, return
                    self.stack.pop();
                    continue;
                }
            }
        };
        let entry = match entry {
            Ok(entry) => entry,
            Err(e) => return Some(Err(e)),
        };
        match self.on_dir_entry(entry) {
            Ok(Some(data)) => return Some(Ok(data)),
            Ok(None) => {
                // read a link, continue
                return self._next();
            }
            Err(e) => return Some(Err(e)),
        }
    }
    fn on_dir_entry(&mut self, entry: fs::DirEntry) -> io::Result<Option<PathBuf>> {
        let file_type = entry.file_type()?;
        let path = entry.path();

        if file_type.is_file() {
            return Ok(Some(path));
        } else if file_type.is_dir() {
            if let Some(dir_name) = path.file_name().and_then(|name| name.to_str()) {
                if let Some(ignore) = self.ignore {
                    if ignore.contains(dir_name) {
                        return Ok(None);
                    }
                }
            }
            // visit dir
            let read_dir = fs::read_dir(path)?;
            self.stack.push(read_dir);
            return Ok(None);
        }
        // symlink
        Ok(None)
    }
}
impl Iterator for FileYielder<'_> {
    type Item = io::Result<PathBuf>;
    fn next(&mut self) -> Option<Self::Item> {
        self._next()
    }
}
