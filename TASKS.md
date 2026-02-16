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
- [ ] Normalize runtime node graph semantics (reduce fallback-view shortcuts).
- [x] Replace oracle subprocess in `less_db_compat::apply_patch` with native runtime apply path.
- [x] Replace oracle subprocess in `diff_runtime` with native Rust diff dispatcher.
- [ ] Expand structural model encoder to support model-state generation from runtime graph.
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
