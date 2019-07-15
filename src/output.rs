//! ITM output.

use crate::cli;
use failure::Error;
use std::{
    cell::RefCell,
    fs::{File, OpenOptions},
    io::{self, Stdout, Write},
};

/// Opened output.
pub struct Output<'cli> {
    /// Stimulus ports.
    pub ports: &'cli [u8],
    /// Output stream.
    pub output: RefCell<Stream>,
}

/// Output stream.
pub enum Stream {
    /// Standard output.
    Stdout(Stdout),
    /// File output.
    File(File),
}

impl<'cli> Output<'cli> {
    /// Opens all output streams.
    pub fn open_all(outputs: &'cli [cli::Output]) -> io::Result<Vec<Output<'cli>>> {
        outputs
            .iter()
            .map(|cli::Output { ports, path }| {
                match path {
                    Some(path) => OpenOptions::new().write(true).open(path).map(Stream::File),
                    None => Ok(Stream::Stdout(io::stdout())),
                }
                .map(|output| Self {
                    ports,
                    output: RefCell::new(output),
                })
            })
            .collect()
    }
}

impl Stream {
    /// Writes to the output stream.
    pub fn write(&mut self, data: &[u8]) -> Result<(), Error> {
        match self {
            Stream::Stdout(stdout) => write_stream(stdout, data),
            Stream::File(file) => write_stream(file, data),
        }
    }
}

fn write_stream<T: Write>(stream: &mut T, data: &[u8]) -> Result<(), Error> {
    stream.write_all(data)?;
    stream.flush()?;
    Ok(())
}
