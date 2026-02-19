//! `json-pack` — encode JSON (stdin) to MessagePack or CBOR (stdout).
//!
//! Usage:
//!   json-pack [--format msgpack|cbor] [--cbor]
//!
//! Mirrors `packages/json-joy/src/json-cli/json-pack.ts`.

use json_joy::json_cli::{pack, CliError};
use std::io::{self, Read, Write};

fn main() {
    let args: Vec<String> = std::env::args().collect();

    // Parse --format and --cbor flags.
    let mut format = "msgpack".to_string();
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--cbor" => {
                format = "cbor".to_string();
            }
            "--format" => {
                i += 1;
                if let Some(f) = args.get(i) {
                    format = f.clone();
                }
            }
            _ => {}
        }
        i += 1;
    }

    let mut buf = String::new();
    if let Err(e) = io::stdin().read_to_string(&mut buf) {
        eprintln!("{e}");
        std::process::exit(1);
    }

    match pack(buf.trim(), &format) {
        Ok(bytes) => {
            io::stdout().write_all(&bytes).unwrap();
        }
        Err(CliError::UnknownFormat(f)) => {
            eprintln!("Unknown format: {f}");
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    }
}
