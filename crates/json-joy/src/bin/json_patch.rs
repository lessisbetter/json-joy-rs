//! `json-patch` — apply a JSON Patch (RFC 6902) to a document.
//!
//! Usage:
//!   json-patch '<patch-array-json>'
//!
//! The document is read from stdin. The patch operations are the first argument.
//!
//! Mirrors `packages/json-joy/src/json-cli/json-patch.ts`.

use std::io::{self, Read, Write};
use json_joy::json_cli::apply_json_patch;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let patch = match args.get(1) {
        Some(p) => p.clone(),
        None => {
            eprintln!("First argument must be a JSON patch array.");
            std::process::exit(1);
        }
    };

    let mut buf = String::new();
    if let Err(e) = io::stdin().read_to_string(&mut buf) {
        eprintln!("{e}");
        std::process::exit(1);
    }

    match apply_json_patch(buf.trim(), &patch) {
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
