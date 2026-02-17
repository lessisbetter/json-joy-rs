# IMPLEMENTATION_INVENTORY

Upstream target: `json-joy@17.67.0`  
Local upstream source: `/Users/nchapman/Code/json-joy`

This file is the function-level port queue and evidence ledger.

Status legend:
- `exact`: behavior/shape matches upstream for covered inputs.
- `approx`: implemented but not proven exact (or known deltas exist).
- `missing`: not implemented.

Evidence gates per row:
- `upstream test mapped`
- `fixture coverage`
- `differential parity`
- `perf checked`

A row is only considered complete when all four gates are `yes`.

## Diff: JsonCrdtDiff.ts

| Upstream function | Rust target | Status | upstream test mapped | fixture coverage | differential parity | perf checked | Notes |
|---|---|---|---|---|---|---|---|
| `diff(src, dst)` | `crates/json-joy-core/src/diff_runtime/mod.rs` (`diff_model_to_patch_bytes`, `diff_runtime_to_patch_bytes`) | approx | yes | yes | yes | yes | Logical models now route through direct runtime recursive path first; server-clock still uses legacy route. |
| `diffDstKeys(src, dst)` | `crates/json-joy-core/src/diff_runtime/dst_keys.rs` | approx | yes | yes | yes | no | Behavior parity covered; perf not separately tracked yet. |
| `diffAny` dispatcher | `crates/json-joy-core/src/diff_runtime/common.rs` | approx | yes | yes | yes | yes | Main family dispatch is native; mismatch-to-replace semantics covered in `tests/upstream_port_diff_any_matrix.rs`. |
| `diffObj` | `crates/json-joy-core/src/diff_runtime/common.rs` (`try_emit_object_recursive_diff`) | approx | yes | yes | yes | yes | Two-pass delete/insert tuple ordering and child-recursion preference covered in `tests/upstream_port_diff_obj_matrix.rs`. |
| `diffArr` | `crates/json-joy-core/src/diff_runtime/common.rs` | approx | yes | yes | yes | yes | Uses line diff + structural hash path; scalar `InsVal` array element shape aligned with upstream. |
| `diffVec` | `crates/json-joy-core/src/diff_runtime/vec.rs` + `common.rs` | approx | yes | yes | yes | yes | ConNode primitive replacement semantics aligned with upstream and covered in `tests/upstream_port_diff_vec_matrix.rs`; realistic perf rerun recorded (~94.6% best wasm/upstream). |
| `diffStr` | `crates/json-joy-core/src/diff_runtime/common.rs` + `string.rs` | approx | yes | yes | yes | no | Uses util diff + RGA spans; keep exactness checks for insert reference choices. |
| `diffBin` | `crates/json-joy-core/src/diff_runtime/common.rs` + `bin.rs` | approx | yes | yes | yes | no | Uses util diff + RGA spans; keep exactness checks for mixed replace windows. |
| `diffVal` | `crates/json-joy-core/src/diff_runtime/scalar.rs` + `mod.rs` | approx | yes | yes | yes | no | Value replace path covered; continue exactness checks for timestamp/ref semantics. |
| `buildView` | `crates/json-joy-core/src/diff_runtime/common.rs` (`NativeEmitter::emit_value`/`emit_array_item`) | approx | yes | yes | yes | yes | Array scalar `val->con` behavior now aligned. |
| `buildConView` | distributed (`scalar.rs`, `object.rs`, `common.rs`) | approx | yes | yes | yes | no | Const-vs-json selection is spread across helpers; needs explicit mapping audit. |

## Patch Builder: PatchBuilder.ts

| Upstream function family | Rust target | Status | upstream test mapped | fixture coverage | differential parity | perf checked | Notes |
|---|---|---|---|---|---|---|---|
| operation constructors (`obj/arr/vec/str/bin/con/val`) | `crates/json-joy-core/src/patch_builder.rs` | exact | yes | yes | yes | no | Canonical timeline + op shape covered by matrix + fixture + differential suites. |
| edit ops (`insObj/insVec/setVal/insStr/insBin/insArr/del/nop`) | `crates/json-joy-core/src/patch_builder.rs` | exact | yes | yes | yes | no | Shared native encoder path in `patch/encode.rs`. |
| json constructors (`jsonObj/jsonArr/jsonStr/jsonBin/jsonVal/json`) | `crates/json-joy-core/src/patch_builder.rs` + diff emitter helpers | approx | yes | yes | yes | no | Some builder-like behavior exists in diff emitter; keep explicit exactness checks. |
| helpers (`constOrJson/maybeConst/pad`) | `crates/json-joy-core/src/patch_builder.rs` | exact | yes | yes | yes | no | Covered by patch-builder matrix and canonical parity tests. |

## json-hash family

| Upstream function | Rust target | Status | upstream test mapped | fixture coverage | differential parity | perf checked | Notes |
|---|---|---|---|---|---|---|---|
| `hash.ts` (`hashJson/hashStr/hashBin`) | `crates/json-joy-core/src/json_hash.rs` | approx | yes | no | yes | no | Differential parity added in `tests/differential_json_hash_seeded.rs`; fixture integration still pending. |
| `structHash.ts` (`structHash`) | `crates/json-joy-core/src/json_hash.rs` (`struct_hash_json`) | approx | yes | no | yes | yes | Differential parity added; string/key hashing now matches upstream `hash(value)` semantics. |
| `structHashCrdt.ts` | `crates/json-joy-core/src/json_hash.rs` (`struct_hash_crdt`) | approx | yes | no | yes | yes | Differential parity added in `tests/differential_json_hash_seeded.rs`. |
| `structHashSchema.ts` | `crates/json-joy-core/src/json_hash.rs` (`struct_hash_schema`) | approx | yes | no | no | yes | Needs dedicated differential/fixture proof. |

## Current hot-path queue

1. `JsonCrdtDiff.diffAny` exactness audit: document every error/throw-to-replace case and map to Rust branch.
2. `JsonCrdtDiff.diffObj` exactness audit: op ordering, delete encoding, insertion value constructor parity.
3. `JsonCrdtDiff.diffVec` exactness audit: stale/deleted slot behavior and const-vs-json replacement semantics.
4. `json-hash` schema hashing (`structHashSchema`) differential + fixture integration.
5. Server-clock diff route: remove legacy path once parity evidence is in place (current direct-route experiment fails `upstream_port_diff_server_clock_matrix`).
