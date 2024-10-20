use std::{fs, io, path::Path};

pub trait PathHandler {
    fn handle_file(&self, path: &Path, target: &Path) -> io::Result<FileOperation>;
    fn handle_dir(&self, path: &Path, target: &Path) -> io::Result<DirOperation>;
    fn handle_symlink(&self, path: &Path, target: &Path) -> io::Result<SymLinkOperation>;
}

#[derive(Default)]
pub struct Perm {
    #[cfg(unix)]
    pub uid: Option<u32>,
    #[cfg(unix)]
    pub gid: Option<u32>,
    pub permissions: Option<std::fs::Permissions>,
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

pub fn link_dir<H: PathHandler>(
    src: impl AsRef<Path>,
    target: impl AsRef<Path>,
    handle: &H,
) -> io::Result<()> {
    let (src, target) = (src.as_ref(), target.as_ref());
    if !src.exists() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Source path does not exist. Create the source path first.",
        ));
    }
    if src.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "src is not directory",
        ));
    }
    if !target.exists() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Target path does not exist. Create the target path first.",
        ));
    }
    if target.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "target is not directory",
        ));
    }

    run_on_dir(src, target, handle)?;

    Ok(())
}

fn run_on_dir<H: PathHandler>(src: &Path, target: &Path, handle: &H) -> io::Result<()> {
    for src_entry in fs::read_dir(&src)? {
        let src_entry = src_entry?;
        let src_file_name = src_entry.file_name();
        let src_path = src_entry.path();
        let target_path = target.join(src_file_name);
        if src_path.is_file() {
            match handle.handle_file(&src_path, &target_path)? {
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
            match handle.handle_symlink(&src_path, &target_path)? {
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
            match handle.handle_dir(&src_path, &target_path)? {
                DirOperation::Skip => continue,
                DirOperation::Process { perm } => {
                    // mkdir target_path
                    fs::create_dir(&target_path)?;
                    perm.apply(&target_path)?;
                    run_on_dir(&src_path, &target_path, handle)?;
                }
            }
        }
    }

    Ok(())
}
