# json-joy-rs Port Plan (Test-First, Compatibility-First)

Last updated: 2026-02-15  
Project: `/Users/nchapman/Drive/Code/json-joy-rs`

## 1. Goal

Deliver a Rust implementation that is functionally and wire-format compatible with upstream `json-joy`, then expose it via Python bindings.

Compatibility here means:
- Equivalent CRDT behavior and convergence properties.
- Equivalent binary interoperability for model and patch codecs.
- Equivalent public behavior for the subset used by `less-db-js`.

## 2. Compatibility Target (Pinned)

Use upstream **latest stable** package version:
- `json-joy@17.67.0` (`npm dist-tag: latest`)

Verification command used:
- `npm view json-joy version dist-tags --json`

Local upstream source reference:
- `/Users/nchapman/Code/json-joy`

Version anchors:
- `/Users/nchapman/Code/json-joy/package.json`
- `/Users/nchapman/Code/json-joy/packages/json-joy/package.json`

## 3. Upstream Source of Truth (Specific References)

Core model behavior:
- `/Users/nchapman/Code/json-joy/packages/json-joy/src/json-crdt/model/Model.ts`
- `/Users/nchapman/Code/json-joy/packages/json-joy/src/json-crdt/model/api/nodes.ts`

Diff behavior:
- `/Users/nchapman/Code/json-joy/packages/json-joy/src/json-crdt-diff/JsonCrdtDiff.ts`
- `/Users/nchapman/Code/json-joy/packages/json-joy/src/json-crdt-diff/index.ts`

Patch behavior:
- `/Users/nchapman/Code/json-joy/packages/json-joy/src/json-crdt-patch/Patch.ts`
- `/Users/nchapman/Code/json-joy/packages/json-joy/src/json-crdt-patch/clock/clock.ts`

Binary codecs:
- `/Users/nchapman/Code/json-joy/packages/json-joy/src/json-crdt/codec/structural/binary/Encoder.ts`
- `/Users/nchapman/Code/json-joy/packages/json-joy/src/json-crdt/codec/structural/binary/Decoder.ts`
- `/Users/nchapman/Code/json-joy/packages/json-joy/src/json-crdt-patch/codec/binary/Encoder.ts`
- `/Users/nchapman/Code/json-joy/packages/json-joy/src/json-crdt-patch/codec/binary/Decoder.ts`

Upstream tests to mirror as contract inspiration:
- `/Users/nchapman/Code/json-joy/packages/json-joy/src/json-crdt/model/__tests__/Model.binary.spec.ts`
- `/Users/nchapman/Code/json-joy/packages/json-joy/src/json-crdt-diff/__tests__/JsonCrdtDiff.spec.ts`
- `/Users/nchapman/Code/json-joy/packages/json-joy/src/json-crdt-patch/__tests__/Patch.spec.ts`

## 4. Consumer-Driven Scope (less-db-js)

Primary downstream compatibility consumer:
- `/Users/nchapman/Code/lessisbetter/less-platform/less-db-js`

High-value usage references:
- `/Users/nchapman/Code/lessisbetter/less-platform/less-db-js/src/crdt/model-manager.ts`
- `/Users/nchapman/Code/lessisbetter/less-platform/less-db-js/src/crdt/patch-log.ts`
- `/Users/nchapman/Code/lessisbetter/less-platform/less-db-js/src/storage/record-manager.ts`
- `/Users/nchapman/Code/lessisbetter/less-platform/less-db-js/tests/scenarios/conflict.test.ts`
- `/Users/nchapman/Code/lessisbetter/less-platform/less-db-js/tests/storage/correctness.test.ts`

## 5. Quality Bar

“Excellent and compatible” means all of the following:

1. Rust implementation passes its own unit/integration/property/fuzz tests.
2. Differential tests against upstream Node oracle show zero mismatches in accepted scope.
3. Golden fixtures round-trip with exact expected outputs (including binary payloads where required).
4. Error and defensive-limit behavior matches contract (oversize/corrupt inputs).
5. Python binding smoke tests prove usable API and cross-language correctness.

## 6. Test Strategy (Build This First)

## 6.1 Test layers

1. **Golden fixture tests (deterministic)**
   - Fixed fixture corpus generated from upstream Node oracle.
   - Includes model binaries, patch binaries, expected views, expected errors.

2. **Differential oracle tests (deterministic + randomized traces)**
   - For the same operation sequence, compare Rust vs upstream Node outputs.
   - Compare:
     - `view()`
     - patch presence (`None` vs `Patch`)
     - patch/model binary bytes when available and meaningful

3. **Property tests (stateful)**
   - Convergence under peer forks and replay.
   - Idempotence of replaying the same patch set.
   - Serialization round-trip invariants.

4. **Fuzz tests (defensive correctness)**
   - Binary decoders (`from_binary`) against malformed input.
   - Patch application and diff entry points.

5. **Downstream scenario replay**
   - Port key less-db-js merge/update scenarios as Rust integration tests.

## 6.2 Planned test tree

Create this structure in `json-joy-rs`:

- `/Users/nchapman/Drive/Code/json-joy-rs/tests/compat/fixtures/`
- `/Users/nchapman/Drive/Code/json-joy-rs/tests/compat/golden.rs`
- `/Users/nchapman/Drive/Code/json-joy-rs/tests/compat/differential.rs`
- `/Users/nchapman/Drive/Code/json-joy-rs/tests/scenarios/merge.rs`
- `/Users/nchapman/Drive/Code/json-joy-rs/tests/scenarios/serialization.rs`
- `/Users/nchapman/Drive/Code/json-joy-rs/tests/scenarios/session_clock.rs`
- `/Users/nchapman/Drive/Code/json-joy-rs/fuzz/` (cargo-fuzz targets)

Oracle generation tooling:

- `/Users/nchapman/Drive/Code/json-joy-rs/tools/oracle-node/`
  - Node scripts pinned to `json-joy@17.67.0` and used only to generate/verify fixtures.

## 6.3 Fixture schema (v1)

Every fixture should include:

- `name`: scenario id
- `base_json`: initial JSON value
- `ops`: sequence of operations (set/diff/apply/fork/merge)
- `expected`:
  - `view_json`
  - `patch_present` (bool)
  - `patch_binary_hex` (optional)
  - `model_binary_hex` (optional)
  - `error_contains` (optional)
- `meta`:
  - `upstream_version`: `17.67.0`
  - `source_script`
  - `generated_at`

## 6.4 Must-have scenario classes

1. Root replacement and scalar updates.
2. Object key insert/update/delete (LWW map behavior).
3. String concurrent edits and deterministic convergence behavior.
4. Array insert/delete/update with concurrent operations.
5. Forked peers + replay merge/idempotence checks.
6. Binary codec round-trips for model and patch.
7. Corrupt/truncated/oversized model and patch payload handling.
8. Session ID constraints and load/fork semantics.

## 7. Milestone Plan (Implementation Against Tests)

## M0: Harness and Contracts

Deliverables:
- Compatibility contract doc and fixture schema.
- Node oracle generator.
- Empty Rust differential runner wired into CI.

Exit criteria:
- `make check` runs harness skeleton.
- At least 20 meaningful fixtures generated and consumed by tests.

## M1: Patch Codec + Patch Core

References:
- `Patch.ts`
- `json-crdt-patch/codec/binary/*`

Deliverables:
- Rust patch data model.
- Binary decode/encode parity for patch format.
- Patch round-trip and corruption tests passing.

Exit criteria:
- Golden patch fixtures pass with zero mismatch.

## M2: Model Decode + View

References:
- `Model.ts`
- `json-crdt/codec/structural/binary/*`

Deliverables:
- Model decode/encode and view materialization.
- Binary round-trip and view parity tests.

Exit criteria:
- Model binary fixtures pass.
- `model_roundtrip` fixture count >= 60.
- `model_decode_error` fixture count >= 20.
- `model_canonical_encode` fixture count >= 6 with exact byte parity tests.
- Decoder malformed-input compatibility is fixture-backed and documented.

## M3: Apply Patch + Clock Semantics

References:
- `Model.applyPatch`, `applyOperation`
- `clock/clock.ts`

Deliverables:
- Operation application engine.
- Session/vector behavior required for replay idempotence.

Exit criteria:
- `model_apply_replay` fixture count >= 30.
- `model_apply_replay_from_fixtures` tests pass:
  - `apply_replay_fixtures_match_oracle_view`
  - `duplicate_patch_replay_is_idempotent`
  - `out_of_order_replay_matches_oracle`
- Replay/idempotence behavior is fixture-backed and documented in runtime apply code.

## M4: Diff Parity

References:
- `JsonCrdtDiff.ts`
- `json-crdt-diff/index.ts`

Deliverables:
- Diff implementation for used value types.
- `None`/patch behavior parity.

Exit criteria:
- `model_diff_parity` fixture count >= 50.
- Exact patch binary parity for patch-present fixtures (`patch_binary_hex` match).
- No-op parity: Rust returns `None` exactly where oracle returns no patch.
- `model_diff_parity_from_fixtures` tests pass:
  - `model_diff_parity_fixtures_match_oracle_patch_binary`
  - `model_diff_parity_apply_matches_oracle_view`
  - `model_diff_noop_fixtures_return_none`

## M5: less-db-js Compatibility Layer

Deliverables:
- Rust API mirroring current `model-manager` + `patch-log` behavior.
- Patch-log format parity with less-db-js v1 framing.
- Minimal FFI exposure for model lifecycle + patch-log helpers.

Exit criteria:
- `lessdb_model_manager` fixture count >= 30.
- `lessdb_model_manager_from_fixtures` tests pass:
  - `lessdb_create_diff_apply_matches_oracle`
  - `lessdb_noop_diff_returns_none`
  - `lessdb_merge_with_pending_patches_is_idempotent`
  - `lessdb_fork_and_merge_scenarios_match_oracle`
  - `lessdb_load_size_limit_is_enforced`
- `lessdb_patch_log_integration` tests pass.
- Full workspace test suite remains green.

Implementation note:
- M5 compatibility layer initially used oracle-backed lifecycle operations.
- Native status update (M6): `apply_patch` is now fully native in
  `crates/json-joy-core/src/less_db_compat.rs` via `RuntimeModel`
  decode/apply/encode.
- Remaining bridge surface is in compatibility orchestration (`less_db_compat`
  `create_model` and fallback `diff_model` for unsupported native shapes).
- `diff_runtime` production path is now native-only.

## M6: Python Package Hardening

Deliverables:
- Binding API smoke tests.
- Packaging build checks for Python artifacts.

Exit criteria:
- `make bindings-python` + Python import/smoke tests pass in CI.

## M6+: Core-Complete Native Runtime Port

Deliverables:
- Runtime-core parity tracking via `CORE_PARITY_MATRIX.md`.
- Native patch construction path (`patch_builder`) used by runtime features.
- Upstream-mapped runtime suites (`upstream_port_*` tests).
- Seeded differential checks (`differential_runtime_*` tests).
- Property/state-machine replay checks (`property_replay_*` tests).

Exit criteria:
- Runtime production paths for diff/apply/model lifecycle no longer use oracle
  subprocess bridge.
- In-scope runtime-core families in `CORE_PARITY_MATRIX.md` marked `native`.
- Existing fixture parity tests plus new upstream/differential/property suites
  all pass.

## 8. CI Gates

Minimum required gates before accepting “compatible”:

1. Unit tests pass.
2. Compatibility golden tests pass.
3. Differential tests pass (fixed corpus + randomized seeded corpus).
4. Fuzz smoke corpus runs clean for decode/apply/diff targets.
5. Python binding smoke tests pass.

## 9. Non-Goals (Current Phase)

Until compatibility baseline is complete, defer:
- Additional `json-joy` extensions not used by `less-db-js`.
- Performance optimization work not backed by profiler data.
- API expansion beyond tested compatibility surface.

## 10. Immediate Next Steps

1. Create `tests/compat` fixture schema and loader.
2. Add `tools/oracle-node` fixture generator pinned to `json-joy@17.67.0`.
3. Commit first fixture set (model/patch roundtrip + merge/idempotence basics).
4. Implement M1 patch codec against golden fixtures.
