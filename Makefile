.PHONY: help up down db-up logs \
       download-worldpop download-naturalearth download-geonames download-all \
       ingest-worldpop ingest-naturalearth ingest-geonames ingest-all \
       init-db migrate bootstrap deploy \
       setup api-build test bench clean

# Load .env and export every variable to recipe sub-processes
ifneq (,$(wildcard .env))
include .env
export POSTGRES_USER POSTGRES_PASSWORD POSTGRES_DB DB_PORT API_PORT POOL_SIZE \
       DATABASE_URL HOST_DATABASE_URL
endif

API_PORT ?= 8080
API_URL  ?= http://localhost:$(API_PORT)/api/v1

# Host-side tools (psql, ingestion python scripts) cannot resolve
# `host.docker.internal`, so allow an explicit override. Falls back to
# DATABASE_URL for the simple case where the DB is reachable from the host.
HOST_DB_URL ?= $(if $(HOST_DATABASE_URL),$(HOST_DATABASE_URL),$(DATABASE_URL))

help: ## Show available targets
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-22s\033[0m %s\n", $$1, $$2}'

# ── Docker ──

up: ## Start all services (db + api)
	docker compose up -d --build

down: ## Stop all services
	docker compose down

db-up: ## Start only the database
	docker compose up -d db

logs: ## Tail service logs
	docker compose logs -f

# ── Data downloads ──

download-worldpop: ## Download WorldPop GeoTIFF (~723 MB)
	bash ingestion/download_worldpop.sh

download-naturalearth: ## Download Natural Earth boundaries (~5 MB)
	bash ingestion/download_naturalearth.sh

download-geonames: ## Download GeoNames data (~380 MB)
	bash ingestion/download_geonames.sh

download-all: download-worldpop download-naturalearth download-geonames ## Download all datasets

# ── Data ingestion ──

ingest-worldpop: ## Ingest WorldPop into database
	pip install -q -r ingestion/requirements.txt
	DATABASE_URL="$(HOST_DB_URL)" python -u ingestion/ingest.py

ingest-naturalearth: ## Ingest Natural Earth into database
	pip install -q -r ingestion/requirements.txt
	DATABASE_URL="$(HOST_DB_URL)" python -u ingestion/ingest_naturalearth.py

ingest-geonames: ## Ingest GeoNames into database
	pip install -q -r ingestion/requirements.txt
	DATABASE_URL="$(HOST_DB_URL)" python -u ingestion/ingest_geonames.py

ingest-all: ingest-naturalearth ingest-worldpop ingest-geonames ## Ingest all datasets

# ── Schema / migration ──

init-db: ## Create base schema on an empty database (runs docker/init.sql)
	@test -n "$(HOST_DB_URL)" || (echo "Set HOST_DATABASE_URL (or DATABASE_URL reachable from host) in .env" && exit 1)
	psql "$(HOST_DB_URL)" -v ON_ERROR_STOP=1 -f docker/init.sql

migrate: ## Idempotently ensure extensions + indexes (safe to re-run)
	@test -n "$(HOST_DB_URL)" || (echo "Set HOST_DATABASE_URL (or DATABASE_URL reachable from host) in .env" && exit 1)
	psql "$(HOST_DB_URL)" -v ON_ERROR_STOP=1 -f docker/migrate.sql

bootstrap: ## First-time VPS bootstrap: init schema → download → ingest → migrate
	@test -n "$(HOST_DB_URL)" || (echo "Set HOST_DATABASE_URL (or DATABASE_URL reachable from host) in .env" && exit 1)
	$(MAKE) init-db
	$(MAKE) download-all
	$(MAKE) ingest-all
	$(MAKE) migrate

deploy: ## Re-deploy against an existing DB: migrate schema + (re)start api
	$(MAKE) migrate
	docker compose up -d --build --no-deps api

# ── Full setup ──

setup: download-all db-up ingest-all migrate up ## Full setup: download, ingest, migrate, start

# ── Development ──

api-build: ## Build the API binary locally
	cd api && cargo build --release

test: ## Run smoke tests against the running API
	@echo "=== Root ===" && curl -sf http://localhost:$(API_PORT)/ | python3 -m json.tool
	@echo "\n=== Health ===" && curl -sf $(API_URL)/health | python3 -m json.tool
	@echo "\n=== Population (London) ===" && curl -sf "$(API_URL)/population?lat=51.5074&lon=-0.1278" | python3 -m json.tool
	@echo "\n=== Reverse (Tokyo) ===" && curl -sf "$(API_URL)/reverse?lat=35.6762&lon=139.6503" | python3 -m json.tool
	@echo "\n=== Exposure (NYC 10km) ===" && curl -sf "$(API_URL)/exposure?lat=40.7128&lon=-74.006&radius=10" | python3 -m json.tool
	@echo "\n=== Country (GBR) ===" && curl -sf "$(API_URL)/country/GBR" | python3 -m json.tool
	@echo "\n=== City search (colom, LK) ===" && curl -sf "$(API_URL)/cities/search?q=colom&country=LK&limit=5" | python3 -m json.tool

bench: ## Benchmark (requires 'hey': go install github.com/rakyll/hey@latest)
	hey -n 10000 -c 50 "$(API_URL)/population?lat=51.5&lon=-0.1"

clean: ## Remove all containers, volumes, and downloaded data
	docker compose down -v
	rm -rf data/
