use anyhow::{bail, Context};
use std::{fs, io, path::Path};
use tracing::*;

pub trait PathHandler {
    fn handle_file(&self, path: &Path, target: &Path) -> io::Result<FileOperation>;
    fn handle_dir(&self, path: &Path, target: &Path) -> io::Result<DirOperation>;
    fn handle_symlink(&self, path: &Path, target: &Path) -> io::Result<SymLinkOperation>;
}

#[derive(Debug, Default)]
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

#[derive(Debug)]
pub enum FileOperation {
    Skip,
    Link,
    Copy { perm: Perm },
}
#[derive(Debug)]
pub enum DirOperation {
    Skip,
    Process { perm: Perm },
}
#[derive(Debug)]
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
) -> anyhow::Result<()> {
    let (src, target) = (src.as_ref(), target.as_ref());
    if !src.exists() {
        bail!("Source path does not exist. Create the source path first.");
    }
    if src.is_file() {
        bail!("src is not directory");
    }
    if !target.exists() {
        bail!("Target path does not exist. Create the target path first.");
    }
    if target.is_file() {
        bail!("target is not directory");
    }

    run_on_dir(src, target, handle)?;

    Ok(())
}

fn run_on_dir<H: PathHandler>(src: &Path, target: &Path, handle: &H) -> anyhow::Result<()> {
    for src_entry in fs::read_dir(src)? {
        let src_entry = src_entry?;
        let src_file_name = src_entry.file_name();
        let src_path = src_entry.path();
        let target_path = target.join(src_file_name);
        debug!("Run {} => {}", src_path.display(), target_path.display());
        if src_path.is_file() {
            let op = handle
                .handle_file(&src_path, &target_path)
                .with_context(|| format!("handle file {} failed", src_path.display()))?;
            debug!("handle file {} op={op:?}", src_path.display());
            match op {
                FileOperation::Skip => continue,
                FileOperation::Link => {
                    fs::hard_link(src_path, target_path).context("hard link failed")?;
                }
                FileOperation::Copy { perm } => {
                    fs::copy(src_path, &target_path).context("copy failed")?;
                    perm.apply(&target_path).context("apply perm failed")?;
                }
            }
        } else if src_path.is_symlink() {
            let op = handle
                .handle_symlink(&src_path, &target_path)
                .with_context(|| format!("handle symlink {} failed", src_path.display()))?;
            debug!("handle symlink {} op={op:?}", src_path.display());
            match op {
                SymLinkOperation::Skip => continue,
                SymLinkOperation::LinkTarget => {
                    let src_path =
                        fs::canonicalize(&src_path).context("canonicalize src path failed")?;
                    fs::hard_link(src_path, target)?;
                }
                SymLinkOperation::LinkSymlink => {
                    fs::hard_link(src_path, target_path).context("create hard link failed")?;
                }
                SymLinkOperation::CopyTarget { perm } => {
                    fs::copy(src_path, &target_path).context("copy target failed")?;
                    perm.apply(&target_path).context("apply perm failed")?;
                }
            }
        } else if src_path.is_dir() {
            let op = handle
                .handle_dir(&src_path, &target_path)
                .with_context(|| format!("handle dir {} failed", src_path.display()))?;
            debug!("handle dir {} op={op:?}", src_path.display());
            match op {
                DirOperation::Skip => continue,
                DirOperation::Process { perm } => {
                    if !target_path.exists() {
                        fs::create_dir(&target_path).context("create dir failed")?;
                        perm.apply(&target_path)
                            .context("apply perm to dir failed")?;
                    }
                    run_on_dir(&src_path, &target_path, handle).with_context(|| {
                        format!(
                            "recursive run on {} => {} failed",
                            src_path.display(),
                            target_path.display()
                        )
                    })?;
                }
            }
        }
    }

    Ok(())
}
