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
