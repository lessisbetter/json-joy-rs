//! `json-pointer` — look up a JSON Pointer (RFC 6901) in a document.
//!
//! Usage:
//!   json-pointer '<pointer>'
//!
//! The document is read from stdin. The pointer is the first argument.
//!
//! Mirrors `packages/json-joy/src/json-cli/json-pointer.ts`.

use json_joy::json_cli::lookup_pointer;
use std::io::{self, Read, Write};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let pointer = match args.get(1) {
        Some(p) => p.clone(),
        None => {
            eprintln!("First argument must be a JSON Pointer.");
            std::process::exit(1);
        }
    };

    let mut buf = String::new();
    if let Err(e) = io::stdin().read_to_string(&mut buf) {
        eprintln!("{e}");
        std::process::exit(1);
    }

    match lookup_pointer(buf.trim(), &pointer) {
        Ok(result) => {
            io::stdout().write_all(result.as_bytes()).unwrap();
            io::stdout().write_all(b"\n").unwrap();
        }
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    }
}
