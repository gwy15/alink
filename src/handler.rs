use crate::{config, db, searcher};
use anyhow::Result;
use recursive_link::*;
use std::{
    io,
    path::{Path, PathBuf},
};

pub struct Handler<'s> {
    pub basic: &'s config::Basic,
    pub db: db::Pool,
    #[cfg_attr(not(unix), allow(unused))]
    pub searcher: searcher::PathSearcher<'s>,
}

impl Handler<'_> {
    fn should_ignore(&self, path: &Path) -> bool {
        let Some(p) = path.file_name() else {
            return false;
        };
        let Some(p) = p.to_str() else {
            return false;
        };
        self.basic.ignore.contains(p)
    }
    fn perm(&self) -> recursive_link::Perm {
        Perm {
            #[cfg(unix)]
            uid: self.uid,
            #[cfg(unix)]
            gid: self.gid,
            ..Default::default()
        }
    }
    fn handle_link_file(&self, src: &Path, target: &Path) -> Result<FileOperation> {
        // 1. if target exists and have same inode, skip
        #[cfg(unix)]
        let src_inode = src.metadata()?.ino();
        if target.exists() {
            #[cfg(unix)]
            {
                let target_inode = target.metadata()?.ino();
                if src_inode == target_inode {
                    return Ok(FileOperation::Skip);
                } else {
                    // ??? skip now
                    return Ok(FileOperation::Skip);
                }
            }
        }
        // 2. check cached db entry
        let src_s = src.as_os_str().to_string_lossy();
        let mut conn = self.db.get()?;
        let entry = db::Link::from_src(&src_s, &mut conn)?;
        if let Some(entry) = entry {
            let actual_target = PathBuf::from(entry.target);
            let exists = actual_target.exists();
            if exists {
                // 不检查了
                // #[cfg(unix)] {
                //     if src_inode == actual_target.metadata()?.ino() {
                //         return Ok(Some(FileOperation::Skip));
                //     } else {
                //         // ??
                //     }
                // }
                return Ok(FileOperation::Skip);
            } else {
                db::Link::delete(entry.id, &mut conn)?;
            }
        }
        // 3. do a disk search
        #[cfg(unix)]
        if src.metadata()?.nlink() > 1 {
            let config_path = ();
            if let Some(path) = self.searcher.search(src_inode, config_path)? {
                if path.exists() && path.metadata()?.ino() == src_inode {
                    db::Link::link(&src_s, &target_s, &mut conn)?;
                    return Ok(FileOperation::Skip);
                }
            }
        }

        // 4. do db-link first
        debug!("link {} => {}", src.display(), target.display());
        let target_s = src.as_os_str().to_string_lossy();
        db::Link::link(&src_s, &target_s, &mut conn)?;
        return Ok(FileOperation::Link);
    }
}
impl PathHandler for Handler<'_> {
    fn handle_dir(&self, path: &Path, _target: &Path) -> io::Result<DirOperation> {
        let ans = if self.should_ignore(path) {
            DirOperation::Skip
        } else {
            debug!("process directory {}", path.display());
            DirOperation::Process { perm: self.perm() }
        };
        Ok(ans)
    }
    fn handle_file(&self, path: &Path, target: &Path) -> io::Result<FileOperation> {
        if self.should_ignore(path) {
            return Ok(FileOperation::Skip);
        }
        let Some(ext) = path.extension().and_then(|ext| ext.to_str()) else {
            return Ok(FileOperation::Skip);
        };
        if self.basic.link_ext.contains(ext) {
            return self
                .handle_link_file(path, target)
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e));
        }
        if self.basic.copy_ext.contains(ext) {
            // FIXME: copy 移动之后没法跟踪
            debug!("copy {}", path.display());
            let op = FileOperation::Copy { perm: self.perm() };
            return Ok(op);
        }
        Ok(FileOperation::Skip)
    }
    fn handle_symlink(&self, _path: &Path, _target: &Path) -> io::Result<SymLinkOperation> {
        Ok(SymLinkOperation::Skip)
    }
}
