# CORE_PARITY_MATRIX

Upstream target: `json-joy@17.67.0`  
Local upstream source: `/Users/nchapman/Code/json-joy`

Status legend:
- `native`: no runtime oracle subprocess dependency.
- `partial`: implemented but behavior/model coverage is incomplete.
- `bridge`: runtime behavior currently delegated to Node oracle subprocess.
- `missing`: not implemented in Rust.

Gate legend:
- `test-port mapped`: upstream test intent is represented in Rust tests.
- `fixture coverage`: compatibility fixtures include this family.
- `differential parity`: seeded Rust-vs-Node differential checks exist.
- `no bridge`: runtime path has no oracle subprocess delegation.

## Runtime-core family status

| Upstream family | Rust target | Status | test-port mapped | fixture coverage | differential parity | no bridge | Notes |
|---|---|---|---|---|---|---|---|
| `json-crdt-patch/Patch.ts` + binary codec | `crates/json-joy-core/src/patch/mod.rs` | native | yes | yes | yes | yes | Native decode/rewrite/rebase + canonical encode are fixture-backed (`patch_canonical_encode >= 40`, `patch_decode_error >= 35`) with upstream matrix and seeded differential parity. |
| `json-crdt-patch/PatchBuilder.ts` + operations/clock | `crates/json-joy-core/src/patch_builder.rs` | native | yes | yes | yes | yes | `patch_builder` now delegates to shared native encoder in `patch/encode.rs`; canonical timeline/opcode/clock semantics are fixture/matrix/differential backed. |
| `json-crdt/model/Model.ts` decode/view | `crates/json-joy-core/src/model/mod.rs` | native | yes | yes | yes | yes | Decode/view parity is fixture-backed at expanded floors (`model_roundtrip >= 110`, `model_decode_error >= 35`) with upstream-mapped encode/decode matrix coverage. |
| `json-crdt/model` apply semantics + node graph | `crates/json-joy-core/src/model_runtime/mod.rs` | native | yes | yes | yes | yes | Runtime graph apply/replay semantics are native and validated via expanded replay fixtures (`model_apply_replay >= 140`), invariant matrices, and seeded differential runtime checks. |
| `json-crdt/model/api/*` (`ModelApi`, `NodeApi`, finder/proxy/fanout/events) | `crates/json-joy-core/src/model_api/mod.rs` | native | yes | yes | yes | yes | Path-builder/proxy/fanout/events behaviors are fixture + upstream-matrix covered. Extension-specific typed APIs are explicitly out-of-scope runtime families. |
| `json-crdt/model/Model.ts` higher-level lifecycle (`fromPatches`, `applyBatch`, schema-aware `load`) | `crates/json-joy-core/src/model/mod.rs`, `crates/json-joy-core/src/model_runtime/mod.rs`, `crates/json-joy-core/src/model_api/mod.rs`, `crates/json-joy-core/src/less_db_compat.rs` | native | yes | yes | yes | yes | `from_patches`/`apply_batch`/load-session lifecycle semantics are fixture-backed (`model_lifecycle_workflow >= 60`) and run native in production paths. |
| `json-crdt/codec/structural/binary/*` encode | `crates/json-joy-core/src/model/mod.rs` (+ runtime graph encoder in `model_runtime/encode.rs`) | native | yes | yes | yes | yes | Logical structural encoding follows upstream `ClockEncoder` + json-pack CBOR semantics with expanded canonical coverage (`model_canonical_encode >= 30`) and differential/model-matrix parity checks. |
| `json-crdt/codec/indexed/*` and `json-crdt/codec/sidecar/*` | `crates/json-joy-core/src/codec_indexed_binary/mod.rs`, `crates/json-joy-core/src/codec_sidecar_binary/mod.rs` | native | yes | yes | yes | yes | Native indexed and sidecar binary codec parity is fixture-backed and differential-checked (`codec_*_from_fixtures.rs`, `upstream_port_codec_*_matrix.rs`, `differential_codec_seeded.rs`). |
| `json-crdt/nodes/*` behavior families | `crates/json-joy-core/src/model_runtime/mod.rs` (+ split modules) | native | yes | yes | yes | yes | Obj/arr/str/bin/vec/con/val families are fixture/matrix covered, including RGA insert/delete ordering and runtime query semantics (`find`/`findInterval`). |
| `json-crdt/nodes/rga/*` lower-level algorithm parity (`AbstractRga`, utilities) | partially embedded in runtime ops | native | yes | yes | yes | yes | RGA behaviors are validated through upstream matrices and runtime differential/replay suites; primitive behavior is implemented natively in split runtime modules. |
| `json-crdt-diff/JsonCrdtDiff.ts` | `crates/json-joy-core/src/diff_runtime/mod.rs` | native | yes | yes | yes | yes | Native diff dispatcher covers runtime-core JSON families without `UnsupportedShape`; fixture floor is `model_diff_parity >= 300` with upstream + seeded differential parity. |
| `json-crdt-diff/JsonCrdtDiff.diffDstKeys` | `crates/json-joy-core/src/diff_runtime/mod.rs` (`diff_model_dst_keys_to_patch_bytes`) | native | yes | yes | yes | yes | Destination-key mode is native with fixture floor `model_diff_dst_keys >= 80` and upstream parity tests. |
| `util/diff/{str,bin,line}` algorithm-level parity | `crates/json-joy-core/src/util_diff/{str,bin,line}.rs` | native | yes | yes | yes | yes | Native util-diff families are fixture-backed (`util_diff_parity >= 80`), upstream-mapped, and seeded differential-checked (`str`, `bin`, and `line`). |
| `json-crdt-patch/compaction.ts` | `crates/json-joy-core/src/patch_compaction.rs` | native | yes | yes | yes | yes | Native `combine` + `compact` behaviors are covered by fixtures (`patch_compaction_parity >= 40`), upstream matrix tests, and seeded differential checks. |
| `json-crdt-patch/codec/{compact,compact-binary,verbose}` | `crates/json-joy-core/src/patch_compact_codec.rs`, `crates/json-joy-core/src/patch_compact_binary_codec.rs`, `crates/json-joy-core/src/patch_verbose_codec.rs` | native | yes | yes | yes | yes | All alternate patch codecs are native and parity-checked via fixtures (`patch_alt_codecs >= 40`), upstream matrix suites, and seeded differential comparisons. |
| `json-crdt-patch/schema.ts` | `crates/json-joy-core/src/schema.rs` | native | yes | yes | yes | yes | Native schema patch builder parity is fixture-backed (`patch_schema_parity >= 45`) and differential/upstream matrix tested. |
| less-db compatibility lifecycle apply | `crates/json-joy-core/src/less_db_compat.rs` | native | yes | yes | yes | yes | `create_model`, `diff_model`, and `apply_patch` are native in production; compatibility flows stay parity-clean across expanded fixtures (`lessdb_model_manager >= 90`). |

## Adjacent Utility Coverage

| Upstream family | Rust target | Status | test-port mapped | fixture coverage | differential parity | no bridge | Notes |
|---|---|---|---|---|---|---|---|
| `json-pointer` (`util.ts` parse/format/escape/unescape) | `crates/json-joy-json-pointer/src/lib.rs` (re-exported by `crates/json-joy-core/src/lib.rs`) | native | yes | no | no | yes | Shared pointer utility used by `model_api/path.rs`; covered by upstream-shaped matrix tests in `tests/upstream_port_json_pointer_matrix.rs` and model API pointer workflow matrices. |
| `json-pack/cbor/*` (core CBOR helpers used by runtime codecs) | `crates/json-joy-json-pack/src/cbor.rs` (consumed by `json-joy-core` patch/model/runtime codecs) | native | yes | no | no | yes | Consolidated CBOR conversion + json-pack-style writer semantics now shared across core; workspace tests cover roundtrips/header behavior and core integration remains parity green. |

## M6 exit targets

1. Every in-scope runtime-core row above is `native` with no runtime bridge usage.
2. `json-crdt-diff` and compatibility apply lifecycle are `native` + `no bridge = yes`.
3. Differential parity suites exist for model apply, diff generation, and model encode/decode roundtrip.
4. Upstream-mapped test modules exist for each in-scope family.
