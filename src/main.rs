
mod config;
mod schema;
mod db;
mod cli;

use clap::Parser;

fn main() -> anyhow::Result<()> {
    let args = cli::Cli::parse();
    args.run()?;
    Ok(())
}
