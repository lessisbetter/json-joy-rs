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
| `json-crdt-patch/Patch.ts` + binary codec | `crates/json-joy-core/src/patch.rs` | partial | yes | yes | partial | yes | Decode parity is broad; encode and richer clock/operation parity still expanding. |
| `json-crdt-patch/PatchBuilder.ts` + operations/clock | `crates/json-joy-core/src/patch_builder.rs` | partial | yes | yes | partial | yes | Native canonical builder added for runtime op families in current fixture corpus. |
| `json-crdt/model/Model.ts` decode/view | `crates/json-joy-core/src/model.rs` | partial | yes | yes | partial | yes | Decode/view parity achieved for fixture corpus; broader model semantics pending. |
| `json-crdt/model` apply semantics + node graph | `crates/json-joy-core/src/model_runtime.rs` | partial | yes | yes | partial | yes | Runtime graph now has replay-matrix invariant validation and debug-build invariant enforcement during apply; further normalization work remains. |
| `json-crdt/codec/structural/binary/*` encode | `crates/json-joy-core/src/model.rs` (+ new encoder module) | partial | partial | yes | partial | yes | Logical structural encoding now follows upstream `ClockEncoder` + json-pack CBOR semantics; apply-replay parity is 30/30 and roundtrip-decode parity is green across model-roundtrip matrix (`tests/upstream_port_model_encode_matrix.rs`). |
| `json-crdt/nodes/*` behavior families | `crates/json-joy-core/src/model_runtime.rs` (+ split modules) | partial | partial | partial | no | yes | Obj/arr/str/bin/vec/con/val are present but need upstream-mapped behavior expansion. |
| `json-crdt-diff/JsonCrdtDiff.ts` | `crates/json-joy-core/src/diff_runtime.rs` | partial | yes | yes | partial | yes | Runtime diff path is native-only; unsupported shapes return `UnsupportedShape` and are not bridged in production diff runtime. Matrix test coverage: `tests/upstream_port_diff_matrix.rs`. |
| less-db compatibility lifecycle apply | `crates/json-joy-core/src/less_db_compat.rs` | partial | yes | yes | partial | yes | `create_model`, `diff_model`, and `apply_patch` are native in production. less-db diff fixture surface is fully native-covered (25/25 inventory in `tests/lessdb_model_manager_from_fixtures.rs`). |

## M6 exit targets

1. Every row above is `native` or `partial` without runtime bridge usage.
2. `json-crdt-diff` and compatibility apply lifecycle are `native` + `no bridge = yes`.
3. Differential parity suites exist for model apply, diff generation, and model encode/decode roundtrip.
4. Upstream-mapped test modules exist for each in-scope family.
