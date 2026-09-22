//! Reference encoder for the polyglot NMP/1 Ping contract.
use neuromesh_protocol::{encode, Envelope, Message};
use std::io::{self, Write};
fn uint(s: &str) -> Result<u64, Box<dyn std::error::Error>> {
    if s.is_empty()
        || s.len() > 20
        || !s.bytes().all(|b| b.is_ascii_digit())
        || (s.len() > 1 && s.starts_with('0'))
    {
        return Err("expected canonical unsigned decimal".into());
    }
    Ok(s.parse()?)
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 3 {
        return Err("expected: NODE_HEX MESSAGE_ID(1..u64) NONCE(0..u64)".into());
    }
    let e = Envelope::new(
        args[0].parse()?,
        uint(&args[1])?,
        Message::Ping {
            nonce: uint(&args[2])?,
        },
    );
    io::stdout().write_all(&encode(&e)?)?;
    Ok(())
}
