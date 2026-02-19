# json-joy-rs

Rust-first JSON CRDT implementation with Python bindings.

## Repository layout

- `crates/json-joy`: core Rust library (CRDT engine and upstream package parity target).
- `crates/json-joy-ffi`: UniFFI bridge crate exported as `cdylib` for other languages.
- `crates/json-joy-wasm`: coarse-grained WASM bridge crate for core model/patch JS interop.
- `tools/embedded-uniffi-bindgen`: pinned local bindgen CLI wrapper.
- `bindings/python`: generated Python package artifacts.
- `bindings/wasm`: wasm benchmark harness and scripts.
- `bin/`: helper scripts for generating bindings.

## Quick start

```bash
make check
```

Generate bindings:

```bash
make bindings-python
```

Build and run the current WASM coarse-API benchmark:

```bash
make wasm-bench
```

Generate compatibility fixtures from upstream `json-joy@17.67.0`:

```bash
make compat-fixtures
```

Run fixture parity checks against the pinned compatibility corpus:

```bash
make parity-fixtures
```

Run the live TS<->WASM core differential check (manual only, not in `test-gates`):

```bash
make parity-live
```

Note: JS editor ecosystem adapters (Slate/ProseMirror/Quill-specific helpers)
are intentionally out of scope here. For those integrations, use upstream JS
`json-joy` directly.

Run both:

```bash
make parity
```

If you call Rust tooling directly, prefer `mise` to ensure the pinned toolchain:

```bash
mise x -- cargo check
```
