# WASM bindings

Upstream Credit: This WASM bridge is for the Rust port of upstream
`json-joy` by streamich.
- Upstream repository: <https://github.com/streamich/json-joy>
- Upstream docs: <https://jsonjoy.com/libs/json-joy-js>
- Upstream package reference: `packages/json-joy`

This directory contains benchmark and interoperability harness scripts for the
`json-joy-wasm` crate.

## Build and run

From repository root:

```bash
just wasm-build
```

Benchmarks:

```bash
just wasm-bench
just wasm-bench-engine-one
just wasm-bench-realistic
```

Core parity differential check:

```bash
just parity-live
```

`parity-live` validates core mixed upstream/wasm compatibility:
- upstream patch -> wasm apply
- wasm patch -> upstream apply

## Scope note

This harness validates core model/patch compatibility only.
Editor-specific JS adapter formats (Slate/ProseMirror/Quill) are out of scope.
