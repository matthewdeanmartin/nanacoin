SHELL := bash
.DEFAULT_GOAL := help

CARGO ?= cargo
PYTHON ?= python
export CARGO

.PHONY: help run migrate-login build test test-s2 check fmt smoke certs certs-check rotate-certs firmware web bundle deploy provision probe-board web-check run-bundle require-board

help:
	@printf '%s\n' \
	  'make run       Start JSON API at http://127.0.0.1:8080 (Angular client: localhost:4200)' \
	  'make migrate-login  Upgrade an existing Rust token account to a username/password' \
	  'make build     Build the desktop server' \
	  'make test      Run Rust tests' \
	  'make check     Check formatting, lint, tests, and HTTP restart smoke' \
	  'make fmt       Format Rust source' \
	  'make smoke     Build and test the real desktop HTTP server' \
	  'make certs     Generate local development TLS certificate files' \
	  'make certs-check  Validate certificate/key compatibility without a board' \
	  'make rotate-certs  Archive the current CA and generate a compatible replacement' \
	  'make firmware BOARD=s3|s2  Build one bank firmware including Angular (no flashing)' \
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

check:
	$(CARGO) fmt --check
	$(CARGO) clippy --locked --all-targets -- -D warnings
	$(CARGO) clippy --locked --all-targets --features board-s2 -- -D warnings
	$(CARGO) test --locked
	"$(MAKE)" test-s2
	"$(MAKE)" smoke

fmt:
	$(CARGO) fmt

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
