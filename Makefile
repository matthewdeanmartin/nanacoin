.DEFAULT_GOAL := help
NPM ?= npm

.PHONY: help setup test test-ui test-ui-demo test-rust test-rust-s2 coverage coverage-setup coverage-ui coverage-ui-demo coverage-rust coverage-rust-s2
help:
	@echo "make setup           Install Angular dependencies from the lockfile"
	@echo "make test            Run Angular and Rust unit/integration tests"
	@echo "make test-ui-demo    Run Angular tests against the demo build"
	@echo "make test-rust-s2    Run Rust tests with the S2 capacity profile"
	@echo "make coverage-setup  Install Rust coverage tooling (once per toolchain)"
	@echo "make coverage        Generate Angular and Rust HTML/LCOV/JSON reports"
	@echo "make coverage-ui-demo / coverage-rust-s2  Cover alternate profiles"
	@echo "See CONTRIBUTING.md for report paths and commands without Make."
setup:
	cd nanacoin_ui && $(NPM) ci
test: test-ui test-rust
test-ui:
	"$(MAKE)" -C nanacoin_ui test
test-ui-demo:
	"$(MAKE)" -C nanacoin_ui test-demo
test-rust:
	"$(MAKE)" -C nanacoin_rs test
test-rust-s2:
	"$(MAKE)" -C nanacoin_rs test-s2
coverage-setup:
	"$(MAKE)" -C nanacoin_rs coverage-setup
# Recursive recipes are sequential, even with make -j: Rust profiles share instrumentation.
coverage:
	"$(MAKE)" coverage-ui
	"$(MAKE)" coverage-rust
coverage-ui:
	"$(MAKE)" -C nanacoin_ui coverage
coverage-ui-demo:
	"$(MAKE)" -C nanacoin_ui coverage-demo
coverage-rust:
	"$(MAKE)" -C nanacoin_rs coverage
coverage-rust-s2:
	"$(MAKE)" -C nanacoin_rs coverage-s2
