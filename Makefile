.PHONY: check fmt test test-smoke test-suite test-suite-filter test-crate test-gates port-slice bindings-python compat-fixtures test-core-fixtures test-core-upstream test-core-differential test-core-property test-core test-core-fast-diff test-core-fast-hash test-core-full wasm-build wasm-bench wasm-bench-one wasm-bench-engine-one wasm-interop wasm-bench-realistic

check:
	mise x -- cargo check

fmt:
	mise x -- cargo fmt --all

test:
	mise x -- cargo test --workspace

test-smoke:
	mise x -- cargo test -p json-joy-core --test compat_fixtures --test suite_coverage_inventory --offline

test-suite:
	@if [ -z "$(SUITE)" ]; then echo "Usage: make test-suite SUITE=<integration_test_name>"; exit 1; fi
	mise x -- cargo test -p json-joy-core --test $(SUITE) --offline

test-suite-filter:
	@if [ -z "$(SUITE)" ]; then echo "Usage: make test-suite-filter SUITE=<integration_test_name> FILTER=<test_name_substring>"; exit 1; fi
	@if [ -z "$(FILTER)" ]; then echo "Usage: make test-suite-filter SUITE=<integration_test_name> FILTER=<test_name_substring>"; exit 1; fi
	mise x -- cargo test -p json-joy-core --test $(SUITE) --offline -- $(FILTER)

test-crate:
	@if [ -z "$(PKG)" ]; then echo "Usage: make test-crate PKG=<cargo_package_name>"; exit 1; fi
	mise x -- cargo test -p $(PKG) --offline

test-gates:
	$(MAKE) test-core-fixtures
	$(MAKE) test-core-upstream
	$(MAKE) test-core-differential
	$(MAKE) test-core-property

# One-command fast port loop for a package slice.
# Required:
#   PKG=<cargo_package_name> SUITE=<integration_test_name>
# Optional:
#   FILTER=<test_name_substring>   (run only matching tests in the suite)
#   FIXTURES=0                     (skip fixture regeneration; default is regenerate)
#   GATES=1                        (run full core gates after fast loop)
port-slice:
	@if [ -z "$(PKG)" ]; then echo "Usage: make port-slice PKG=<cargo_package_name> SUITE=<integration_test_name> [FILTER=<test_name_substring>] [FIXTURES=0] [GATES=1]"; exit 1; fi
	@if [ -z "$(SUITE)" ]; then echo "Usage: make port-slice PKG=<cargo_package_name> SUITE=<integration_test_name> [FILTER=<test_name_substring>] [FIXTURES=0] [GATES=1]"; exit 1; fi
	@if [ "$(FIXTURES)" != "0" ]; then $(MAKE) compat-fixtures; fi
	$(MAKE) test-smoke
	$(MAKE) test-crate PKG=$(PKG)
	@if [ -n "$(FILTER)" ]; then \
		$(MAKE) test-suite-filter SUITE=$(SUITE) FILTER=$(FILTER); \
	else \
		$(MAKE) test-suite SUITE=$(SUITE); \
	fi
	@if [ "$(GATES)" = "1" ]; then $(MAKE) test-gates; fi

test-core-fixtures:
	mise x -- cargo test -p json-joy-core --test compat_fixtures --test patch_codec_from_fixtures --test patch_encode_from_canonical_fixtures --test patch_alt_codecs_from_fixtures --test patch_compaction_from_fixtures --test patch_schema_from_fixtures --test util_diff_from_fixtures --test model_codec_from_fixtures --test model_encode_from_canonical_fixtures --test model_apply_replay_from_fixtures --test model_diff_parity_from_fixtures --test model_diff_dst_keys_from_fixtures --test model_api_from_fixtures --test model_api_proxy_fanout_from_fixtures --test model_lifecycle_from_fixtures --test lessdb_model_manager_from_fixtures --test codec_indexed_binary_from_fixtures --test codec_sidecar_binary_from_fixtures

test-core-upstream:
	mise x -- cargo test -p json-joy-core --test upstream_port_diff_matrix --test upstream_port_diff_smoke --test upstream_port_diff_native_support_matrix --test upstream_port_diff_server_clock_matrix --test upstream_port_diff_nonempty_scalar_matrix --test upstream_port_diff_nonempty_recursive_matrix --test upstream_port_model_apply_matrix --test upstream_port_model_graph_invariants --test upstream_port_model_encode_matrix --test upstream_port_model_runtime_smoke --test upstream_port_model_api_matrix --test upstream_port_model_api_proxy_matrix --test upstream_port_model_api_fanout_matrix --test upstream_port_model_api_events_matrix --test upstream_port_nodes_family_matrix --test upstream_port_nodes_rga_matrix --test upstream_port_str_rga_matrix --test upstream_port_val_lww_matrix --test upstream_port_patch_builder_matrix --test upstream_port_patch_builder_smoke --test upstream_port_patch_rebase_matrix --test upstream_port_patch_compaction_matrix --test upstream_port_patch_compact_codec_matrix --test upstream_port_patch_compact_binary_codec_matrix --test upstream_port_patch_verbose_codec_matrix --test upstream_port_patch_schema_matrix --test upstream_port_patch_clock_codec_matrix --test upstream_port_codec_indexed_binary_matrix --test upstream_port_codec_sidecar_binary_matrix --test upstream_port_util_diff_str_bin_matrix --test upstream_port_util_diff_line_matrix

test-core-differential:
	mise x -- cargo test -p json-joy-core --test differential_runtime_seeded --test differential_codec_seeded --test differential_patch_codecs_seeded --test differential_patch_compaction_seeded --test differential_patch_schema_seeded --test differential_util_diff_seeded

test-core-property:
	mise x -- cargo test -p json-joy-core --test property_replay_idempotence --test property_codec_roundtrip_invariants --test property_model_api_event_convergence

test-core:
	mise x -- cargo test -p json-joy-core

test-core-fast-diff:
	mise x -- cargo test -p json-joy-core --test upstream_port_build_con_view_matrix --test upstream_port_diff_any_matrix --test upstream_port_diff_obj_matrix --test upstream_port_diff_vec_matrix --test upstream_port_diff_dst_keys_matrix --test upstream_port_diff_smoke --test model_diff_parity_from_fixtures --offline

test-core-fast-hash:
	mise x -- cargo test -p json-joy-core --test upstream_port_json_hash_matrix --test differential_json_hash_seeded --test json_hash_from_fixtures --offline

test-core-full:
	mise x -- cargo test -p json-joy-core --offline

bindings-python:
	bin/generate-bindings.sh python

compat-fixtures:
	bin/generate-compat-fixtures.sh

wasm-build:
	CARGO_NET_OFFLINE=true wasm-pack build crates/json-joy-wasm --target nodejs --release

wasm-bench: wasm-build
	node bindings/wasm/bench/replay-batch-matrix.cjs

wasm-bench-one: wasm-build
	node bindings/wasm/bench/replay-batch.cjs

wasm-bench-engine-one: wasm-build
	node bindings/wasm/bench/replay-engine-one.cjs

wasm-interop: wasm-build
	node bindings/wasm/bench/interop-mixed.cjs

wasm-bench-realistic: wasm-build
	node bindings/wasm/bench/lessdb-realistic.cjs
