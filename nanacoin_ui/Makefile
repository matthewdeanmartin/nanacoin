.DEFAULT_GOAL := help
NPM ?= npm
.PHONY: help test test-demo coverage coverage-demo
help:
	@echo "make test / test-demo       Run live or demo Angular tests once"
	@echo "make coverage / coverage-demo  Generate coverage/live or coverage/demo"
test:
	$(NPM) run test:ci
test-demo:
	$(NPM) run test:demo
coverage:
	$(NPM) run coverage
coverage-demo:
	$(NPM) run coverage:demo
