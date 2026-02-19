# Exact Parity Port Plan (Upstream Layout Match)

Target upstream:
- `json-joy@17.67.0`
- `/Users/nchapman/Code/json-joy/packages`

Goal:
- Exact functional parity and matching package/module layout.
- Port all upstream package pieces.
- Execute quickly using whole-file batch ports and fast test loops.

## Scope

Port these upstream packages with mirrored local package layout:
- `base64`
- `buffers`
- `codegen`
- `json-expression`
- `json-joy`
- `json-pack`
- `json-path`
- `json-pointer`
- `json-random`
- `json-type`
- `util`

## Fast test workflow

Tight local loop while porting:
1. One-command slice loop:
   - `make port-slice PKG=<cargo_package_name> SUITE=<integration_test_name>`
2. Optional controls for the same command:
   - `FILTER=<test_name_substring>`
   - `FIXTURES=0` (skip fixture regeneration)
   - `GATES=1` (run full gates after fast loop)
3. Manual drill-down commands when needed:
   - `make test-smoke`
   - `make test-suite SUITE=<integration_test_name>`
   - `make test-suite-filter SUITE=<integration_test_name> FILTER=<test_name_substring>`
   - `make test-crate PKG=<cargo_package_name>`

Agent runbook:
- Follow `AGENTS.md` "Standard Slice SOP (for AI agents)" exactly per slice.
- Always drive slice selection from unchecked rows in `PARITY_FILE_CHECKLIST.md`.

Checkpoint gates before marking a slice complete:
1. `make test-gates`
2. `make test`

Parity verification commands:
1. `make parity-fixtures` (fixture corpus replay + inventory contract checks)
2. `make parity-live` (manual TS<->WASM core differential check)
3. `make parity` (runs both; not part of default `test-gates`)

Parity policy:
- Fixture parity requires byte + semantic matching per expected field.
- Known divergences are tracked explicitly in `tests/compat/xfail.toml`.
- Live differential checks remain manual-only.

Workspace note:
- Legacy `crates/json-joy-core` has been retired.
- Gate commands now target the active `crates/json-joy` + bridge crates in workspace.

## Batch order

1. `util`, `buffers`, `base64` (foundation dependencies)
2. `json-pointer`, `json-path`, `json-type`, `json-random`, `json-expression`
3. `json-pack`
4. `json-joy` (9 slices — see below)
5. `codegen` and remaining support files

## json-joy port — 9-slice sequence

All 17 upstream sub-modules (`packages/json-joy/src/`) map to a single
`crates/json-joy` crate with sibling modules.

| Slice | Sub-modules | Status |
|---|---|---|
| 0 | Crate scaffold (Cargo.toml, lib.rs) | DONE |
| 1 | json-walk, json-pretty, json-stable, json-size, json-ml | DONE |
| 2 | json-crdt-patch (41 files — foundational patch protocol) | DONE |
| 3 | util (15 files — wraps crdt-patch clock) | DONE |
| 4 | json-patch, json-patch-ot, json-ot, json-patch-diff | DONE |
| 5 | json-hash (12 files) + json-crdt (263 files — largest!) | DONE |
| 6 | json-crdt-diff (4 files) | DONE |
| 7 | json-crdt-extensions (225 files) | PARTIAL — mval, cnt, peritext core (rga, slice) DONE; JS editor adapters intentionally out of scope in Rust/WASM |
| 8 | json-crdt-peritext-ui (UndoManager trait only; React/RxJS skipped) | DONE |
| 9 | json-cli (35 files) | DONE |

### Note: JS editor adapters are out of scope (Slice 7)

The upstream `json-crdt-extensions` includes adapters for three JS editors:
- `quill-delta/` — Quill editor interop
- `prosemirror/` — ProseMirror interop
- `slate/` — Slate.js interop

**Decision**: these adapters are not a Rust/WASM compatibility target. Use upstream JS `json-joy` for editor-specific integrations.

Rationale:
- These formats (Quill Delta, ProseMirror doc, Slate tree) only exist in JavaScript editor contexts.
- Their sole consumer will always be JavaScript, calling through the WASM boundary.
- No standalone Rust application would receive or produce these JS-editor-specific document formats.
- Keeping them in the WASM crate places the adapter glue right next to the boundary it serves,
  and keeps `crates/json-joy` focused on portable CRDT primitives.

The adapter logic is intentionally left to upstream JavaScript packages so the
Rust workspace can stay focused on core CRDT and codec parity.

Internal dependency order:
```
Leaves (no internal deps)
  json-walk, json-pretty, json-stable, json-size, json-ml  ← Slice 1
        │
  json-crdt-patch  ← Slice 2 (foundational)
        │
  util  ← Slice 3
     │
  json-patch ── json-ot ── json-patch-ot  ← Slice 4
                        └── json-patch-diff ← needs json-hash (Slice 5)
  json-hash + json-crdt  ← Slice 5 (sibling modules; circular dep OK)
     │
  ├── json-crdt-diff  ← Slice 6
  ├── json-crdt-extensions  ← Slice 7
  └── json-crdt-peritext-ui  ← Slice 8
json-cli  ← Slice 9
```

## Working model

1. Work in package batches, then file-family batches.
2. Port whole files in one shot (avoid piecemeal rewrites).
3. Keep directory/module shape aligned with upstream paths.
4. Use Node oracle fixtures + differential checks.

## Definition of done

A file is done only when:
- It is ported in mirrored path layout.
- Upstream-mapped tests pass.
- Fixture and differential checks pass.
- Required perf check is green.
- Its row is checked in `PARITY_FILE_CHECKLIST.md`.

## Tracking

`PARITY_FILE_CHECKLIST.md` is the single file-level execution checklist.
