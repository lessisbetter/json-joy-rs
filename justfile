set shell := ["bash", "-eu", "-o", "pipefail", "-c"]

# List available recipes
default:
    @just --list

# Run all checks (format, lint, gates, full test)
check: fmt lint test-gates test

# Format code
fmt:
    mise x -- cargo fmt --all

# Clippy baseline for default quality gate
lint:
    mise x -- cargo clippy --workspace --all-features --lib --bins --examples --offline -- -D clippy::invalid_regex -D clippy::todo -D clippy::dbg_macro

# Run full workspace tests
test *args:
    mise x -- cargo test --workspace {{args}}

# Run tests with verbose output
test-v *args:
    mise x -- cargo test --workspace {{args}} -- --nocapture

# Run benchmarks
bench *args:
    mise x -- cargo bench --workspace {{args}}

# Build all targets
build:
    mise x -- cargo build --workspace

# Build release
build-release:
    mise x -- cargo build --workspace --release

# Clean build artifacts
clean:
    mise x -- cargo clean

test-smoke:
    mise x -- cargo test -p json-joy --lib --offline

test-suite suite pkg='json-joy':
    mise x -- cargo test -p {{pkg}} --test {{suite}} --offline

test-suite-filter suite filter pkg='json-joy':
    mise x -- cargo test -p {{pkg}} --test {{suite}} --offline -- {{filter}}

test-crate pkg:
    mise x -- cargo test -p {{pkg}} --offline

test-gates: test-smoke
    just test-crate json-joy-ffi
    just parity-fixtures

# One-command fast port loop for a package slice.
# Required:
#   pkg=<cargo_package_name> suite=<integration_test_name>
# Optional:
#   filter=<test_name_substring>  (run only matching tests in the suite)
#   fixtures=0                    (skip fixture regeneration; default is regenerate)
#   gates=1                       (run full gates after fast loop)
port-slice pkg suite filter='' fixtures='1' gates='0':
    if [ "{{fixtures}}" != "0" ]; then just compat-fixtures; fi
    just test-smoke
    just test-crate {{pkg}}
    if [ -n "{{filter}}" ]; then just test-suite-filter {{suite}} {{filter}} {{pkg}}; else just test-suite {{suite}} {{pkg}}; fi
    if [ "{{gates}}" = "1" ]; then just test-gates; fi

bindings-python:
    bin/generate-bindings.sh python

compat-fixtures:
    bin/generate-compat-fixtures.sh

parity-fixtures:
    mise x -- cargo test -p json-joy --test compat_inventory --test compat_fixtures --offline

parity-live-core: wasm-build
    node bench/interop.cjs

parity-live: parity-live-core

parity: parity-fixtures parity-live-core

wasm-build:
    CARGO_NET_OFFLINE=true wasm-pack build crates/json-joy-wasm --target nodejs --release

wasm-bench: wasm-build
    node bindings/wasm/bench/replay-batch-matrix.cjs

wasm-bench-one: wasm-build
    node bindings/wasm/bench/replay-batch.cjs

wasm-bench-engine-one: wasm-build
    node bindings/wasm/bench/replay-engine-one.cjs

wasm-bench-realistic: wasm-build
    node bindings/wasm/bench/lessdb-realistic.cjs
