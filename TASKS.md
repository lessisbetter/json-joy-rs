# TASKS

## M1: Patch Codec (Fixture-First)

- [x] Expand oracle fixture surface to >= 50 patch-focused fixtures.
- [x] Enforce fixture coverage floor and required scenarios in Rust tests.
- [x] Add fixture-driven patch decode/roundtrip tests (`patch_diff_apply`).
- [x] Add fixture-driven malformed input acceptance/rejection tests (`patch_decode_error`).
- [x] Implement initial Rust `Patch` binary decoder/validator and `to_binary` roundtrip.
- [x] Add explicit code comments documenting fixture-driven compatibility choices.
- [x] Deepen opcode-level semantic assertions (opcodes/op count/span parity from fixtures).
- [x] Add encode tests from canonical operation models (not only roundtrip opaque bytes).

## M2: Model Binary + View

- [x] Generate model-focused fixtures (decode/view/roundtrip).
- [x] Add failing model compatibility tests.
- [x] Implement model binary decode parity baseline (accept/reject + roundtrip).
- [x] Implement model `view()` materialization parity from binary.
- [x] Expand model roundtrip corpus to >= 60 (including vec/bin and tombstones).
- [x] Expand model decode-error corpus to >= 20 malformed classes.
- [x] Add `model_canonical_encode` fixture scenario and parity tests.
- [x] Refactor decoder phases and centralize malformed compatibility classes.

## M3: Patch Application + Clock Semantics

- [x] Generate fork/replay/idempotence fixtures.
- [x] Add failing merge/idempotence tests.
- [x] Implement patch application + clock/vector semantics.

## M4: Diff Parity

- [x] Expand differential diff fixtures/traces.
- [x] Add failing diff parity tests.
- [x] Implement diff behavior to parity.

## M5: less-db-js Compatibility Layer

- [x] Add `lessdb_model_manager` fixtures (>= 30).
- [x] Add fixture-driven less-db compatibility tests.
- [x] Implement core compatibility module mirroring `model-manager` behavior.
- [x] Reuse patch-log framing with integration tests.
- [x] Expose minimal compatibility API via FFI.
- [x] Document temporary oracle-bridge dependency.

## M6: Core-Complete Native Port (Runtime Core Only)

- [x] Add `CORE_PARITY_MATRIX.md` with runtime-core family status and gate tracking.
- [x] Add native `PatchBuilder` + production patch encode path in `json-joy-core`.
- [x] Normalize runtime node graph semantics (reduce fallback-view shortcuts).
- [x] Replace oracle subprocess in `less_db_compat::apply_patch` with native runtime apply path.
- [x] Replace oracle subprocess in `diff_runtime` with native Rust diff dispatcher.
- [x] Expand structural model encoder to support model-state generation from runtime graph.
- [x] Add upstream-mapped runtime-core test-port suites (`tests/upstream_port/*`).
- [x] Add seeded differential parity suites for apply/diff/model roundtrip.
- [x] Add property/state-machine convergence tests (idempotence/order/tombstones/clocks).
- [x] Add less-db diff native-support inventory test to track fallback reduction over time.
- [x] Ensure oracle scripts are test tooling only (no production runtime dependency).
- [x] Update docs (`PORT_PLAN.md`, `AGENTS.md`, `ARCHITECTURE.md`) for bridge-retired runtime.

### M6 coverage notes (current)

- Added broad upstream-mapped matrix suites:
  - `crates/json-joy-core/tests/upstream_port_model_apply_matrix.rs`
  - `crates/json-joy-core/tests/upstream_port_diff_matrix.rs`
  - `crates/json-joy-core/tests/upstream_port_patch_builder_matrix.rs`
- Added runtime graph invariant matrix:
  - `crates/json-joy-core/tests/upstream_port_model_graph_invariants.rs`
- Expanded differential seeds from single-seed to five deterministic seeds:
  - `crates/json-joy-core/tests/differential_runtime_seeded.rs`
- Structural model-encode parity inventory added:
  - `crates/json-joy-core/tests/upstream_port_model_encode_matrix.rs`
  - Current baseline: `50/50` replay fixtures exact-binary match.
- Fixture floors hardened:
  - `model_diff_parity >= 100`
  - `model_apply_replay >= 50`
  - `lessdb_model_manager >= 50`
  - `model_canonical_encode >= 12`

### Matrix expansion follow-ups from upstream sweep (`/Users/nchapman/Drive/Code/json-joy/packages/json-joy/src`)

- [~] Add explicit matrix/test-port coverage for `json-crdt/model/api/*` (`ModelApi`, `NodeApi`, finder/proxy/events).
  Baseline added in `crates/json-joy-core/src/model_api.rs` + `crates/json-joy-core/tests/upstream_port_model_api_matrix.rs` and `crates/json-joy-core/tests/upstream_port_model_api_events_matrix.rs` (`from_patches`, `apply_batch`, `find/read/select`, root `diff`, `merge`, core mutators, tolerant `add/replace/remove/op`, path-bound `NodeHandle` proxy-style mutations, typed wrappers `as_obj/as_arr/as_str/as_val/as_bin/as_vec/as_con`, `on_change`/`off_change`, local-vs-remote change origin). Remaining gap is generated proxy tree semantics and extension-specific API typing.
- [~] Add explicit matrix/test-port coverage for `Model.ts` lifecycle helpers (`fromPatches`, `applyBatch`, schema-aware `load`).
  Lifecycle baseline now fixture-backed via `model_lifecycle_workflow` + `crates/json-joy-core/tests/model_lifecycle_from_fixtures.rs` and native `NativeModelApi::{from_patches,apply_batch,from_model_binary(load sid)}`.
  Schema-aware typing behavior remains deferred.
- [ ] Add tracking row and decision for deferred codecs: `json-crdt/codec/indexed/*` and `json-crdt/codec/sidecar/*`.
- [~] Add dedicated tracking/tests for `json-crdt-diff` destination-key mode (`diffDstKeys` parity or explicit defer).
  Added native entrypoint `diff_model_dst_keys_to_patch_bytes` + fixture scenario `model_diff_dst_keys` (20 deterministic cases) and parity test `crates/json-joy-core/tests/model_diff_dst_keys_from_fixtures.rs`.
- [~] Add dedicated tracking/tests for low-level `util/diff/{str,bin,line}` parity (beyond fixture black-box coverage).
  Native baseline added for `str` + `bin` + `line` in
  `crates/json-joy-core/src/util_diff/{str,bin,line}.rs`
  with upstream-mapped tests in
  `crates/json-joy-core/tests/upstream_port_util_diff_str_bin_matrix.rs` and
  `crates/json-joy-core/tests/upstream_port_util_diff_line_matrix.rs`.
- [x] Port and track `json-crdt-patch/compaction.ts` baseline (`combine` + `compact`) with upstream-mapped tests.
- [~] Port/track patch alternate codecs (`compact`, `compact-binary`, `verbose`).
  Native baseline now includes `codec/compact` encode/decode in
  `crates/json-joy-core/src/patch_compact_codec.rs` with upstream-mapped tests in
  `crates/json-joy-core/tests/upstream_port_patch_compact_codec_matrix.rs`.
  Native baseline now also includes `codec/verbose` encode/decode in
  `crates/json-joy-core/src/patch_verbose_codec.rs` with upstream-mapped tests in
  `crates/json-joy-core/tests/upstream_port_patch_verbose_codec_matrix.rs`.
  Native baseline now also includes `codec/compact-binary` encode/decode in
  `crates/json-joy-core/src/patch_compact_binary_codec.rs` with upstream-mapped tests in
  `crates/json-joy-core/tests/upstream_port_patch_compact_binary_codec_matrix.rs`.
- [~] Port and track `json-crdt-patch/schema.ts`.
  Native baseline added in `crates/json-joy-core/src/schema.rs` with upstream-mapped tests in
  `crates/json-joy-core/tests/upstream_port_patch_schema_matrix.rs`.
- [x] Add upstream-mapped `Patch.ts` timeline transform baseline (`rewrite_time`/`rebase`) in
  `crates/json-joy-core/src/patch.rs` with matrix tests in
  `crates/json-joy-core/tests/upstream_port_patch_rebase_matrix.rs`.
