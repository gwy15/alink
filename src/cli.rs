use anyhow::{Context, Result};
use recursive_link::*;
use std::path::{Path, PathBuf};

use crate::config;

#[derive(clap::Parser)]
pub struct Cli {
    #[clap(short, long, help = "config toml path")]
    config: PathBuf,
}

impl config::Basic {
    fn should_ignore(&self, path: &Path) -> bool {
        let Some(p) = path.file_name() else {
            return false;
        };
        let Some(p) = p.to_str() else {
            return false;
        };
        self.ignore.contains(p)
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
}
impl PathHandler for config::Basic {
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
        if self.link_ext.contains(ext) {
            debug!("link {}", path.display());
            return FileOperation::Link;
        }
        if self.copy_ext.contains(ext) {
            debug!("copy {}", path.display());
            return FileOperation::Copy { perm: self.perm() };
        }
        FileOperation::Skip
    }
    fn handle_symlink(&self, _path: &Path) -> SymLinkOperation {
        SymLinkOperation::Skip
    }
}

impl Cli {
    pub fn run(self) -> Result<()> {
        tracing_subscriber::fmt::init();
        let config_s =
            std::fs::read_to_string(self.config.as_path()).context("Cannot read config file")?;
        let config = toml::from_str::<config::Config>(&config_s)?;
        for rule in config.rule.iter() {
            Self::run_rule(&config.basic, rule)?;
        }

        Ok(())
    }
    fn run_rule(basic: &config::Basic, rule: &config::Rule) -> Result<()> {
        let (src, target) = (rule.src.as_path(), rule.target.as_path());
        let task_name = format!("{} => {}", src.display(), target.display());
        info!("Running task {task_name}...");
        link_dir(src, target, basic).with_context(|| format!("Run task {task_name} failed"))?;
        info!("Task {task_name} finished.");
        Ok(())
    }
}
