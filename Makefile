.PHONY: check fmt test test-smoke test-suite test-suite-filter test-crate test-gates port-slice bindings-python compat-fixtures parity-fixtures parity-live parity wasm-build wasm-bench wasm-bench-one wasm-bench-engine-one wasm-interop wasm-bench-realistic

check:
	mise x -- cargo check

fmt:
	mise x -- cargo fmt --all

test:
	mise x -- cargo test --workspace

test-smoke:
	mise x -- cargo test -p json-joy --lib --offline

test-suite:
	@if [ -z "$(SUITE)" ]; then echo "Usage: make test-suite SUITE=<integration_test_name>"; exit 1; fi
	mise x -- cargo test -p $(if $(PKG),$(PKG),json-joy) --test $(SUITE) --offline

test-suite-filter:
	@if [ -z "$(SUITE)" ]; then echo "Usage: make test-suite-filter SUITE=<integration_test_name> FILTER=<test_name_substring>"; exit 1; fi
	@if [ -z "$(FILTER)" ]; then echo "Usage: make test-suite-filter SUITE=<integration_test_name> FILTER=<test_name_substring>"; exit 1; fi
	mise x -- cargo test -p $(if $(PKG),$(PKG),json-joy) --test $(SUITE) --offline -- $(FILTER)

test-crate:
	@if [ -z "$(PKG)" ]; then echo "Usage: make test-crate PKG=<cargo_package_name>"; exit 1; fi
	mise x -- cargo test -p $(PKG) --offline

test-gates:
	$(MAKE) test-smoke
	$(MAKE) test-crate PKG=json-joy-ffi

# One-command fast port loop for a package slice.
# Required:
#   PKG=<cargo_package_name> SUITE=<integration_test_name>
# Optional:
#   FILTER=<test_name_substring>   (run only matching tests in the suite)
#   FIXTURES=0                     (skip fixture regeneration; default is regenerate)
#   GATES=1                        (run full gates after fast loop)
port-slice:
	@if [ -z "$(PKG)" ]; then echo "Usage: make port-slice PKG=<cargo_package_name> SUITE=<integration_test_name> [FILTER=<test_name_substring>] [FIXTURES=0] [GATES=1]"; exit 1; fi
	@if [ -z "$(SUITE)" ]; then echo "Usage: make port-slice PKG=<cargo_package_name> SUITE=<integration_test_name> [FILTER=<test_name_substring>] [FIXTURES=0] [GATES=1]"; exit 1; fi
	@if [ "$(FIXTURES)" != "0" ]; then $(MAKE) compat-fixtures; fi
	$(MAKE) test-smoke
	$(MAKE) test-crate PKG=$(PKG)
	@if [ -n "$(FILTER)" ]; then \
		$(MAKE) test-suite-filter PKG=$(PKG) SUITE=$(SUITE) FILTER=$(FILTER); \
	else \
		$(MAKE) test-suite PKG=$(PKG) SUITE=$(SUITE); \
	fi
	@if [ "$(GATES)" = "1" ]; then $(MAKE) test-gates; fi

bindings-python:
	bin/generate-bindings.sh python

compat-fixtures:
	bin/generate-compat-fixtures.sh

parity-fixtures:
	mise x -- cargo test -p json-joy --test compat_inventory --test compat_fixtures --offline

parity-live: wasm-build
	node bench/interop.cjs

parity: parity-fixtures parity-live

wasm-build:
	CARGO_NET_OFFLINE=true wasm-pack build crates/json-joy-wasm --target nodejs --release

wasm-bench: wasm-build
	node bindings/wasm/bench/replay-batch-matrix.cjs

wasm-bench-one: wasm-build
	node bindings/wasm/bench/replay-batch.cjs

wasm-bench-engine-one: wasm-build
	node bindings/wasm/bench/replay-engine-one.cjs

wasm-interop: parity-live

wasm-bench-realistic: wasm-build
	node bindings/wasm/bench/lessdb-realistic.cjs
