# Parity Audit (json-joy@17.67.0)

Last updated: 2026-02-20

This document tracks known, explicit parity gaps between:

- Upstream source of truth: `/Users/nchapman/Code/json-joy/packages`
- Local port: `/Users/nchapman/Drive/Code/json-joy-rs/crates`

It is a review checkpoint artifact and should be updated as gaps are closed.

## Current gate status

- `just test-gates`: pass (2026-02-20)
- `just test`: pass (2026-02-20)
- `cargo test -p json-joy --test upstream_port_diff_workflows --offline`: pass (2026-02-20)
- `cargo test -p json-joy --test upstream_port_model_api_workflow --offline`: pass (2026-02-20)
- `cargo test -p json-joy --test upstream_port_model_api_proxy_fanout_workflow --offline`: pass (2026-02-20)

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
| `json-path` | `json-joy-json-path` | 24 | 8 |
| `json-pointer` | `json-joy-json-pointer` | 31 | 34 |
| `json-random` | `json-joy-json-random` | 18 | 10 |
| `json-type` | `json-joy-json-type` | 123 | 39 |
| `util` | `util` | 71 | 23 |

Notes:

- `json-pointer` local `src` count is +3 vs upstream because Rust requires crate/module scaffolding files (`lib.rs`, `codegen/mod.rs`, `findByPointer/mod.rs`) that have no direct TS counterparts.
- `json-path` now includes explicit `codegen`, `util`, and `value` modules mapped from upstream package families; key parser/evaluator semantics from upstream test families are aligned (function filters, no-paren filters, reverse/negative slices, root-object filters, recursive descent selectors, and strict rejection of malformed trailing/empty selectors), with remaining differences primarily around broader test-family coverage and Rust decomposition.
- Prefixed crate naming is intentional and documented in `AGENTS.md` package mapping.

## Explicit non-parity choices currently in tree

These are intentionally documented non-parity areas and should remain tracked until removed.

### Harness-level accepted failures (`tests/compat/xfail.toml`)

Current xfail scenarios:

- none

Notes:

- `model_api_workflow` and `model_api_proxy_fanout_workflow` wildcard xfails were removed; scenarios pass unmasked.
- `patch_diff_apply` fixture-level xfails were removed; scenario now passes unmasked.
- `model_roundtrip` xfail was removed; scenario now passes unmasked.
- `model_apply_replay` xfail was removed after aligning evaluator semantics with upstream fixture generation:
  - effective apply count now increments only on binary state change (`before !== after`).
  - `clock_observed.patch_ids` is now emitted from patch IDs.
  - root `bin` view is normalized to JS `Uint8Array` JSON shape (`{"0":...}`).
- `model_diff_parity` wildcard xfail was removed; scenario passes unmasked.
- `codec_indexed_binary_parity` wildcard xfail was removed after indexed codec parity alignment:
  - indexed timestamp IDs now encode/decode absolute `time` (upstream), not relative deltas.
  - object field encoding preserves insertion order (upstream `Map.forEach`), not sorted order.
  - CBOR string and scalar value encoding now mirrors upstream encoder behavior.
  - indexed CBOR decoder now handles float32 (`0xfa`) as well as float64 (`0xfb`).
- `codec_sidecar_binary_parity` wildcard xfail was removed after sidecar binary view/meta parity alignment:
  - sidecar object encoding now writes interleaved key/value CBOR pairs (upstream order), with decoder matching that layout.
  - sidecar view-value encoding now mirrors upstream CBOR encoder behavior for scalar values.
  - sidecar CBOR decoder now handles float32 (`0xfa`) in addition to float64 (`0xfb`).
- `model_canonical_encode` wildcard xfail was removed after porting canonical model encoder fixture logic into Rust compat harness:
  - fixture evaluator now generates canonical model binary bytes from fixture DSL for both `logical` and `server` modes.
  - evaluator decodes the generated model bytes with structural decoder and reports `view_json`/`decode_error_message` parity fields.
- `model_lifecycle_workflow` wildcard xfail was removed after porting fixture workflow execution:
  - `from_patches_apply_batch` and `load_apply_batch` now mirror upstream fixture semantics.
  - load-time SID override uses clock forking semantics to match upstream `Model.load(..., sid)` behavior.
- `lessdb_model_manager` wildcard xfail was removed after porting workflow adapters:
  - `create_diff_apply`, `fork_merge`, and `merge_idempotent` fixture workflows are now executed in Rust harness.
  - pending patch-log append/deserialize behavior mirrors upstream fixture generator wire format.
- `model_decode_error` wildcard xfail was removed after aligning compat evaluator classification with upstream fixture semantics for malformed payload classes.
- `patch_decode_error` wildcard xfail was removed after aligning compat evaluator classification with upstream fixture semantics for malformed payload classes.
- Slice A closures completed in this pass:
  - `patch_schema_parity` xfail removed after aligning schema fixture replay root wiring plus binary string/header parity.
  - `patch_canonical_encode` xfail removed after canonical patch encoder parity fixes.
  - `patch_compaction_parity` xfail removed after UTF-16 span semantics in compaction.
  - `patch_alt_codecs` xfail removed after compact codec wire-shape parity (`encode`/`decode`) was ported to upstream structure.
- No active compat xfails remain.

### In-code stubs and intentional behavior notes

- `crates/codegen/src/lib.rs`: package is explicitly a stub; runtime JS codegen not ported.
- `crates/json-joy-json-type/src/codegen/binary/mod.rs`: TODO for binary codegen classes.
- `crates/json-joy-json-type/src/codegen/json/mod.rs`: JSON text codegen stub/TODO.
- `crates/json-joy-json-type/src/codegen/discriminator/mod.rs`: discriminator codegen stub/TODO.
- `crates/json-joy-json-type/src/codegen/capacity/mod.rs`: capacity estimator codegen stub/TODO.
- `crates/json-joy/src/json_crdt/draft.rs`: redo methods are explicit stubs.
- `crates/json-joy/src/json_crdt/codec/structural/verbose.rs`: local stub-node fallback path has comment noting upstream would error.
- `crates/json-joy-json-pack/src/ejson/encoder.rs`: Decimal128 encoder keeps upstream "return 0" stub behavior.
- `crates/json-joy-json-pack/src/ejson/decoder.rs`: Decimal128 decoder returns zero 16-byte stub (matching upstream stub behavior).
- `crates/json-joy-json-pointer/src/findByPointer/v1.rs`..`v5.rs`: variants are mirrored for path/layout parity, but delegate to `v6` implementation.
- `crates/json-joy-json-pointer/src/codegen/find.rs` and `crates/json-joy-json-pointer/src/codegen/findRef.rs`: upstream emits specialized JS code; Rust uses closure wrappers over runtime traversal.
- `crates/json-joy-json-path/src/codegen.rs`: upstream generates specialized JS code; Rust uses pre-parsed AST closures over `JsonPathEval`.
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

1. `json-path`: port additional upstream `__tests__` families (especially `testJsonPathExec` and `descendant-selector`) into Rust parity matrices to widen behavioral coverage.
2. `json-type`: close codegen stub modules first (`binary`, `json`, `discriminator`, `capacity`).
3. Revisit xfail scenarios one family at a time and remove wildcard entries as cases are fixed.
