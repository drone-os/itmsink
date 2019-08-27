#![deny(elided_lifetimes_in_paths)]
#![warn(clippy::pedantic)]

use exitfailure::ExitFailure;
use itmsink::cli::Cli;
use structopt::StructOpt;

fn main() -> Result<(), ExitFailure> {
    let args = Cli::from_args();
    args.verbosity.setup_env_logger(env!("CARGO_PKG_NAME"))?;
    args.run()?;
    Ok(())
}
