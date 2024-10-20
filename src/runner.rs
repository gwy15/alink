use crate::{
    config::Basic,
    db::{self, Pool},
};
use anyhow::{Context, Result};
use std::{
    collections::HashSet,
    future::Future,
    path::{Path, PathBuf},
    pin::Pin,
    sync::Arc,
};
use tokio::fs;

#[cfg(unix)]
use parking_lot::Mutex;
#[cfg(unix)]
use std::{collections::HashMap, os::unix::fs::MetadataExt};

#[cfg(unix)]
type Cache = HashMap<u64, PathBuf>;

#[derive(Debug, Clone)]
pub struct Ctx {
    pub pool: Pool,
    pub basic: Arc<Basic>,
    pub src: Arc<PathBuf>,
    pub target: Arc<PathBuf>,
    /// 相对于 src 的路径
    pub relative: PathBuf,
    #[cfg(unix)]
    /// 快速定位 inode -> target PathBuf
    pub inode_to_path: Arc<Mutex<Cache>>,

    pub dry_run: bool,
}

pub fn run_on(path: PathBuf, ctx: Ctx) -> Pin<Box<dyn Future<Output = Result<()>>>> {
    Box::pin(async move {
        trace!("run on path {}", path.display());
        if path.is_dir() {
            run_on_dir(path, ctx).await?;
        } else {
            run_on_file(path, ctx).await?;
        }

        Ok(())
    })
}
async fn run_on_dir(path: PathBuf, ctx: Ctx) -> Result<()> {
    anyhow::ensure!(path.is_dir(), "path {} is not a directory", path.display());

    let mut read_dir = fs::read_dir(path).await?;
    while let Some(entry) = read_dir.next_entry().await? {
        let entry_path = entry.path();

        let name = entry.file_name();
        if let Some(s) = name.to_str() {
            if ctx.basic.ignore.contains(s) {
                continue;
            }
        }

        let relative = ctx.relative.join(name);
        let mut ctx = ctx.clone();
        ctx.relative = relative;

        run_on(entry_path, ctx).await?;
    }

    Ok(())
}
async fn run_on_file(file: PathBuf, ctx: Ctx) -> Result<()> {
    if !is_media(&file, &ctx.basic.media_ext)? {
        return Ok(());
    }
    let single_file = ctx.relative.components().count() == 1;
    trace!(
        "relative: {} single_file: {}",
        ctx.relative.display(),
        single_file
    );

    // 1. 检查 DB 里面是否有缓存，避免重复搜索
    if let Some(link_record) = db::Link::from_src(&file, &ctx.pool).await? {
        let linked_path = PathBuf::from(link_record.target);
        if linked_path.exists() {
            info!("DB 缓存存在而且验证成功，跳过: {}", linked_path.display());
            return Ok(());
        } else {
            warn!("DB 缓存存在，但是目标不存在，删除 DB 缓存");
            db::Link::delete(link_record.id, &ctx.pool).await?;
        }
    }

    // 2. 判断理想情况是否已经存在，如果存在直接返回不进行任何操作
    let ideal_target = if single_file {
        let stem = file.file_stem().context("file stem error")?;
        ctx.target.join(stem).join(ctx.relative.as_os_str())
    } else {
        ctx.target.join(ctx.relative)
    };
    if ideal_target.exists() {
        #[cfg(unix)]
        if ideal_target.metadata()?.ino() != file.metadata()?.ino() {
            warn!("目标存在，但是两个文件 inode 不匹配");
        }
        info!("目标已存在，跳过: {}", ideal_target.display());
        db::Link::link(&file, &ideal_target, &ctx.pool).await?;
        return Ok(());
    }

    // 3. 如果缓存也不存在，根据 inode 搜索目录
    #[cfg(unix)]
    if file.metadata()?.nlink() > 1 {
        info!("文件存在硬链接，搜索: {}", file.display());
        let src_inode = file.metadata()?.ino();
        let search_result =
            search_target(ctx.target.as_ref().clone(), src_inode, &ctx.inode_to_path)?;
        if let Some(target) = search_result {
            info!("搜索到目标: {}", target.display());
            db::Link::link(&file, &target, &ctx.pool).await?;
            return Ok(());
        }
    }

    // 4. 真的没有链接，硬链接到理想目标
    let target_parent = ideal_target.parent().context("cannot get target parent")?;
    info!("link {} => {}", file.display(), ideal_target.display());
    if !ctx.dry_run {
        fs::create_dir_all(target_parent).await?;
        fs::hard_link(&file, &ideal_target).await?;
        db::Link::link(&file, &ideal_target, &ctx.pool).await?;
    }

    // 处理其他文件（ass、nfo 啥的）直接拷贝
    for ext in ["ass", "nfo", "jpg", "png", "srt"] {
        let attachment = file.with_extension(ext);
        if attachment.exists() {
            let attachment_target = ideal_target.with_extension(ext);
            info!(
                "copy {} => {}",
                attachment.display(),
                attachment_target.display()
            );
            if !ctx.dry_run {
                fs::copy(attachment, attachment_target).await?;
            }
        }
    }

    Ok(())
}

fn is_media(file: &Path, media_ext: &HashSet<String>) -> Result<bool> {
    let ext = match file.extension() {
        Some(ext) => ext.to_str().context("extension is not a valid utf8")?,
        None => return Ok(false),
    };
    Ok(media_ext.contains(ext))
}

#[cfg(unix)]
fn search_target(path: PathBuf, inode: u64, cache: &Mutex<Cache>) -> Result<Option<PathBuf>> {
    if let Some(target) = cache.lock().remove(&inode) {
        if target.exists() && target.metadata()?.ino() == inode {
            return Ok(Some(target));
        }
    }
    search_target_recursive(path, inode, cache)
}

#[cfg(unix)]
fn search_target_recursive(
    path: PathBuf,
    inode: u64,
    cache: &Mutex<Cache>,
) -> Result<Option<PathBuf>> {
    let read_dir = std::fs::read_dir(path)?;
    for entry in read_dir {
        let entry = entry?;
        let entry_path = entry.path();
        if entry_path.is_dir() {
            if let Some(target) = search_target_recursive(entry_path, inode, cache)? {
                return Ok(Some(target));
            }
        } else {
            // file
            let this_inode = entry_path.metadata()?.ino();
            if this_inode == inode {
                return Ok(Some(entry_path));
            } else {
                cache.lock().insert(this_inode, entry_path.clone());
            }
        }
    }
    Ok(None)
}
