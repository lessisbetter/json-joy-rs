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
   - `GATES=1` (run full core gates after fast loop)
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

## Batch order

1. `util`, `buffers`, `base64` (foundation dependencies)
2. `json-pointer`, `json-path`, `json-type`, `json-random`, `json-expression`
3. `json-pack`
4. `json-joy` (all module families)
5. `codegen` and remaining support files

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
