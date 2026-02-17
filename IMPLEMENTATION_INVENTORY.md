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
| `diff(src, dst)` | `crates/json-joy-core/src/diff_runtime/mod.rs` (`diff_model_to_patch_bytes`, `diff_runtime_to_patch_bytes`) | exact | yes | yes | yes | yes | Direct runtime recursive path is used for both logical and server models; server-clock ids are based on `server_clock_time`. |
| `diffDstKeys(src, dst)` | `crates/json-joy-core/src/diff_runtime/dst_keys.rs` | exact | yes | yes | yes | no | Upstream key-order/update-only behavior covered in `tests/upstream_port_diff_dst_keys_matrix.rs`; perf not separately tracked yet. |
| `diffAny` dispatcher | `crates/json-joy-core/src/diff_runtime/common.rs` | exact | yes | yes | yes | yes | Main family dispatch is native; mismatch-to-replace semantics covered in `tests/upstream_port_diff_any_matrix.rs`. |
| `diffObj` | `crates/json-joy-core/src/diff_runtime/common.rs` (`try_emit_object_recursive_diff`) | exact | yes | yes | yes | yes | Two-pass delete/insert tuple ordering and child-recursion preference covered in `tests/upstream_port_diff_obj_matrix.rs`. |
| `diffArr` | `crates/json-joy-core/src/diff_runtime/common.rs` | exact | yes | yes | yes | yes | Line-diff + structural-hash array deltas are byte-parity covered by fixture and recursive differential suites. |
| `diffVec` | `crates/json-joy-core/src/diff_runtime/vec.rs` + `common.rs` | exact | yes | yes | yes | yes | ConNode primitive replacement semantics aligned with upstream and covered in `tests/upstream_port_diff_vec_matrix.rs`; realistic perf rerun recorded (~94.6% best wasm/upstream). |
| `diffStr` | `crates/json-joy-core/src/diff_runtime/common.rs` + `string.rs` | exact | yes | yes | yes | no | Insert/delete op shape and reference behavior are covered in `tests/upstream_port_diff_smoke.rs` (single/multi-root, nested, multi-leaf) plus util-diff matrix tests. |
| `diffBin` | `crates/json-joy-core/src/diff_runtime/common.rs` + `bin.rs` | exact | yes | yes | yes | no | Mixed replace windows and multi-root/nested bin op-shape/reference behavior are covered in `tests/upstream_port_diff_smoke.rs` plus util-diff matrix tests. |
| `diffVal` | `crates/json-joy-core/src/diff_runtime/scalar.rs` + `mod.rs` | exact | yes | yes | yes | no | Scalar replacement/timestamp wiring is covered via upstream byte-parity matrix + seeded scalar differential tests. |
| `buildView` | `crates/json-joy-core/src/diff_runtime/common.rs` (`NativeEmitter::emit_value`/`emit_array_item`) | exact | yes | yes | yes | yes | Emitter value construction is exercised by fixture matrix + recursive/scalar differential parity (including array scalar `val->con`). |
| `buildConView` | distributed (`scalar.rs`, `object.rs`, `common.rs`) | exact | yes | yes | yes | no | Explicit const-vs-json emission shape is covered in `tests/upstream_port_build_con_view_matrix.rs` plus diff smoke/matrix parity suites. |

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
| `hash.ts` (`hashJson/hashStr/hashBin`) | `crates/json-joy-core/src/json_hash.rs` | exact | yes | yes | yes | no | Differential + fixture parity now in `tests/differential_json_hash_seeded.rs` and `tests/json_hash_from_fixtures.rs`. |
| `structHash.ts` (`structHash`) | `crates/json-joy-core/src/json_hash.rs` (`struct_hash_json`) | exact | yes | yes | yes | yes | Differential + fixture parity in place; string/key hashing matches upstream `hash(value)` semantics. |
| `structHashCrdt.ts` | `crates/json-joy-core/src/json_hash.rs` (`struct_hash_crdt`) | exact | yes | yes | yes | yes | Differential + fixture parity in place. |
| `structHashSchema.ts` | `crates/json-joy-core/src/json_hash.rs` (`struct_hash_schema`) | exact | yes | yes | yes | yes | Differential + fixture parity in place. |

## Current hot-path queue

1. `JsonCrdtDiff.diffAny` exactness audit: document every error/throw-to-replace case and map to Rust branch.
2. `JsonCrdtDiff.diffObj` exactness audit: op ordering, delete encoding, insertion value constructor parity.
3. `JsonCrdtDiff.diffVec` exactness audit: stale/deleted slot behavior and const-vs-json replacement semantics.
4. Server-clock diff route: direct runtime path now uses `server_clock_time` base and passes `upstream_port_diff_server_clock_matrix`; keep validating via broader parity suites.
5. Consolidate remaining doc status rows (`CORE_PARITY_MATRIX.md`) to match function-level `exact` state where applicable.
