use anyhow::{Context, Result};
use parking_lot::Mutex;
use std::path::PathBuf;

use crate::{config, handler::Handler, searcher};

#[derive(clap::Parser)]
pub struct Cli {
    #[clap(short, long, help = "config toml path")]
    config: PathBuf,
}

impl Cli {
    pub fn run(self) -> Result<()> {
        tracing_subscriber::fmt::init();
        let config_s =
            std::fs::read_to_string(self.config.as_path()).context("Cannot read config file")?;
        let config = toml::from_str::<config::Config>(&config_s)?;
        let db = crate::db::new_pool(config.db_url.clone())
            .context("Create db with db_url failed. Please check config db_url")?;

        for rule in config.rule.iter() {
            Self::run_rule(&config.basic, rule, db.clone())?;
        }

        Ok(())
    }
    fn run_rule(basic: &config::Basic, rule: &config::Rule, db: crate::db::Pool) -> Result<()> {
        let (src, target) = (rule.src.as_path(), rule.target.as_path());
        let task_name = format!("{} => {}", src.display(), target.display());
        info!("Running task {task_name}...");

        let handler = Handler {
            basic,
            rule,
            db,
            searcher: Mutex::new(searcher::PathSearcher::new(&basic.ignore)),
        };

        recursive_link::link_dir(src, target, &handler)
            .with_context(|| format!("Run task {task_name} failed"))?;
        info!("Task {task_name} finished.");
        Ok(())
    }
}
