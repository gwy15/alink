use crate::config;
use anyhow::Result;
use recursive_link::*;
use std::path::Path;

pub struct Handler {
    pub basic: config::Basic,
    pub rule: config::Rule,
}

impl Handler {
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
    fn pre_link_check(&self, file: &Path) -> Result<()> {
        let ideal = file.to
        Ok(())
    }
}
impl PathHandler for Handler {
    fn handle_dir(&self, path: &std::path::Path) -> DirOperation {
        if self.should_ignore(path) {
            DirOperation::Skip
        } else {
            debug!("process directory {}", path.display());
            DirOperation::Process { perm: self.perm() }
        }
    }
    fn handle_file(&self, path: &Path) -> FileOperation {
        if self.should_ignore(path) {
            return FileOperation::Skip;
        }
        let Some(ext) = path.extension().and_then(|ext| ext.to_str()) else {
            return FileOperation::Skip;
        };
        if self.basic.link_ext.contains(ext) {
            debug!("link {}", path.display());
            return FileOperation::Link;
        }
        if self.basic.copy_ext.contains(ext) {
            // FIXME: copy 移动之后没法跟踪
            debug!("copy {}", path.display());
            return FileOperation::Copy { perm: self.perm() };
        }
        FileOperation::Skip
    }
    fn handle_symlink(&self, _path: &Path) -> SymLinkOperation {
        SymLinkOperation::Skip
    }
}
