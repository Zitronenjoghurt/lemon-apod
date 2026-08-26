DATA ?= $(CURDIR)/data
ARCHIVER = APOD_DATA_DIR=$(DATA) cargo run -q --profile cli -p apod-archiver --
API = APOD_DATA_DIR=$(DATA) APOD_STATIC_DIR=$(CURDIR)/web/dist cargo run -q -p apod-api
COMPOSE = docker compose -f docker/compose.yaml

.PHONY: help check test fmt lint api web dev backfill status quality reparse thumbs pictures sky \
        notify rating rating-import rating-export legacy-export legacy-import docker seed up \
        down logs ps shell

help:
	@grep -E '^[a-z-]+:.*?## .*$$' $(MAKEFILE_LIST) | awk -F':.*?## ' '{printf "  \033[36m%-12s\033[0m %s\n", $$1, $$2}'

check: fmt lint test ## Everything CI runs

fmt: ## Format and check formatting
	cargo fmt --all
	cd web && npm run format

lint: ## Clippy + eslint + vue-tsc
	cargo clippy --all-targets --all-features -- -D warnings
	cd web && npm run type-check && npm run lint

test: ## Rust test suite
	cargo test --workspace --all-features

web: ## Build the frontend into web/dist
	cd web && npm run build

api: web ## Serve the API and the built frontend on :51995
	$(API)

dev: ## Vite dev server (proxies /api to a locally running apod-api)
	cd web && npm run dev

backfill: ## Fetch a few pages into ./data. Respect the rate limit, this hits NASA
	$(ARCHIVER) backfill --limit $(or $(N),5)

status: ## Coverage and index health
	$(ARCHIVER) status

quality: ## Parse warnings, to guide parser refinement
	$(ARCHIVER) quality

reparse: ## Rebuild the index from the HTML on disk
	$(ARCHIVER) reparse

thumbs: ## Generate any missing thumbnails
	$(ARCHIVER) thumbs

pictures: ## Hash thumbnails and group the pictures the archive has shown more than once
	$(ARCHIVER) pictures

sky: ## Poll the launch and space weather feeds once into ./data/sky.db
	$(ARCHIVER) sky

rating: ## Votes collected per category, and how far each has to go
	$(ARCHIVER) rating status

rating-import: ## Load baseline/rating into ./data/votes.db as priors. Run this if votes.db is lost
	$(ARCHIVER) rating import

rating-export: ## Fit what has been collected and write baseline/rating back out, ready to commit
	$(ARCHIVER) rating export

legacy-export: ## Pack ./data/html and its fetch state into a snapshot. OUT=<dir> to place it
	$(ARCHIVER) legacy-export --out $(or $(OUT),$(CURDIR))

legacy-import: ## Restore a snapshot into ./data. FROM=<path|url>, SHA=<hash>, FORCE=1 to replace differing pages
	$(ARCHIVER) legacy-import $(FROM) $(if $(SHA),--sha256 $(SHA)) $(if $(FORCE),--force)

notify: ## Send what is due. DRY=1 lists it instead, SEED=1 records it as sent without sending
	$(ARCHIVER) notify $(if $(DRY),--dry-run) $(if $(SEED),--seed)

docker: ## Build both images locally
	docker build -f docker/Dockerfile --target archiver -t lemon-apod-archiver:dev .
	docker build -f docker/Dockerfile --target api -t lemon-apod-api:dev .

seed: ## Optional: fetch a few pages through the container so a fresh ./data is not empty
	$(COMPOSE) run --rm archiver backfill --limit $(or $(N),5)

up: ## Build and start both services on :51995
	$(COMPOSE) up -d --build

down: ## Stop both services (./data is a bind mount and survives)
	$(COMPOSE) down

logs: ## Follow both services' logs
	$(COMPOSE) logs -f

ps: ## Container and health status
	$(COMPOSE) ps

shell: ## Poke at ./data with a shell. The service images are distroless and carry none
	docker run --rm -it -v $(DATA):/data -w /data busybox sh