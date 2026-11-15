.PHONY: check test test-terminal
check:
	cargo fmt --all -- --check
	cargo clippy --locked --all-targets -- -D warnings
	cargo build --locked

test:
	INSTA_UPDATE=no INSTA_FORCE_PASS=0 cargo test --locked --lib --bins --tests
	$(MAKE) test-terminal

test-terminal:
	cargo build --locked --example terminal_lifecycle
	cargo test --locked --test terminal_pty -- --ignored --test-threads=1
	python3 tests/terminal_screen.py
	python3 tests/terminal_lifecycle.py
	python3 tests/release.py

.PHONY: test-replay test-scheduler test-live bench test-load test-soak
E2E_SUITE ?= slice

test-replay:
	cargo test --locked --test replay

test-scheduler:
	cargo test --locked --test scheduler

test-live:
	./scripts/e2e-local-k3d.sh --suite $(E2E_SUITE)

bench:
	cargo bench --locked --bench pipeline

test-load:
	mkdir -p artifacts/performance
	CNPG_WATCH_LOAD_SECONDS=60 CNPG_WATCH_LOAD_REPORT=artifacts/performance/load.json cargo test --release --locked --test load -- --ignored --nocapture

test-soak:
	mkdir -p artifacts/performance
	CNPG_WATCH_LOAD_SECONDS=1800 CNPG_WATCH_LOAD_REPORT=artifacts/performance/soak.json cargo test --release --locked --test load -- --ignored --nocapture

.PHONY: cli-artifacts test-cleanup test-tui package
test-tui:
	$(MAKE) test-live E2E_SUITE=operations

package:
	bash scripts/package-local.sh

cli-artifacts:
	cargo run --locked --example generate_cli_artifacts -- artifacts/release-docs
	cp artifacts/release-docs/command-reference.md docs/command-reference.md
	mkdir -p examples/completions
	cp artifacts/release-docs/completions/* examples/completions/

test-cleanup:
	python3 scripts/check-cleanup.py
