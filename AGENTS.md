# AGENTS.md

## Mission

Build and maintain a high-quality Rust implementation of `json-joy` that is:

1. Excellent Rust code (safety, clarity, testability, performance).
2. Behaviorally compatible with upstream where parity is expected.
3. Easy to keep in sync with upstream through explicit traceability.

Upstream source of truth:
- `/Users/nchapman/Code/json-joy/packages`

Pinned upstream baseline:
- `json-joy@17.67.0`

## Project stance

- Prefixed crate names are intentional in this repo.
- Layout does not need to match upstream folder names exactly as long as mapping is explicit and documented.
- Upstream compatibility remains critical for core behavior and fixture-backed workflows.

## Upstream-to-local package mapping

- `base64` -> `crates/base64` (`json-joy-base64`)
- `buffers` -> `crates/buffers` (`json-joy-buffers`)
- `codegen` -> `crates/codegen` (`json-joy-codegen`)
- `json-expression` -> `crates/json-expression` (`json-expression`)
- `json-joy` -> `crates/json-joy` (`json-joy`)
- `json-pack` -> `crates/json-joy-json-pack` (`json-joy-json-pack`)
- `json-path` -> `crates/json-joy-json-path` (`json-joy-json-path`)
- `json-pointer` -> `crates/json-joy-json-pointer` (`json-joy-json-pointer`)
- `json-random` -> `crates/json-joy-json-random` (`json-joy-json-random`)
- `json-type` -> `crates/json-joy-json-type` (`json-joy-json-type`)
- `util` -> `crates/util` (`json-joy-util`)

## Non-negotiable rules

1. Rust quality first: readable APIs, explicit error handling, deterministic tests, and no avoidable panics in library paths.
2. Upstream traceability always: for each ported module family, keep a clear note linking to upstream source file(s) or package path.
3. Compatibility-first for behavior changes: add/update fixture tests before implementation when behavior is expected to match upstream.
4. No behavior deltas without explicit written approval unless fixing a clear bug; document every approved divergence.
5. Do not mark work complete unless lint, tests, and parity gates pass.
6. Do not leave flaky tests in tree; fix determinism or adjust assertions to match stable invariants.

## Default quality workflow (run by default)

For normal development, run:

1. `just fmt`
2. `just lint`
3. `just test-gates`
4. `just test`

If a change touches parity-sensitive paths, also run:

1. `just parity-fixtures`
2. `just parity-live` (when relevant to WASM/interop changes)

If a change touches a targeted hot path, run the relevant benchmark/perf check before merge.

## Porting workflow (when bringing in upstream changes)

Use this sequence for each package/file-family slice:

1. Add/update fixture(s) and upstream-mapped tests first.
2. Run: `just port-slice <cargo_package_name> <integration_test_name>`
3. Port full file families (not partial snippets).
4. Re-run focused suite:
   - `just port-slice <cargo_package_name> <integration_test_name> <optional_test_name_substring> fixtures=0`
5. Run completion gates:
   - `just test-gates`
   - `just test`
6. Update docs/audit notes for any parity status change.

Required inputs per run:
- `pkg`: local cargo package name.
- `suite`: integration test file name (without `.rs`).
- `filter` (optional): single test/case focus.

## Documentation requirements

When behavior, parity status, or workflow changes, update docs in the same change:

- `README.md`
- `tests/compat/README.md`
- `tests/compat/PARITY_AUDIT.md`

For intentional divergences, record:
- what differs,
- why it differs,
- whether it is temporary or permanent,
- and how it is tested.
