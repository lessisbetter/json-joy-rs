# Parity Audit (json-joy@17.67.0)

Last updated: 2026-02-20

This document tracks known, explicit parity gaps between:

- Upstream source of truth: `/Users/nchapman/Code/json-joy/packages`
- Local port: `/Users/nchapman/Drive/Code/json-joy-rs/crates`

It is a review checkpoint artifact and should be updated as gaps are closed.

## Current gate status

- `make test-gates`: pass (2026-02-20)
- `make test`: pass (2026-02-20)

## Package layout and source-family parity snapshot

`src` file counts (upstream package -> local crate mapping currently used):

| Upstream package | Local crate | Upstream `src` files | Local `src` files |
| --- | --- | ---: | ---: |
| `base64` | `base64` | 26 | 13 |
| `buffers` | `buffers` | 61 | 14 |
| `codegen` | `codegen` | 11 | 2 |
| `json-expression` | `json-expression` | 29 | 23 |
| `json-joy` | `json-joy` | 1044 | 107 |
| `json-pack` | `json-joy-json-pack` | 398 | 97 |
| `json-path` | `json-joy-json-path` | 24 | 5 |
| `json-pointer` | `json-joy-json-pointer` | 31 | 34 |
| `json-random` | `json-joy-json-random` | 18 | 10 |
| `json-type` | `json-joy-json-type` | 123 | 42 |
| `util` | `util` | 71 | 23 |

Notes:

- `json-pointer` local `src` count is +3 vs upstream because Rust requires crate/module scaffolding files (`lib.rs`, `codegen/mod.rs`, `findByPointer/mod.rs`) that have no direct TS counterparts.
- Structural crate-name divergence from AGENTS target layout is still present:
  - expected: `crates/json-pack`, `crates/json-path`, `crates/json-pointer`, `crates/json-random`, `crates/json-type`
  - current: `crates/json-joy-json-pack`, `crates/json-joy-json-path`, `crates/json-joy-json-pointer`, `crates/json-joy-json-random`, `crates/json-joy-json-type`

## Explicit non-parity choices currently in tree

These are intentionally documented non-parity areas and should remain tracked until removed.

### Harness-level accepted failures (`tests/compat/xfail.toml`)

Current xfail scenarios:

- `model_canonical_encode`
- `patch_clock_codec_parity`
- `model_lifecycle_workflow`
- `model_api_workflow`
- `model_api_proxy_fanout_workflow`
- `lessdb_model_manager`
- `model_diff_parity`
- `model_diff_dst_keys`
- `model_apply_replay`
- `codec_indexed_binary_parity`
- `codec_sidecar_binary_parity`
- `patch_schema_parity`
- `model_roundtrip`
- `model_decode_error`
- `patch_alt_codecs`
- `patch_compaction_parity`
- `patch_decode_error`
- `patch_canonical_encode`
- `patch_diff_apply`

### In-code stubs and intentional behavior notes

- `crates/codegen/src/lib.rs`: package is explicitly a stub; runtime JS codegen not ported.
- `crates/json-joy-json-type/src/codegen/binary/mod.rs`: TODO for binary codegen classes.
- `crates/json-joy-json-type/src/codegen/json/mod.rs`: JSON text codegen stub/TODO.
- `crates/json-joy-json-type/src/codegen/discriminator/index.rs`: runtime discriminator evaluator is ported, but Rust does not memoize generated closures by union identity.
- `crates/json-joy-json-type/src/codegen/capacity/capacity_estimator_codegen.rs`: runtime estimator is ported, but Rust does not model JS `Value<T>` wrapper objects in estimator input and does not emit/generated JS code.
- `crates/json-joy/src/json_crdt/draft.rs`: redo methods are explicit stubs.
- `crates/json-joy/src/json_crdt/codec/structural/verbose.rs`: local stub-node fallback path has comment noting upstream would error.
- `crates/json-joy-json-pack/src/ejson/encoder.rs`: Decimal128 encoder keeps upstream "return 0" stub behavior.
- `crates/json-joy-json-pack/src/ejson/decoder.rs`: Decimal128 decoder returns zero 16-byte stub (matching upstream stub behavior).
- `crates/json-joy-json-pointer/src/findByPointer/v1.rs`..`v5.rs`: variants are mirrored for path/layout parity, but delegate to `v6` implementation.
- `crates/json-joy-json-pointer/src/codegen/find.rs` and `crates/json-joy-json-pointer/src/codegen/findRef.rs`: upstream emits specialized JS code; Rust uses closure wrappers over runtime traversal.
- `crates/sonic-forest/src/util/mod.rs`: key-based helpers (`find`, `insert`, `find_or_next_lower`) take a `key_of` closure instead of direct node-field access to fit arena-indexed Rust nodes.
- `crates/sonic-forest/src/llrb-tree/LlrbTree.rs`: `get_or_next_lower`, `for_each`, `iterator0`, and `iterator` intentionally panic with "Method not implemented." to match upstream stubs; `clear()` intentionally mirrors upstream and only clears `root`.
- `crates/sonic-forest/src/radix/radix.rs`: string-key prefix math uses Unicode scalar (`char`) boundaries to stay Rust-safe; upstream JS indexes UTF-16 code units.
- `crates/sonic-forest/src/radix/radix.rs` and `crates/sonic-forest/src/radix/binaryRadix.rs`: debug print paths intentionally emit a generic `[value]` marker instead of full JS-style runtime value stringification.
- `crates/sonic-forest/src/TreeNode.rs`: stores `v` as `Option<V>` so `Tree.delete()` can return owned values from an arena-backed structure without removing nodes from the vector.

## sonic-forest parity status

Upstream reference:

- `/Users/nchapman/Code/sonic-forest/src`

Current local status:

- upstream source files: 81
- local source files: 60

Top-level families:

- upstream: `SortedMap`, `Tree.ts`, `TreeNode.ts`, `avl`, `data-types`, `llrb-tree`, `print`, `radix`, `red-black`, `splay`, `trie`, `types.ts`, `types2.ts`, `util`, `util2.ts`
- local: `lib.rs`, `Tree.rs`, `TreeNode.rs`, `avl`, `data-types`, `llrb-tree`, `print`, `radix`, `red-black`, `splay`, `trie`, `types.rs`, `util` (split to `first/next/swap/print/mod`), `util2.rs`

Implication:

- Top-level family parity is in place across `SortedMap`, `Tree`, `TreeNode`, `red-black`, `data-types`, `avl`, `llrb-tree`, `print`, `trie`, `radix`, `splay`, `types`, `types2`, `util`, and `util2`.
- Upstream test families are covered by Rust parity matrices:
  - `upstream_port_sorted_map_matrix.rs`
  - `upstream_port_tree_matrix.rs`
  - `upstream_port_util_matrix.rs`
  - `upstream_port_avl_matrix.rs`
  - `upstream_port_llrb_tree_matrix.rs`
  - `upstream_port_radix_matrix.rs`
  - `upstream_port_radix_slice_matrix.rs`
  - `upstream_port_red_black_map_matrix.rs`
  - `upstream_port_red_black_util_matrix.rs`
- Remaining differences are mostly Rust file/module decomposition and intentional upstream-stub parity (`Method not implemented`) surfaces in `SortedMap` and `LlrbTree`.

## Recommended next review slices

1. `json-type`: close remaining codegen stub modules (`binary`, `json`).
2. `json-pack`: target remaining codec family parity and remove ejson Decimal128 stubs where feasible.
3. `json-joy`: continue family-by-family coverage expansion against upstream package layout.
4. Revisit xfail scenarios one family at a time and remove wildcard entries as cases are fixed.
