//! Validate foreign encoder output with the real Rust protocol decoder.
use std::io::{self, Read};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut bytes = Vec::new();
    io::stdin()
        .take(neuromesh_protocol::MAX_FRAME as u64 + 1)
        .read_to_end(&mut bytes)?;
    neuromesh_protocol::decode(&bytes)?;
    println!("valid");
    Ok(())
}
