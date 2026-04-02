SHELL := /bin/sh

.PHONY: help dev db-up db-down

help:
	@echo "Targets:"
	@echo "  make dev      # run docker compose stack"
	@echo "  make db-up    # start database only"
	@echo "  make db-down  # stop database"

# Placeholder commands (Docker Compose file will be wired step-by-step)
dev:
	docker compose up --build

db-up:
	docker compose up db

db-down:
	docker compose stop db
