SHELL := bash
.DEFAULT_GOAL := help

CARGO ?= cargo
PYTHON ?= python
export CARGO

.PHONY: help run migrate-login build test check fmt smoke certs certs-check rotate-certs firmware web bundle deploy probe-board web-check run-bundle

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
	  'make firmware  Build ESP32-S3 firmware including Angular (no flashing)' \
	  'make bundle    Build desktop server including Angular' \
	  'make run-bundle  Serve UI + API locally on port 8080' \
	  'make web-check Test bundled assets and deployment safety without a board' \
	  'make deploy PORT=COM9  Build and update an existing Rust board; preserves ledger' \
	  'make probe-board ADDRESS=192.168.1.158  Strict TLS/API/site post-flash check'

run:
	bash scripts/run-local.sh

migrate-login: build
	$(PYTHON) scripts/migrate-login.py

build:
	$(CARGO) build --locked

test:
	$(CARGO) test --locked

check:
	$(CARGO) fmt --check
	$(CARGO) clippy --locked --all-targets -- -D warnings
	$(CARGO) test --locked
	"$(MAKE)" smoke

fmt:
	$(CARGO) fmt

smoke: build
	$(PYTHON) scripts/smoke.py

certs:
	bash scripts/dev-certs.sh

certs-check:
	bash scripts/test-certs.sh

rotate-certs:
	bash scripts/rotate-certs.sh --yes

firmware:
	bash scripts/build-esp32.sh

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

deploy:
	bash scripts/deploy.sh "$(PORT)"

probe-board:
	$(PYTHON) scripts/probe-board.py --address "$(ADDRESS)"
