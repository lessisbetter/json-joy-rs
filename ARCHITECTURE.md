# Architecture

This repository is structured as a Rust-core + bindings monorepo, following the same high-level model as Glean.

1. One authoritative Rust core crate.
2. One dedicated FFI crate exposing a stable cross-language API.
3. Generated language bindings committed/published from `bindings/python/src/json_joy_rs/generated`.
4. One local pinned bindgen tool in the workspace for reproducible generation.

## Crates

- `crates/json-joy-core`
  - Business logic and data structures.
  - No language-specific concerns.

- `crates/json-joy-ffi`
  - UniFFI UDL + exported API surface.
  - Produces `cdylib` for Python consumers.

- `tools/embedded-uniffi-bindgen`
  - Runs UniFFI bindgen from a workspace-controlled version.

## Binding generation flow

1. Build Rust FFI library: `cargo build -p json-joy-ffi`
2. Generate Python bindings: `bin/generate-bindings.sh python`
3. Package artifacts in `bindings/python`.

## Versioning guidance

- `json-joy-core` can evolve rapidly.
- `json-joy-ffi` should provide a stable semver API contract for non-Rust consumers.
- The Python package should track `json-joy-ffi` releases.

## Native parity transition

- Compatibility fixtures and oracle scripts are authoritative test oracles, not
  the desired runtime architecture.
- Runtime-core parity progress is tracked in `CORE_PARITY_MATRIX.md`.
- Production bridge paths in `json-joy-core` have been removed; runtime
  diff/apply/create lifecycle paths are native.
- Patch binary construction is centralized through the native patch family:
  `crates/json-joy-core/src/patch/encode.rs` is the shared encoder path used by
  `patch_builder`.
- Oracle tooling remains for fixture generation and differential verification.
- Differential hardening runs with 40-seed/case deterministic suites across
  runtime/model codecs/patch codecs/patch compaction/schema/util diff.

## Core module layout

- Runtime-heavy modules are organized as folder modules to mirror upstream file
  families and keep porting diffs localized:
  - `crates/json-joy-core/src/model_api/mod.rs` (+ `types.rs`, `ops.rs`, `handles.rs`, `events.rs`, `path.rs`)
  - `crates/json-joy-core/src/model_runtime/mod.rs` (+ `types.rs`, `rga.rs`, `apply.rs`, `view.rs`, `decode.rs`, `encode.rs`, `query.rs`)
  - `crates/json-joy-core/src/diff_runtime/mod.rs` (+ `common.rs`, `scalar.rs`, `object.rs`, `string.rs`, `array.rs`, `bin.rs`, `vec.rs`, `dst_keys.rs`)
  - `crates/json-joy-core/src/patch/mod.rs` (+ `types.rs`, `rewrite.rs`, `decode.rs`, `encode.rs`)
  - `crates/json-joy-core/src/model/mod.rs` (+ `error.rs`, `view.rs`, `decode.rs`, `encode.rs`)
- This layout is intentionally aligned with the upstream `json-crdt/model/api`,
  `json-crdt/model`, and `json-crdt-diff` families for easier side-by-side
  verification against `/Users/nchapman/Code/json-joy`.
