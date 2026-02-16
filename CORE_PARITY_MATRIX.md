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
| `json-crdt-patch/Patch.ts` + binary codec | `crates/json-joy-core/src/patch/mod.rs` | partial | yes | yes | partial | yes | Decode parity is broad; native `rewrite_time`/`rebase` baseline added with upstream-mapped matrix tests, while broader operation parity still expanding. |
| `json-crdt-patch/PatchBuilder.ts` + operations/clock | `crates/json-joy-core/src/patch_builder.rs` | partial | yes | yes | partial | yes | Native canonical builder added for runtime op families in current fixture corpus. |
| `json-crdt/model/Model.ts` decode/view | `crates/json-joy-core/src/model/mod.rs` | partial | yes | yes | partial | yes | Decode/view parity achieved for fixture corpus; broader model semantics pending. |
| `json-crdt/model` apply semantics + node graph | `crates/json-joy-core/src/model_runtime/mod.rs` | partial | yes | yes | partial | yes | Runtime graph now has replay-matrix invariant validation and debug-build invariant enforcement during apply; further normalization work remains. |
| `json-crdt/model/api/*` (`ModelApi`, `NodeApi`, finder/proxy/fanout/events) | `crates/json-joy-core/src/model_api/mod.rs` | partial | yes | yes | partial | yes | Native API baseline now includes `from_patches`, `apply_batch`, path `find/read/select` and pointer-path variants (escaped tokens and `-` append), root `diff` + `merge`, core mutators (`set`, `obj_put`, `arr_push`, `str_ins`), direct native CRDT op emission for object/array/string/bin mutators (`InsObj`/`InsArr`/`InsStr`/`InsBin` + `Del`) with local clock progression and fallback to diff for unsupported shapes, tolerant JSON-Patch-style helpers with tuple-style op dispatch (`op_tuple`/`op_ptr_tuple`) and length-aware remove spans, per-patch and batch fanout hooks (`on_change`/`off_change`, `on_changes`/`off_changes`) plus scoped path subscriptions (`on_change_at`) with local-vs-remote origin tagging, path-bound `NodeHandle` proxy-style mutation/read composition, proxy convenience aliases (`s`, `s_ptr`, `node_ptr`, `at_ptr`, `find_ptr`), and typed wrappers (`as_obj/as_arr/as_str/as_val/as_bin/as_vec/as_con`). Remaining runtime-core gap is extension-specific typed APIs. |
| `json-crdt/model/Model.ts` higher-level lifecycle (`fromPatches`, `applyBatch`, schema-aware `load`) | partial across `model.rs`, `model_runtime/mod.rs`, `model_api/mod.rs`, `less_db_compat.rs` | partial | partial | partial | partial | yes | `from_patches`/`apply_batch`/load-session semantics are now fixture-backed (`model_lifecycle_workflow` + `model_lifecycle_from_fixtures.rs`); schema-aware model typing remains out of current runtime-core scope. |
| `json-crdt/codec/structural/binary/*` encode | `crates/json-joy-core/src/model/mod.rs` (+ runtime graph encoder in `model_runtime/encode.rs`) | partial | partial | yes | partial | yes | Logical structural encoding now follows upstream `ClockEncoder` + json-pack CBOR semantics; apply-replay parity is 50/50 and roundtrip-decode parity is green across model-roundtrip matrix (`tests/upstream_port_model_encode_matrix.rs`). |
| `json-crdt/codec/indexed/*` and `json-crdt/codec/sidecar/*` | `crates/json-joy-core/src/codec_indexed_binary/mod.rs`, `crates/json-joy-core/src/codec_sidecar_binary/mod.rs` | native | yes | yes | yes | yes | Native indexed and sidecar binary codec parity is fixture-backed and differential-checked (`codec_*_from_fixtures.rs`, `upstream_port_codec_*_matrix.rs`, `differential_codec_seeded.rs`). |
| `json-crdt/nodes/*` behavior families | `crates/json-joy-core/src/model_runtime/mod.rs` (+ split modules) | partial | yes | partial | no | yes | Obj/arr/str/bin/vec/con/val are present; upstream-mapped matrices now include array/bin/string RGA insertion/deletion ordering plus native `find`/`findInterval` query semantics in `model_runtime/query.rs`; broader fixture/differential expansion is still pending. |
| `json-crdt/nodes/rga/*` lower-level algorithm parity (`AbstractRga`, utilities) | partially embedded in runtime ops | partial | yes | partial | partial | yes | Upstream-mapped behavioral matrix expanded with array/bin insertion and deletion ordering cases (`crates/json-joy-core/tests/upstream_port_nodes_rga_matrix.rs`) alongside existing string RGA matrix; direct primitive-level split/link parity remains pending. |
| `json-crdt-diff/JsonCrdtDiff.ts` | `crates/json-joy-core/src/diff_runtime/mod.rs` | partial | yes | yes | partial | yes | Runtime diff path is native-only; unsupported shapes return `UnsupportedShape` and are not bridged in production diff runtime. Matrix test coverage: `tests/upstream_port_diff_matrix.rs`. |
| `json-crdt-diff/JsonCrdtDiff.diffDstKeys` | `crates/json-joy-core/src/diff_runtime/mod.rs` (`diff_model_dst_keys_to_patch_bytes`) | partial | partial | yes | partial | yes | Destination-key-only diff mode is now fixture-backed (`model_diff_dst_keys` + `model_diff_dst_keys_from_fixtures.rs`); broader random/deep shape expansion is deferred to keep exact-byte parity deterministic. |
| `util/diff/{str,bin,line}` algorithm-level parity | `crates/json-joy-core/src/util_diff/{str,bin,line}.rs` | partial | yes | yes | partial | yes | Native `str` and `line` modules were ported in upstream shape and verified with seeded Node differential checks (`tests/differential_util_diff_seeded.rs`) plus upstream-mapped matrix tests and fixture scenario `util_diff_parity`; broader random differential expansion remains. |
| `json-crdt-patch/compaction.ts` | `crates/json-joy-core/src/patch_compaction.rs` | partial | yes | yes | partial | yes | Native baseline port includes `combine` and `compact` semantics; covered by upstream matrix tests, seeded differential checks, and fixture scenario `patch_compaction_parity` (now with expanded canonical op-family inputs). |
| `json-crdt-patch/codec/{compact,compact-binary,verbose}` | `crates/json-joy-core/src/patch_compact_codec.rs`, `crates/json-joy-core/src/patch_compact_binary_codec.rs`, `crates/json-joy-core/src/patch_verbose_codec.rs` | partial | yes | yes | partial | yes | Native baselines now cover `compact`, `compact-binary`, and `verbose` encode/decode, and compact-binary uses json-pack CBOR string-header rules (`0x78/0x79/0x7a`). Coverage is upstream-mapped, differential-seeded, and fixture-backed via `patch_alt_codecs` over expanded canonical op families (`new_vec/new_bin/ins_vec/ins_bin/upd_arr/del/new_con_ref`). |
| `json-crdt-patch/schema.ts` | `crates/json-joy-core/src/schema.rs` | partial | yes | yes | partial | yes | Native schema-node builder baseline added (`json`/`json_con` + node families) with upstream-mapped tests, seeded differential patch-byte checks, and fixture scenario `patch_schema_parity`; printable/type-level APIs remain out of scope. |
| less-db compatibility lifecycle apply | `crates/json-joy-core/src/less_db_compat.rs` | partial | yes | yes | partial | yes | `create_model`, `diff_model`, and `apply_patch` are native in production. less-db diff fixture surface is fully native-covered in the expanded `>=50` fixture corpus (`tests/lessdb_model_manager_from_fixtures.rs`). |

## M6 exit targets

1. Every row above is `native` or `partial` without runtime bridge usage.
2. `json-crdt-diff` and compatibility apply lifecycle are `native` + `no bridge = yes`.
3. Differential parity suites exist for model apply, diff generation, and model encode/decode roundtrip.
4. Upstream-mapped test modules exist for each in-scope family.
