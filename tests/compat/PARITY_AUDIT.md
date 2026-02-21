# Parity Audit (json-joy@17.67.0)

Last updated: 2026-02-21

This document tracks known, explicit parity gaps between:

- Upstream source of truth: `/Users/nchapman/Code/json-joy/packages`
- Local port: `/Users/nchapman/Drive/Code/json-joy-rs/crates`

It is a review checkpoint artifact and should be updated as gaps are closed.

## Current gate status

- `just test-gates`: pass (2026-02-21)
- `just test`: pass (2026-02-21)
- `cargo test -p json-joy --test upstream_port_diff_workflows --offline`: pass (2026-02-21)
- `cargo test -p json-joy --test upstream_port_model_api_workflow --offline`: pass (2026-02-21)
- `cargo test -p json-joy --test upstream_port_model_api_proxy_fanout_workflow --offline`: pass (2026-02-21)

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
- `json-path` now includes explicit `codegen`, `util`, and `value` modules mapped from upstream package families; key parser/evaluator semantics from upstream test families are aligned (function filters including edge cases such as unknown/wrong-arity/nested calls, no-paren filters, reverse/negative slices, root-object filters, recursive descent selectors, and strict rejection of malformed trailing/empty selectors), with remaining differences primarily around broader test-family coverage and Rust decomposition.
- `json-path` now has an upstream-mapped integration matrix at `crates/json-joy-json-path/tests/upstream_port_json_path_matrix.rs` covering canonical bookstore queries from upstream `testJsonPathExec`.
- `json-path` now has an upstream-mapped integration matrix at `crates/json-joy-json-path/tests/upstream_port_json_path_descendant_matrix.rs` covering descendant-selector behavior and codegen/eval equivalence from upstream `descendant-selector.spec.ts`.
- `json-path` now has an upstream-mapped integration matrix at `crates/json-joy-json-path/tests/upstream_port_json_path_demo_matrix.rs` covering the complex TypeScript-AST query behavior from upstream `demo.spec.ts`, including path-shape assertions for matched ranges and codegen-vs-eval parity.
- `json-path` now has an upstream-mapped integration matrix at `crates/json-joy-json-path/tests/upstream_port_json_path_exec_matrix.rs` covering root-format errors, combined selectors, practical edge-case scenarios, and codegen-vs-eval parity on complex function-filter and recursive/union expressions from upstream `testJsonPathExec`.
- `json-path` now has an upstream-mapped integration matrix at `crates/json-joy-json-path/tests/upstream_port_json_path_functions_matrix.rs` covering function extension scenarios (`length`, `count`, `match`, `search`, `value`) and combined logical usage from upstream `testJsonPathExec`, including Unicode length checks, descendant counting, regex anchor/case behavior, wrong-arity and unknown-function filters, and null-value function edge cases.
- `json-path` now has an upstream-mapped integration matrix at `crates/json-joy-json-path/tests/upstream_port_json_path_parser_matrix.rs` covering parser-shape scenarios for unions, recursive+filter composition, bracket-notation existence filters, and parser error handling from upstream `JsonPathParser.spec.ts`.
- `json-path` now has an upstream-mapped integration matrix at `crates/json-joy-json-path/tests/upstream_port_json_path_util_matrix.rs` covering utility helper behavior (`json_path_to_string`, `json_path_equals`, `get_accessed_properties`) from upstream `util.spec.ts`, including upstream union-string formatting shape.
- `json-path` parser/eval parity matrices now additionally cover upstream edge syntax and selector breadth: union variants with wildcard/negative indices/whitespace, escaped and empty string keys, whitespace-tolerant path parsing, nested logical+paren filter structures, descendant index selection across nested arrays, and explicit `$..*` descendant result-shape expectations from upstream `testJsonPathExec` and `descendant-selector.spec.ts`.
- `json-path` now has an expression inventory matrix at `crates/json-joy-json-path/tests/upstream_port_json_path_expression_inventory.rs` to enforce a broad set of known-valid and known-invalid parser cases derived from upstream test suites.
- `json-pack` now has an upstream-mapped integration matrix at `crates/json-joy-json-pack/tests/upstream_port_ws_matrix.rs` covering WebSocket frame encoder/decoder behavior from upstream `ws/__tests__/encoder.spec.ts` and `ws/__tests__/decoder.spec.ts`, including control/data framing, mask/unmask flows, fragmentation, size-encoding boundaries, and incomplete ping/pong payload handling.
- `json-pack` now has an upstream-mapped integration matrix at `crates/json-joy-json-pack/tests/upstream_port_resp_matrix.rs` covering RESP encoder/decoder behavior from upstream `resp/__tests__/RespEncoder.spec.ts`, `RespDecoder.spec.ts`, `RespStreamingDecoder.spec.ts`, `skipping.spec.ts`, and `RespEncoderLegacy.spec.ts`, including command parsing, stream chunk reassembly, skip semantics, legacy RESP2 encoding paths, and extension frame coverage.
- `json-pack` now has an upstream-mapped integration matrix at `crates/json-joy-json-pack/tests/upstream_port_rm_matrix.rs` covering RM encoder/decoder behavior from upstream `rm/__tests__/RmRecordEncoder.spec.ts` and `rm/__tests__/RmRecordDecoder.spec.ts`, including header bit/length encoding, fragment writing, streamed byte-by-byte decode, multi-fragment record assembly, and empty/large record edge cases.
- `json-pack` now has an upstream-mapped integration matrix at `crates/json-joy-json-pack/tests/upstream_port_rpc_matrix.rs` covering ONC RPC encoder/decoder behavior from upstream `rpc/__tests__/encoder.spec.ts`, `rpc/__tests__/decoder.spec.ts`, and `rpc/__tests__/fixtures.spec.ts`, including real-world fixture decode parity, roundtrip structure preservation, auth padding, partial-stream handling, and invalid header/auth-size error cases.
- `json-pack` now has an upstream-mapped integration matrix at `crates/json-joy-json-pack/tests/upstream_port_rpc_real_traces_matrix.rs` covering RM-framed real NFS RPC traces from upstream `rpc/__tests__/real-traces.spec.ts` (LOOKUP/ACCESS calls and READDIRPLUS reply decode assertions).
- `json-pack` now has an upstream-mapped integration matrix at `crates/json-joy-json-pack/tests/upstream_port_xdr_matrix.rs` covering XDR primitive codecs plus schema encoder/decoder behavior from upstream `xdr` suites, including array helpers, enum/struct/union roundtrips, size/UTF-8 decoder error paths, and explicit unsupported-surface parity (`quadruple`, string union discriminants).
- `json-pack` now has an upstream-mapped integration matrix at `crates/json-joy-json-pack/tests/upstream_port_xdr_schema_validator_matrix.rs` covering XDR schema/value validator behavior from upstream `xdr/__tests__/XdrSchemaValidator.spec.ts`, including enum/struct/union schema checks, duplicate discriminant detection, and value-validation matrices for arrays/structs/unions/optionals.
- `json-pack` now has an upstream-mapped integration matrix at `crates/json-joy-json-pack/tests/upstream_port_avro_schema_validator_matrix.rs` covering Avro schema/value validator behavior from upstream `avro/__tests__/AvroSchemaValidator.spec.ts`, including primitive and composite schema checks, union uniqueness, enum/default validation, recursive named-reference schema checks, and value-validation matrices for records/enums/arrays/maps/unions/fixed values.
- `json-pack` now has an upstream-mapped integration matrix at `crates/json-joy-json-pack/tests/upstream_port_avro_matrix.rs` covering Avro encoder/decoder/schema codec behavior from upstream `avro` suites, including unsigned length/count varint wire-shape parity, recursive named-schema roundtrips, union branch selection semantics, enum index error paths, varint overflow handling, and invalid map key/schema rejection.
- `json-pack` now has an upstream-mapped integration matrix at `crates/json-joy-json-pack/tests/upstream_port_cbor_matrix.rs` covering CBOR encoder/decoder behavior from upstream `cbor` suites, including numeric/binary/string/array/object roundtrips, indefinite/streaming bin-str-arr-map decoding, strict validation error paths (size mismatch, malformed indefinite map, invalid payload), stable key-order encoding parity, and DAG-specific float/tag behavior.
- `json-pack` now has an upstream-mapped integration matrix at `crates/json-joy-json-pack/tests/upstream_port_ejson_matrix.rs` covering EJSON encoder/decoder/integration behavior from upstream `ejson/__tests__/EjsonEncoder.spec.ts`, `EjsonDecoder.spec.ts`, and `integration.spec.ts`, including canonical/relaxed wrapper shape checks, wrapper decode and strict error-path checks, and roundtrip parity for nested values and BSON wrappers.
- `json-pack` now has an upstream-mapped integration matrix at `crates/json-joy-json-pack/tests/upstream_port_msgpack_matrix.rs` covering MsgPack encoder/decoder behavior from upstream `msgpack` suites, including wire-shape boundaries, one-level container decoding via pre-encoded blobs, shallow path reads (`findKey`/`findIndex`/path traversal), strict value-size validation, extension/precomputed-value handling, stable key-order encoding, and direct MsgPack-to-JSON conversion parity.
- `json-pack` now has an upstream-mapped integration matrix at `crates/json-joy-json-pack/tests/upstream_port_bencode_matrix.rs` covering Bencode encoder/decoder behavior from upstream `bencode/__tests__/BencodeEncoder.spec.ts`, `BencodeDecoder.spec.ts`, and `automated.spec.ts`, including primitive wire shapes, dictionary key sorting, fixture-style roundtrips, and strict truncated-input/invalid-key decode errors.
- `json-pack` now has an upstream-mapped integration matrix at `crates/json-joy-json-pack/tests/upstream_port_ubjson_matrix.rs` covering UBJSON encoder/decoder behavior from upstream `ubjson/__tests__/UbjsonEncoder.spec.ts`, `UbjsonDecoder.spec.ts`, and `automated.spec.ts`, including integer width boundaries, typed binary arrays/extensions, streaming array/object framing, and invalid-key/incomplete-input decode errors.
- `json-pack` now has an upstream-mapped integration matrix at `crates/json-joy-json-pack/tests/upstream_port_ssh_matrix.rs` covering SSH codec behavior from upstream `ssh/__tests__/SshEncoder.spec.ts`, `SshDecoder.spec.ts`, and `codec.spec.ts`, including RFC4251 primitive encodings, UTF-8/ASCII/binary string handling, mpint/name-list roundtrips, packet-like mixed decode flows, and EOF/UTF-8 error paths.
- `json-pack` now has an upstream-mapped integration matrix at `crates/json-joy-json-pack/tests/upstream_port_json_binary_matrix.rs` covering json-binary behavior from upstream `json-binary/__tests__/stringify.spec.ts` and `automated.spec.ts`, including exact data-URI stringify shape, parse/stringify roundtrips for binary/msgpack/blob wrappers, extension URI decode, and invalid-base64 fallback behavior.
- `json-pack` now has an upstream-mapped integration matrix at `crates/json-joy-json-pack/tests/upstream_port_bson_matrix.rs` covering BSON encoder/decoder behavior from upstream `bson` suites, including primitive/special-value roundtrips (ObjectId, DBPointer, JS code/scope, Decimal128, Min/Max key, typed binary subtypes), document wire-shape invariants, and strict decode error paths for unsupported types and invalid UTF-8.
- `json-pack` now has an upstream-mapped integration matrix at `crates/json-joy-json-pack/tests/upstream_port_ion_matrix.rs` covering Ion binary encoder/decoder behavior from upstream `ion` suites, including scalar/container roundtrips, symbol-table annotation flows for object fields, and strict error paths (invalid BVM, negative-zero nint, unknown symbol IDs).
- `json-pack` now has an upstream-mapped integration matrix at `crates/json-joy-json-pack/tests/upstream_port_json_matrix.rs` covering JSON encoder/decoder behavior from upstream `json` suites, including undefined sentinel and binary data-URI handling, stable encoder compatibility, DAG JSON bytes/CID wrappers (including nested decode), and fault-tolerant partial-decoder scenarios.
- `json-pack` now has an upstream-mapped integration matrix at `crates/json-joy-json-pack/tests/upstream_port_json_pack_util_matrix.rs` covering utility behavior from upstream `util` suites, including compression table literal collection and integer run-length encoding, object-key index compression semantics, decompression table import/rebuild flow, and `toDataUri` buffer URI formatting with optional metadata params.
- `json-type` codegen families (`capacity`, `json`, `discriminator`, `binary`) are now implemented as runtime Rust codegen adapters with upstream-mapped parity coverage at `crates/json-joy-json-type/tests/upstream_port_json_type_codegen_matrix.rs` (including tuple head/tail sizing, unknown-key object encoding through refs, optional-only object unknown-key mode permutations, nested-object unknown-key handling, recursive ref/alias chains, custom `or` discriminator-expression handling, native binary preservation for MsgPack/CBOR `bin` fields, and binary codec roundtrips for recursive ref structures).
- `json-crdt` log codec now mirrors upstream component encoding flow:
  - `LogEncoder.serialize/encode` supports `ndjson` and `seq.cbor`.
  - model encodings: `sidecar`, `binary`, `compact`, `verbose`, `none`.
  - history encodings: `binary`, `compact`, `verbose`, `none`.
  - `LogDecoder.decode/deserialize` supports `view`/`history`/`frontier` decode paths, sidecar `sidecar_view` injection, and frontier patch application.
  - upstream-mapped regression coverage includes first-component view readability, sidecar-without-view decode, metadata roundtrip, frontier application, and full format-combination matrix.
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
- `crates/json-joy/src/json_crdt/draft.rs`: redo methods are explicit stubs.
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

1. `json-path`: continue porting additional upstream parser/evaluator corner cases (especially high-complexity nested filter/function combinations) into matrix tests to widen behavioral coverage.
2. Revisit xfail scenarios one family at a time and remove wildcard entries as cases are fixed.
