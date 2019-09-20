//! ITM protocol.

use crate::{
    output::{Output, Stream},
    PORTS_COUNT,
};
use failure::Error;
use log::{debug, warn};
use smallvec::SmallVec;
use std::{
    cell::{Cell, RefCell},
    ops::{Generator, GeneratorState},
    pin::Pin,
    rc::Rc,
};

/// ITM protocol parser.
pub struct Parser<'cli> {
    pipe: Rc<Cell<u8>>,
    gen: Pin<Box<dyn Generator<Yield = (), Return = Result<!, Error>> + 'cli>>,
}

enum Timestamp {
    Local { tc: u8 },
    Global1,
    Global2,
}

type Streams<'cli> = SmallVec<[&'cli RefCell<Stream>; 2]>;

impl<'cli> Parser<'cli> {
    /// Creates a new [`Parser`].
    pub fn new(outputs: &'cli [Output<'cli>]) -> Result<Self, Error> {
        let pipe = Rc::new(Cell::new(0));
        let gen = Box::pin(parser(outputs, Rc::clone(&pipe)));
        let mut parser = Self { pipe, gen };
        parser.resume()?;
        Ok(parser)
    }

    /// Feeds a byte to the parser.
    pub fn pump(&mut self, byte: u8) -> Result<(), Error> {
        debug!("BYTE 0b{0:08b} 0x{0:02X} {1:?}", byte, char::from(byte));
        self.pipe.set(byte);
        self.resume()
    }

    fn resume(&mut self) -> Result<(), Error> {
        match self.gen.as_mut().resume() {
            GeneratorState::Yielded(()) => Ok(()),
            GeneratorState::Complete(Err(err)) => Err(err),
        }
    }
}

fn outputs_map<'cli>(outputs: &'cli [Output<'cli>]) -> [Streams<'cli>; PORTS_COUNT] {
    let mut map: [Streams<'_>; PORTS_COUNT] = Default::default();
    for Output { ports, output } in outputs {
        if ports.is_empty() {
            for outputs in &mut map {
                outputs.push(output);
            }
        } else {
            for port in *ports {
                map[*port as usize].push(output);
            }
        }
    }
    map
}

#[allow(clippy::too_many_lines, clippy::cognitive_complexity)]
fn parser<'cli>(
    outputs: &'cli [Output<'cli>],
    pipe: Rc<Cell<u8>>,
) -> impl Generator<Yield = (), Return = Result<!, Error>> + 'cli {
    let outputs = outputs_map(outputs);
    let mut bytes = SmallVec::<[u8; 16]>::new();
    macro_rules! next_byte {
        () => {{
            yield;
            pipe.get()
        }};
    }
    macro_rules! recycle {
        ($payload:ident) => {
            for &byte in $payload.iter().rev() {
                bytes.push(byte);
            }
        };
    }
    static move || loop {
        bytes.push(next_byte!());
        while let Some(byte) = bytes.pop() {
            if byte == 0 {
                let mut zeros = 8;
                let mut payload = SmallVec::<[u8; 8]>::new();
                loop {
                    let byte = next_byte!();
                    payload.push(byte);
                    zeros += byte.trailing_zeros();
                    if byte != 0 {
                        if zeros >= 47 {
                            synchronization_packet(zeros);
                        } else {
                            warn!("Bad synchronization packet with {} zeros", zeros);
                            recycle!(payload);
                        }
                        break;
                    }
                }
            } else if byte == 0b0111_0000 {
                warn!("Overflow");
            } else if byte & 0b0000_1011 == 0b0000_1000 {
                let sh = byte << 5 >> 7;
                let ex = byte << 1 >> 5;
                if byte >> 7 == 0 {
                    extension_packet(sh, ex, &[]);
                    continue;
                }
                let mut payload = SmallVec::<[u8; 4]>::with_capacity(4);
                loop {
                    let byte = next_byte!();
                    payload.push(byte);
                    if byte >> 7 == 0 {
                        extension_packet(sh, ex, &payload);
                        break;
                    } else if payload.len() == 4 {
                        warn!("Bad extension packet");
                        recycle!(payload);
                        break;
                    }
                }
            } else if byte & 0b0000_1011 == 0 {
                let kind = if byte & 0b1000_1111 == 0
                    && byte & 0b0111_0000 != 0b0000_0000
                    && byte & 0b0111_0000 != 0b0111_0000
                {
                    let payload = byte << 1 >> 5;
                    timestamp_packet(&Timestamp::Local { tc: 0 }, &[payload]);
                    continue;
                } else if byte & 0b1100_1111 == 0b1100_0000 {
                    let tc = byte << 2 >> 6;
                    Timestamp::Local { tc }
                } else if byte == 0b1001_0100 {
                    Timestamp::Global1
                } else if byte == 0b1011_0100 {
                    Timestamp::Global2
                } else {
                    warn!("Invalid header");
                    continue;
                };
                let mut payload = SmallVec::<[u8; 4]>::with_capacity(4);
                loop {
                    let byte = next_byte!();
                    payload.push(byte);
                    if byte >> 7 == 0 {
                        timestamp_packet(&kind, &payload);
                        break;
                    } else if payload.len() == 4 {
                        warn!("Bad local timestamp packet");
                        recycle!(payload);
                        break;
                    }
                }
            } else {
                let software = byte & 0b100 == 0;
                let address = byte >> 3;
                let size = match byte & 0b11 {
                    0b01 => 1,
                    0b10 => 2,
                    0b11 => 4,
                    _ => {
                        warn!("Invalid header");
                        continue;
                    }
                };
                let mut payload = SmallVec::<[u8; 4]>::with_capacity(size);
                while payload.len() < size {
                    let byte = next_byte!();
                    payload.push(byte);
                }
                source_packet(software, address, &payload, &outputs)?;
            }
        }
        bytes.shrink_to_fit();
    }
}

fn synchronization_packet(zeros: u32) {
    debug!("Synchronized with {} zeros", zeros);
}

fn extension_packet(sh: u8, ex: u8, payload: &[u8]) {
    debug!(
        "Extension packet sh={}, ex={}, payload={:?}",
        sh, ex, payload
    );
}

fn timestamp_packet(timestamp: &Timestamp, payload: &[u8]) {
    match timestamp {
        Timestamp::Local { tc } => {
            debug!("Local timestamp tc={}, ts={:?}", tc, payload);
        }
        Timestamp::Global1 => {
            debug!("Global timestamp 1 ts={:?}", payload);
        }
        Timestamp::Global2 => {
            debug!("Global timestamp 2 ts={:?}", payload);
        }
    }
}

fn source_packet(
    software: bool,
    port: u8,
    payload: &[u8],
    outputs: &[Streams<'_>],
) -> Result<(), Error> {
    debug!(
        "{} packet {:?} {:?}",
        if software { "Software" } else { "Hardware" },
        payload,
        String::from_utf8_lossy(payload)
    );
    for output in &outputs[port as usize] {
        output.borrow_mut().write(payload)?;
    }
    Ok(())
}
