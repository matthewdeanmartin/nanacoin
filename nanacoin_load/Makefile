HOST ?= http://192.168.1.158
SCENARIO ?= browse
USERS ?= 4
DURATION ?= 60
STEPS ?= 1,2,4,8,12
STAGE ?= 30
PORT ?= 8090

.PHONY: install test prepare e2e load ramp soak suite ui reports open
install:
	uv sync --python 3.14
test:
	uv run pytest
prepare:
	uv run ncload prepare --host "$(HOST)"
e2e:
	uv run ncload e2e --host "$(HOST)"
load:
	uv run ncload run --host "$(HOST)" --scenario $(SCENARIO) --users $(USERS) --duration $(DURATION)
ramp:
	uv run ncload run --host "$(HOST)" --scenario $(SCENARIO) --steps $(STEPS) --stage $(STAGE)
soak:
	uv run ncload run --host "$(HOST)" --scenario $(SCENARIO) --users $(USERS) --duration 1800
suite:
	uv run ncload suite --host "$(HOST)" --steps $(STEPS) --stage $(STAGE)
ui:
	uv run ncload ui --host "$(HOST)" --scenario $(SCENARIO)
reports:
	uv run ncload reports --port $(PORT)
open:
	uv run ncload open
