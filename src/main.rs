#[macro_use]
extern crate log;
use anyhow::{Context, Result};
use clap::Parser;
use std::{env, path::PathBuf, sync::Arc};

#[derive(Debug, Parser)]
pub struct Opts {
    #[clap(long = "dry-run", short = 'd', help = "不实际进行 link")]
    dry_run: bool,

    #[clap(
        long = "config",
        short = 'c',
        help = "Config file path",
        default_value = "config.toml"
    )]
    config: PathBuf,
}

#[tokio::main]
async fn main() -> Result<()> {
    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "debug,sqlx=warn");
    }
    pretty_env_logger::try_init_timed()?;

    let opts = Opts::parse();

    let config_s = tokio::fs::read_to_string(&opts.config)
        .await
        .with_context(|| format!("配置文件 {} 打开失败", opts.config.display()))?;
    let config: alink::Config = toml::from_str(&config_s)?;

    let db_uri = format!("sqlite://{}", &config.basic.db_path.display());
    debug!("db uri: {}", db_uri);
    let pool = alink::Pool::connect(&db_uri).await?;
    sqlx::migrate!("./migrations").run(&pool).await?;

    let basic = Arc::new(config.basic);

    for rule in config.rule {
        let src = rule.src.clone();
        let target = rule.target.clone();
        let ctx = alink::Ctx {
            pool: pool.clone(),
            basic: basic.clone(),
            src: Arc::new(src.clone()),
            target: Arc::new(target),
            relative: PathBuf::new(),
            #[cfg(unix)]
            inode_to_path: Default::default(),
            dry_run: opts.dry_run,
        };

        alink::run_on(src, ctx).await?;
    }

    Ok(())
}
