#[macro_use]
extern crate log;
use anyhow::Result;
use std::{env, path::PathBuf, sync::Arc};

#[tokio::main]
async fn main() -> Result<()> {
    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "debug,sqlx=warn");
    }
    pretty_env_logger::try_init_timed()?;

    let config_s = tokio::fs::read_to_string("config.toml").await?;
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
        };

        alink::run_on(src, ctx).await?;
    }

    Ok(())
}
