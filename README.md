# json-joy-rs

Rust-first JSON CRDT implementation with Python bindings.

## Repository layout

- `crates/json-joy-core`: core Rust library (CRDT engine).
- `crates/json-joy-ffi`: UniFFI bridge crate exported as `cdylib` for other languages.
- `tools/embedded-uniffi-bindgen`: pinned local bindgen CLI wrapper.
- `bindings/python`: generated Python package artifacts.
- `bin/`: helper scripts for generating bindings.

## Quick start

```bash
make check
```

Generate bindings:

```bash
make bindings-python
```

Generate compatibility fixtures from upstream `json-joy@17.67.0`:

```bash
make compat-fixtures
```

If you call Rust tooling directly, prefer `mise` to ensure the pinned toolchain:

```bash
mise x -- cargo check
```
