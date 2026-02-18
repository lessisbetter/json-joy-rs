//! `json-unpack` — decode MessagePack or CBOR (stdin) to JSON (stdout).
//!
//! Usage:
//!   json-unpack [--format msgpack|cbor] [--cbor]
//!
//! Mirrors `packages/json-joy/src/json-cli/json-unpack.ts`.

use std::io::{self, Read, Write};
use json_joy::json_cli::{unpack, CliError};

fn main() {
    let args: Vec<String> = std::env::args().collect();

    // Parse --format and --cbor flags.
    let mut format = "msgpack".to_string();
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--cbor" => { format = "cbor".to_string(); }
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

    let mut buf = Vec::new();
    if let Err(e) = io::stdin().read_to_end(&mut buf) {
        eprintln!("{e}");
        std::process::exit(1);
    }

    match unpack(&buf, &format) {
        Ok(json) => {
            io::stdout().write_all(json.as_bytes()).unwrap();
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
