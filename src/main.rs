#[macro_use]
extern crate tracing;

mod config;
mod schema;
mod db;
mod cli;
mod handler;

use clap::Parser;

fn main() -> anyhow::Result<()> {
    let args = cli::Cli::parse();
    args.run()?;
    Ok(())
}
