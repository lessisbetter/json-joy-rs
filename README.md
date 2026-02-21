# json-joy-rs

Upstream Credit: This repository ports and adapts the upstream `json-joy`
project by streamich.
- Upstream repository: <https://github.com/streamich/json-joy>
- Upstream docs: <https://jsonjoy.com/libs/json-joy-js>
- Upstream package source mirrored here: `packages/*`

`json-joy-rs` is a Rust-first implementation of core `json-joy` CRDT, patch,
diff, and codec functionality, plus native bridges for WASM and Python.

## Highlights

- Core CRDT model and patch operations in Rust (`crates/json-joy`)
- Fixture-driven parity harness against upstream `json-joy@17.67.0`
- Native platform bridges:
  - WASM (`crates/json-joy-wasm`)
  - UniFFI/Python (`crates/json-joy-ffi`, `bindings/python`)

## Repository layout

- `crates/json-joy`: primary core library and parity target
- `crates/json-joy-wasm`: WASM bridge for core model/patch interop
- `crates/json-joy-ffi`: UniFFI bridge crate (`cdylib`)
- `bindings/python`: Python packaging and generated bindings
- `bindings/wasm`: WASM benchmark and interop harness scripts
- `tests/compat`: compatibility fixture corpus, manifest, and xfail policy
- `tests/compat/PARITY_AUDIT.md`: ongoing file-family parity and intentional divergence log
- `bin/`: helper scripts (`compat` fixture generation, binding generation)

## Quick start

```bash
just check
```

`just check` runs formatting, strict clippy (`-D warnings` across all targets/features), compatibility gates, and full workspace tests.

If running cargo directly, use `mise` for pinned toolchains:

```bash
mise x -- cargo clippy --workspace --all-features --all-targets -- -D warnings
mise x -- cargo test --workspace
```

## Compatibility and parity

Generate upstream compatibility fixtures:

```bash
just compat-fixtures
```

Run fixture parity tests:

```bash
just parity-fixtures
```

Run live manual TS<->WASM core differential check:

```bash
just parity-live
```

Run both:

```bash
just parity
```

## Scope note

JS editor ecosystem adapter APIs (Slate/ProseMirror/Quill-specific helpers)
are intentionally out of scope in this Rust/WASM port. For those integrations,
use upstream JS `json-joy`.

`json-pack` NFS protocol families are not a current parity target in this repo.
In particular, upstream `json-pack/src/nfs/v3/**` is intentionally unsupported
for now while core codec and CRDT parity work is prioritized.
