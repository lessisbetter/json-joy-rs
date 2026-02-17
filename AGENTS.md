# AGENTS.md

## Mission

Achieve exact parity with upstream `json-joy@17.67.0` by porting all upstream packages and source files into this repository with matching package/module layout.

Upstream source of truth:
- `/Users/nchapman/Code/json-joy/packages`

## Non-negotiable rules

1. Layout parity first: mirror upstream package and module layout.
2. Compatibility-first TDD: add fixtures/tests first, then implementation.
3. Whole-file porting: port complete file families in one shot.
4. No behavior deltas without explicit written approval.
5. Do not mark a file complete unless all gates pass.

## Required local layout

Mirror upstream package structure under `crates/`:
- `crates/base64`
- `crates/buffers`
- `crates/codegen`
- `crates/json-expression`
- `crates/json-joy`
- `crates/json-pack`
- `crates/json-path`
- `crates/json-pointer`
- `crates/json-random`
- `crates/json-type`
- `crates/util`

For each package, mirror `src/**` folder structure exactly.

## Fast execution loop

For each file-family slice:
1. Add/update fixture(s) and upstream-mapped test(s).
2. Run one-command loop:
   - `make port-slice PKG=<cargo_package_name> SUITE=<integration_test_name>`
   - Optional: `FILTER=<test_name_substring> FIXTURES=0 GATES=1`
3. Port whole files.
4. Re-run focused suite if needed (`make test-suite SUITE=<suite>` and optional filtered case).
5. At slice checkpoint, run all gates (`make test-gates`).

## Standard Slice SOP (for AI agents)

Use this exact procedure for every slice:

1. Pick next unchecked row(s) from `PARITY_FILE_CHECKLIST.md` for one package family.
2. Mirror missing file paths under `crates/<package>/src/**`.
3. Add/update fixtures and tests for the same family before implementation.
4. Run:
   - `make port-slice PKG=<cargo_package_name> SUITE=<integration_test_name>`
5. Port whole upstream files for that family.
6. Re-run:
   - `make port-slice PKG=<cargo_package_name> SUITE=<integration_test_name> FILTER=<optional_test_name_substring> FIXTURES=0`
7. Run completion gates:
   - `make test-gates`
   - `make test`
8. Check completed rows in `PARITY_FILE_CHECKLIST.md`.
9. Move to the next unchecked family.

Required inputs per run:
- `PKG`: local cargo package name.
- `SUITE`: integration test file name (without `.rs`).
- `FILTER` (optional): single test/case focus.

## Gates required before marking complete

- Upstream behavior mapped in tests.
- Fixture coverage exists.
- Differential parity passes.
- Perf check passes on targeted hot paths.

## Tracking documents

Only these planning docs are authoritative:
- `PORT_PLAN.md`
- `PARITY_FILE_CHECKLIST.md`

If workflow changes, update both files in the same change.
