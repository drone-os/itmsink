//! Command Line Interface.

use crate::PORTS_COUNT;
use anyhow::{bail, Error};
use std::{
    ffi::{OsStr, OsString},
    os::unix::ffi::OsStrExt,
    path::PathBuf,
};
use structopt::StructOpt;

/// ITM protocol parser.
#[allow(intra_doc_link_resolution_failure)]
#[derive(Debug, StructOpt)]
pub struct Cli {
    /// Pass many times for more log output
    #[structopt(long = "verbosity", short = "v", parse(from_occurrences))]
    pub verbosity: u64,
    /// Read raw data from INPUT instead of STDIN
    #[structopt(short = "i", long = "input", name = "INPUT", parse(from_os_str))]
    pub input: Option<PathBuf>,
    /// Output specification in form of "ports[:path]"; "ports" is a
    /// comma-separated list of stimulus port numbers, or "all" to select all
    /// ports; "path" is a file path for the output, or STDOUT if omitted
    #[structopt(
        name = "OUTPUT",
        default_value = "all",
        parse(try_from_os_str = parse_output)
    )]
    pub outputs: Vec<Output>,
}

/// Output specification.
#[derive(Debug)]
pub struct Output {
    /// Selected stimulus ports for the output. If the vector is empty, all
    /// ports are selected.
    pub ports: Vec<u8>,
    /// Path to a file for the output, or STDOUT if `None`.
    pub path: Option<PathBuf>,
}

fn parse_port(src: &str) -> Result<u8, Error> {
    let port = src.parse()?;
    if port as usize >= PORTS_COUNT {
        bail!(
            "Stimulus port number can't be greater than {}",
            PORTS_COUNT - 1
        );
    }
    Ok(port)
}

fn parse_ports(src: &str) -> Result<Vec<u8>, Error> {
    Ok(if src == "all" {
        Vec::new()
    } else {
        src.split(',').map(parse_port).collect::<Result<_, _>>()?
    })
}

fn parse_output(src: &OsStr) -> Result<Output, OsString> {
    let mut chunks = src.as_bytes().splitn(2, |&b| b == b':');
    let ports = String::from_utf8(chunks.next().unwrap().to_vec())
        .map_err(Into::into)
        .and_then(|ports| parse_ports(ports.as_str()))
        .map_err(|err| err.to_string())?;
    let path = chunks.next().map(|path| OsStr::from_bytes(path).into());
    Ok(Output { ports, path })
}
