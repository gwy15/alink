#[macro_use]
extern crate log;
use alink::{Basic, Config, Rule};
use anyhow::{bail, Context, Result};
use clap::Parser;
use notify::Watcher;
use std::{
    env,
    path::{Path, PathBuf},
    process::exit,
    sync::{mpsc, Arc},
    time::Duration,
};
use tokio::sync::{broadcast, watch};

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

fn load_config(config_path: &Path) -> Result<alink::Config> {
    info!("loading config from path {}", config_path.display());

    trace!(
        "path = {}, meta = {:?}",
        config_path.canonicalize()?.display(),
        config_path.metadata()?
    );

    let config_s = std::fs::read_to_string(&config_path)
        .with_context(|| format!("配置文件 {} 打开失败", config_path.display()))?;
    trace!("config_s = \n{}", config_s);
    let config = toml::from_str(&config_s).context("parse config toml failed")?;
    Ok(config)
}

/// 永远监听 config 文件的变化并将其发布到 config_tx 中 (watch)
fn config_watch_thread(config_path: &Path, config_tx: watch::Sender<Config>) -> Result<()> {
    // watch config file
    let (tx, rx) = std::sync::mpsc::channel();
    let mut config_watcher = notify::watcher(tx, std::time::Duration::from_secs(1))?;
    config_watcher.watch(config_path, notify::RecursiveMode::NonRecursive)?;
    loop {
        match rx.recv() {
            Ok(event) => {
                info!(
                    "config file changed: {:?}, reload config and update config_tx",
                    event
                );
                let config = load_config(config_path).context("load config failed")?;
                config_tx.send(config)?;
            }
            Err(e) => {
                error!("watch config file failed: {:?}", e);
                bail!(e);
            }
        }
    }
}

async fn pool_from_config(config: &Config) -> Result<alink::Pool> {
    let db_uri = format!("sqlite://{}", &config.basic.db_path.display());
    debug!("db uri: {}", db_uri);
    let pool = alink::Pool::connect(&db_uri).await?;
    sqlx::migrate!("./migrations").run(&pool).await?;
    Ok(pool)
}

/// 将 notify 的 mpsc 转发到 tokio 的 broadcast，这个函数只进行转发
fn forward_filesystem_event_thread(
    fs_watcher_rx: mpsc::Receiver<notify::DebouncedEvent>,
    fs_tx: broadcast::Sender<()>,
) -> Result<()> {
    loop {
        match fs_watcher_rx.recv() {
            Ok(event) => {
                info!("filesystem changed: {:?}", event);
                fs_tx.send(())?;
            }
            Err(e) => {
                error!("watch filesystem failed: {:?}", e);
                bail!(e);
            }
        }
    }
}

/// 监听 filesystem 变化
/// # 参数
/// - `config_rx`: 获取 config
/// - `config_change_event`: filesystem 变化事件
/// - `fs_tx`: filesystem 变化事件转发到 tokio 的 broadcast
fn filesystem_watch_thread(
    config_rx: watch::Receiver<Config>,
    config_change_event: mpsc::Receiver<()>,
    fs_tx: broadcast::Sender<()>,
) -> Result<()> {
    let (fs_watcher_tx, fs_watcher_rx) = mpsc::channel();
    let mut watcher = notify::watcher(fs_watcher_tx.clone(), std::time::Duration::from_secs(10))?;
    for rule in config_rx.borrow().rule.iter() {
        info!("watch path {}", rule.src.display());
        watcher.watch(&rule.src, notify::RecursiveMode::Recursive)?;
    }

    // 在独立的线程中转发 fs_watcher_rx 的消息
    std::thread::spawn(move || forward_filesystem_event_thread(fs_watcher_rx, fs_tx));

    // 改变 watch
    loop {
        match config_change_event.recv() {
            Ok(_) => {
                info!("the config file changed, reload the watcher");
                // stop the previous watcher
                std::mem::drop(watcher);
                watcher =
                    notify::watcher(fs_watcher_tx.clone(), std::time::Duration::from_secs(10))?;
                for rule in config_rx.borrow().rule.iter() {
                    info!("watch path {}", rule.src.display());
                    watcher.watch(&rule.src, notify::RecursiveMode::Recursive)?;
                }
                info!("new fs watcher ready");
            }
            Err(e) => {
                error!("watch filesystem failed: {:?}", e);
                bail!(e);
            }
        }
    }
}

async fn run_with_config_rx(mut config_rx: watch::Receiver<Config>, dry_run: bool) -> Result<()> {
    // init
    let mut config = config_rx.borrow_and_update().clone();
    let mut pool = pool_from_config(&config).await?;
    let basic_config = Arc::new(config.basic.clone());

    // 用来向监听 fs 的 thread 通知需要重新拉取 config
    let (config_change_event_tx, config_change_event_rx) = mpsc::channel();
    // 用来广播 fs 发生了变化
    let (fs_tx, mut fs_rx) = broadcast::channel(1);
    let config_rx_clone = config_rx.clone();
    std::thread::spawn(move || {
        if let Err(e) = filesystem_watch_thread(config_rx_clone, config_change_event_rx, fs_tx) {
            error!("filesystem_watch_thread failed: {:?}", e);
            exit(1);
        }
    });

    let mut timer = tokio::time::interval(Duration::from_secs(5 * 60));

    loop {
        tokio::select! {
            _ = fs_rx.recv() => {
                info!("fs change detected");
                run_config(basic_config.clone(), &config.rule, pool.clone(), dry_run).await?;
            }
            r = config_rx.changed() => {
                if r.is_err() {
                    bail!("The config channel has been closed.");
                }
                info!("config file changed");
                config_change_event_tx.send(())?;
                config = config_rx.borrow_and_update().clone();
                pool = pool_from_config(&config).await?;
                run_config(basic_config.clone(), &config.rule, pool.clone(), dry_run).await?;
            }
            _ = timer.tick() => {
                info!("timer tick");
                run_config(basic_config.clone(), &config.rule, pool.clone(), dry_run).await?;
            }
        };
    }
}

async fn run_config(
    basic_config: Arc<Basic>,
    rules: &[Rule],
    pool: alink::Pool,
    dry_run: bool,
) -> Result<()> {
    for rule in rules {
        info!(
            "running rule {} => {}",
            rule.src.display(),
            rule.target.display()
        );
        let src = rule.src.clone();
        let target = rule.target.clone();
        let ctx = alink::Ctx {
            pool: pool.clone(),
            basic: basic_config.clone(),
            src: Arc::new(src.clone()),
            target: Arc::new(target),
            relative: PathBuf::new(),
            #[cfg(unix)]
            inode_to_path: Default::default(),
            dry_run,
        };

        alink::run_on(src, ctx).await?;
    }
    info!("run config success.");
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "debug,sqlx=warn");
    }
    pretty_env_logger::try_init_timed()?;
    let opts = Opts::parse();

    // 用来广播 config 事件的 channel
    let (config_tx, config_rx) = watch::channel(load_config(&opts.config)?);
    let config_path = opts.config.clone();
    std::thread::spawn(move || {
        if let Err(e) = config_watch_thread(&config_path, config_tx) {
            error!("config watch thread failed! {:?}", e);
        }
    });

    run_with_config_rx(config_rx, opts.dry_run).await?;

    Ok(())
}
