SHELL := bash
.DEFAULT_GOAL := help

CARGO ?= cargo
PYTHON ?= python
export CARGO

.PHONY: smoke-screen help run migrate-login build test test-s2 check fmt smoke certs certs-check rotate-certs adopt-ca firmware web bundle deploy provision probe-board web-check run-bundle require-board

help:
	@printf '%s\n' \
	  'make run       Start JSON API at http://127.0.0.1:8080 (Angular client: localhost:4200)' \
	  'make migrate-login  Upgrade an existing Rust token account to a username/password' \
	  'make build     Build the desktop server' \
	  'make test      Run Rust tests' \
	  'make coverage-setup  Install cargo-llvm-cov and matching LLVM tools' \
	  'make coverage / coverage-s2  Generate desktop HTML, LCOV and JSON coverage' \
	  'make check     Check formatting, lint, tests, and HTTP restart smoke' \
	  'make fmt       Format Rust source' \
	  'make smoke     Build and test the real desktop HTTP server' \
	  'make certs     Generate local development TLS certificate files' \
	  'make certs-check  Validate certificate/key compatibility without a board' \
	  'make rotate-certs  Archive the current CA and generate a compatible replacement' 	  'make adopt-ca CA_DIR=dir  Sign with an existing household CA (e.g. mastomini); optional' \
	  'make firmware BOARD=s3|s2  Build one bank firmware including Angular (no flashing); HTTP2=1 adds HTTP/2' \
	  'make bundle    Build desktop server including Angular' \
	  'make run-bundle  Serve UI + API locally on port 8080' \
	  'make web-check Test bundled assets and deployment safety without a board' \
	  'make test-s2   Run Rust tests with the S2 bank capacity profile' \
	  'make deploy BOARD=s3|s2 PORT=COM9  Update an existing bank; preserves ledger' \
	  'make provision BOARD=s2 PORT=COMn  FIRST INSTALL ONLY: erase + install a new bank' \
	  'make probe-board BOARD=s3|s2 ADDRESS=192.168.1.158  Strict TLS/API/site post-flash check' \
	  '' \
	  'BOARD is always explicit: s3 = nanacoin.local (ESP32-S3), s2 = nanacoin-s2.local (ESP32-S2).'

run:
	bash scripts/run-local.sh

migrate-login: build
	$(PYTHON) scripts/migrate-login.py

build:
	$(CARGO) build --locked

test:
	$(CARGO) test --locked

test-s2:
	$(CARGO) test --locked --features board-s2

.PHONY: coverage-setup coverage coverage-s2
coverage-setup:
	rustup component add llvm-tools-preview
	$(CARGO) install cargo-llvm-cov --version 0.9.1 --locked

# Desktop tests cover the shared ledger/API. Do not run these profiles concurrently.
# Exclude test harness files and build.rs from the application coverage denominator.
COVERAGE_IGNORE := '([/\\]tests[/\\]|[/\\]build\.rs$$)'
coverage:
	$(CARGO) llvm-cov --locked --all-targets --ignore-filename-regex $(COVERAGE_IGNORE) --html --output-dir coverage
	$(CARGO) llvm-cov report --ignore-filename-regex $(COVERAGE_IGNORE) --lcov --output-path coverage/lcov.info
	$(CARGO) llvm-cov report --ignore-filename-regex $(COVERAGE_IGNORE) --json --summary-only --output-path coverage/summary.json
	$(CARGO) llvm-cov report --ignore-filename-regex $(COVERAGE_IGNORE) --summary-only

coverage-s2:
	$(CARGO) llvm-cov --locked --all-targets --features board-s2 --ignore-filename-regex $(COVERAGE_IGNORE) --html --output-dir coverage-s2
	$(CARGO) llvm-cov report --ignore-filename-regex $(COVERAGE_IGNORE) --lcov --output-path coverage-s2/lcov.info
	$(CARGO) llvm-cov report --ignore-filename-regex $(COVERAGE_IGNORE) --json --summary-only --output-path coverage-s2/summary.json
	$(CARGO) llvm-cov report --ignore-filename-regex $(COVERAGE_IGNORE) --summary-only

check:
	$(CARGO) fmt --check
	$(CARGO) clippy --locked --all-targets -- -D warnings
	$(CARGO) clippy --locked --all-targets --features board-s2 -- -D warnings
	$(CARGO) test --locked
	"$(MAKE)" test-s2
	"$(MAKE)" smoke

fmt:
	$(CARGO) fmt

smoke-screen: build
	$(PYTHON) scripts/minicloud-smoke.py

smoke: build
	$(PYTHON) scripts/smoke.py

certs:
	bash scripts/dev-certs.sh s3
	bash scripts/dev-certs.sh s2

certs-check:
	bash scripts/test-certs.sh s3
	bash scripts/test-certs.sh s2

rotate-certs:
	bash scripts/rotate-certs.sh --yes

adopt-ca:
	@[ -n "$(CA_DIR)" ] || { echo 'Set CA_DIR to a folder with rootCA.pem and rootCA-key.pem, e.g. CA_DIR=../../mastomini/mastomini_rs/.local/ca' >&2; exit 2; }
	bash scripts/adopt-ca.sh "$(CA_DIR)" --yes

require-board:
	@case "$(BOARD)" in s3|s2) ;; *) echo 'Set BOARD=s3 (nanacoin.local) or BOARD=s2 (nanacoin-s2.local).' >&2; exit 2;; esac

firmware: require-board
	bash scripts/build-esp32.sh "$(BOARD)"

web:
	bash scripts/build-web.sh

bundle: web
	$(CARGO) build --locked --features bundled-web

run-bundle: bundle
	$(CARGO) run --locked --features bundled-web

web-check: bundle
	$(CARGO) test --locked --features bundled-web
	$(PYTHON) scripts/web-smoke.py
	$(PYTHON) scripts/test_deploy.py

deploy: require-board
	bash scripts/deploy.sh "$(BOARD)" "$(PORT)"

provision: require-board
	bash scripts/provision.sh "$(BOARD)" "$(PORT)"

probe-board: require-board
	$(PYTHON) scripts/probe-board.py --board "$(BOARD)" --address "$(ADDRESS)"
