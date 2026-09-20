# Documentation targets.
#
#   make serve   - live-reloading preview at http://127.0.0.1:8000
#   make build   - render static HTML into ../site
#   make check   - build with warnings treated as errors
#
# mkdocs.yml lives in the repo root, so every target runs from there.

.EXPORT_ALL_VARIABLES:

# Wrapping in `uv run` is slower, so skip it if a venv is already active.
ifeq ($(origin VIRTUAL_ENV),undefined)
    VENV := uv run
else
    VENV :=
endif

ROOT := ..
PORT ?= 8000

.PHONY: help
help:
	@echo "serve   - preview the docs at http://127.0.0.1:$(PORT)"
	@echo "build   - render static HTML into ../site"
	@echo "check   - build with warnings as errors"
	@echo "deps    - install documentation dependencies"
	@echo "clean   - remove the rendered site"

.PHONY: deps
deps:
	@uv pip install -r requirements.txt

# --dirty rebuilds only changed pages, which keeps reloads fast.
.PHONY: serve
serve:
	cd $(ROOT) && $(VENV) mkdocs serve --dev-addr 127.0.0.1:$(PORT) --dirty

.PHONY: build
build:
	cd $(ROOT) && $(VENV) mkdocs build

# --strict fails on broken links and nav entries pointing at missing files.
.PHONY: check
check:
	cd $(ROOT) && $(VENV) mkdocs build --strict

.PHONY: clean
clean:
	cd $(ROOT) && rm -rf site
