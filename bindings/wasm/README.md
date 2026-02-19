# WASM Engine API

`json-joy-wasm` now exposes a native engine-first API for JS usage.
No compat-layer wrappers in runtime hot paths.

## Core API

Session helpers:
- `session_generate() -> u64`
- `session_is_valid(sid: u64) -> bool`

Engine lifecycle:
- `engine_create_empty(sid) -> engineId`
- `engine_create_from_model(modelBinary, sid) -> engineId`
- `engine_fork(engineId, sid) -> engineId`
- `engine_set_sid(engineId, sid) -> ()`
- `engine_free(engineId) -> bool`

Engine operations:
- `engine_diff_json(engineId, nextJsonUtf8) -> patchBinary | empty`
- `engine_diff_apply_json(engineId, nextJsonUtf8) -> patchBinary | empty`
- `engine_diff_apply_export_json(engineId, nextJsonUtf8, flags) -> envelopeBytes`
- `engine_apply_patch(engineId, patchBinary) -> ()`
- `engine_apply_patch_batch(engineId, patchBatch) -> appliedCount`
- `engine_apply_patch_log(engineId, patchLogV1) -> appliedCount`
- `engine_export_model(engineId) -> modelBinary`
- `engine_export_view_json(engineId) -> jsonUtf8`

Patch-log helpers:
- `patch_log_append(existingPatchLogV1, patchBinary) -> patchLogV1`
- `patch_log_to_batch(patchLogV1) -> patchBatch`

Stateless utility:
- `patch_batch_apply_to_model(baseModel, patchBatch, sidForEmpty) -> modelBinary`

## Binary contracts

Patch batch format:
1. `u32` little-endian patch count
2. repeated `u32` patch length + patch bytes

Patch log format (less-db style):
1. `u8` version byte (`1`)
2. repeated `u32` big-endian patch length + patch bytes

## Run

```bash
make wasm-build
make wasm-bench-engine-one
make parity-live
make wasm-bench-realistic
```

`parity-live` validates core mixed upstream/wasm compatibility:
- upstream patch -> wasm apply
- wasm patch -> upstream apply

It does not validate JS editor adapter formats (Slate/ProseMirror/Quill).

`wasm-bench-realistic` runs a less-db-like end-to-end scenario benchmark:
- local update flow (`load -> diff/apply -> export -> append patch log`)
- remote merge flow (`from remote -> apply local patch log -> export/view`)
- correctness parity checks between upstream and wasm before timing
- includes a coarse-call experiment (`diff+apply+export` in one wasm call)
