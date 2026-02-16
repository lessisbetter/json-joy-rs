# AGENTS.md

## Project workflow: compatibility-first TDD

This repository follows a strict compatibility-first, test-driven process for porting `json-joy`.

### Core rule

For each milestone/section:

1. Expand the oracle fixture surface first.
2. Write or update failing tests against those fixtures.
3. Implement only enough code to pass.
4. Stabilize and freeze that section before moving to the next.

Do not start implementation for a section until its fixture/test surface is in place.

## Oracle and compatibility source

- Upstream compatibility target is pinned to `json-joy@17.67.0` unless explicitly changed.
- Node oracle lives in `tools/oracle-node`.
- Fixtures live in `tests/compat/fixtures`.
- Local upstream source for direct behavior cross-checks:
  - `/Users/nchapman/Code/json-joy`
  - Replay/clock semantics reference:
    - `/Users/nchapman/Code/json-joy/packages/json-joy/src/json-crdt/model/Model.ts`
  - Diff semantics reference:
    - `/Users/nchapman/Code/json-joy/packages/json-joy/src/json-crdt-diff/JsonCrdtDiff.ts`
  - less-db manager semantics references:
    - `/Users/nchapman/Code/lessisbetter/less-platform/less-db-js/src/crdt/model-manager.ts`
    - `/Users/nchapman/Code/lessisbetter/less-platform/less-db-js/src/crdt/patch-log.ts`

## Temporary bridge policy (M5)

- M5 allows oracle-backed behavior for compatibility-layer lifecycle operations.
- Keep this dependency explicit in code comments and milestone docs.
- Do not treat oracle-backed behavior as long-term final architecture; replace
  incrementally with native Rust implementations in hardening milestones.

## Required execution flow per section

1. Generate/update fixtures (`make compat-fixtures` or equivalent section-specific generator).
2. Ensure tests fail for unimplemented behavior.
3. Implement section code in Rust.
4. Run full tests (`make test`) before commit.

## Scope discipline

- Work section-by-section (M1, M2, M3...).
- Keep changes narrowly scoped to the active section.
- Avoid cross-section implementation unless required by failing tests in the active section.

## Quality gates

A section is considered complete only when:

- Fixture schema/integrity tests pass.
- Section compatibility tests pass.
- No regressions in existing tests.

## Documentation discipline

When workflow changes, update this file and relevant plan docs (`PORT_PLAN.md`) in the same change.

## M6 coverage discipline

- Keep `/Users/nchapman/Drive/Code/json-joy-rs/CORE_PARITY_MATRIX.md` current as the
  single source of truth for runtime-core parity status.
- Before starting a new core-port slice, update the matrix row status and gates
  (`test-port mapped`, `fixture coverage`, `differential parity`, `no bridge`).
- Do not mark a family `native` unless production code has no oracle subprocess
  dependency for that family.

## Current bridge boundaries (keep shrinking)

- Native in production now:
  - `crates/json-joy-core/src/less_db_compat.rs` `apply_patch`
  - `crates/json-joy-core/src/diff_runtime.rs` core dispatcher (no subprocess in this module)
- Remaining compatibility-layer oracle fallback:
  - `crates/json-joy-core/src/less_db_compat.rs` `create_model`
  - `crates/json-joy-core/src/less_db_compat.rs` `diff_model` only when `DiffError::UnsupportedShape`

When replacing these, keep fixture parity exact and update `CORE_PARITY_MATRIX.md` and `TASKS.md` in the same change.
