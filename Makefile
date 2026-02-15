.PHONY: check fmt test bindings-python compat-fixtures

check:
	mise x -- cargo check

fmt:
	mise x -- cargo fmt --all

test:
	mise x -- cargo test --workspace

bindings-python:
	bin/generate-bindings.sh python

compat-fixtures:
	bin/generate-compat-fixtures.sh
