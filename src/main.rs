#![deny(elided_lifetimes_in_paths)]
#![warn(clippy::pedantic)]

use anyhow::Result;
use env_logger::Builder as LoggerBuilder;
use itmsink::cli::Cli;
use log::Level;
use structopt::StructOpt;

fn main() -> Result<()> {
    let args = Cli::from_args();
    let log_level = match args.verbosity {
        0 => Level::Error,
        1 => Level::Warn,
        2 => Level::Info,
        3 => Level::Debug,
        _ => Level::Trace,
    };
    LoggerBuilder::new()
        .filter(Some(env!("CARGO_PKG_NAME")), log_level.to_level_filter())
        .filter(None, Level::Warn.to_level_filter())
        .try_init()?;
    args.run()?;
    Ok(())
}
