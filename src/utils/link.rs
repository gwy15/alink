use std::{
    fs, io,
    path::{Path, PathBuf},
};

pub struct PathHandler {
    pub file: Box<dyn FnMut(&Path) -> FileOperation>,
    pub dir: Box<dyn FnMut(&Path) -> DirOperation>,
    pub symlink: Box<dyn FnMut(&Path) -> SymLinkOperation>,
}

pub struct Perm {
    #[cfg(unix)]
    uid: Option<u32>,
    #[cfg(unix)]
    gid: Option<u32>,
    permissions: Option<std::fs::Permissions>,
}
impl Perm {
    pub fn apply(self, path: &Path) -> io::Result<()> {
        #[cfg(unix)]
        if self.uid.is_some() || self.uid.is_some() {
            std::os::unix::fs::chown(path, self.uid, self.gid)?;
        }
        if let Some(perm) = self.permissions {
            std::fs::set_permissions(path, perm)?;
        }
        Ok(())
    }
}

pub enum FileOperation {
    Skip,
    Link,
    Copy { perm: Perm },
}
pub enum DirOperation {
    Skip,
    Process { perm: Perm },
}
pub enum SymLinkOperation {
    Skip,
    /// follow symlink and hard link its final target
    LinkTarget,
    /// Hardlink to the symlink
    LinkSymlink,
    /// Copy source file
    CopyTarget {
        perm: Perm,
    },
}

pub fn link_dir(src: PathBuf, target: PathBuf, mut handle: PathHandler) -> io::Result<()> {
    if src.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "src is not directory",
        ));
    }
    if !target.exists() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "target does not exist. Create the target first.",
        ));
    }
    if target.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "target is not directory",
        ));
    }

    run_on_dir(src, target, &mut handle)?;

    Ok(())
}

fn run_on_dir(src: PathBuf, target: PathBuf, handle: &mut PathHandler) -> io::Result<()> {
    for src_entry in fs::read_dir(&src)? {
        let src_entry = src_entry?;
        let src_file_name = src_entry.file_name();
        let src_path = src_entry.path();
        let target_path = target.join(src_file_name);
        if src_path.is_file() {
            match (handle.file)(&src_path) {
                FileOperation::Skip => continue,
                FileOperation::Link => {
                    fs::hard_link(src_path, target_path)?;
                }
                FileOperation::Copy { perm } => {
                    fs::copy(src_path, &target_path)?;
                    perm.apply(&target_path)?;
                }
            }
        } else if src_path.is_symlink() {
            match (handle.symlink)(&src_path) {
                SymLinkOperation::Skip => continue,
                SymLinkOperation::LinkTarget => {
                    let target = fs::canonicalize(&target_path)?;
                    fs::hard_link(src_path, target)?;
                }
                SymLinkOperation::LinkSymlink => {
                    fs::hard_link(src_path, target_path)?;
                }
                SymLinkOperation::CopyTarget { perm } => {
                    fs::copy(src_path, &target_path)?;
                    perm.apply(&target_path)?;
                }
            }
        } else if src_path.is_dir() {
            match (handle.dir)(&src_path) {
                DirOperation::Skip => continue,
                DirOperation::Process { perm } => {
                    // mkdir target_path
                    fs::create_dir(&target_path)?;
                    perm.apply(&target_path)?;
                    run_on_dir(src_path, target_path, handle)?;
                }
            }
        }
    }

    Ok(())
}
