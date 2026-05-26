#!/usr/bin/env bash
# bootstrap-smoke.sh — Throwaway Postgres bootstrap smoke test
#
# Spins up a temporary postgres:16 container, runs db-bootstrap.sh,
# and asserts post-seed SQL integrity.
#
# Usage:
#   ./scripts/bootstrap-smoke.sh              # fresh bootstrap
#   ./scripts/bootstrap-smoke.sh --migrated   # production-like (N-3 pre-applied)
#
# Requirements: Docker plus a Bash-compatible shell (Git Bash, WSL, macOS, or Linux).
# psql runs inside the container — no host psql needed.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

case "$(uname -s)" in
  MINGW*|MSYS*|CYGWIN*)
    export MSYS_NO_PATHCONV=1
    ;;
esac

host_path() {
  if command -v cygpath >/dev/null 2>&1; then
    cygpath -w "$1"
  else
    printf '%s\n' "$1"
  fi
}

CONTAINER_NAME="seed-smoke-$$-$(date +%s)"
DB_NAME="letta_city_sim"
DB_USER="sim"
DB_PASS="smoke_test_pass"
MODE="fresh"

if [ "${1:-}" = "--migrated" ]; then
  MODE="migrated"
fi

# ── Cleanup trap ───────────────────────────────────────────────────────
cleanup() {
  echo ""
  echo "Cleaning up container $CONTAINER_NAME..."
  docker rm -f "$CONTAINER_NAME" >/dev/null 2>&1 || true
}
trap cleanup EXIT

# ── Start container ────────────────────────────────────────────────────
echo "[$MODE mode] Starting postgres:16 container..."
docker run -d \
  --name "$CONTAINER_NAME" \
  -e POSTGRES_USER="$DB_USER" \
  -e POSTGRES_PASSWORD="$DB_PASS" \
  -e POSTGRES_DB="$DB_NAME" \
  postgres:16 >/dev/null

# Wait for Postgres to be ready
echo "Waiting for Postgres to be ready..."
for i in $(seq 1 30); do
  if docker exec "$CONTAINER_NAME" pg_isready -U "$DB_USER" -d "$DB_NAME" >/dev/null 2>&1; then
    echo "Postgres is ready."
    break
  fi
  if [ "$i" -eq 30 ]; then
    echo "ERROR: Postgres did not become ready in time." >&2
    exit 1
  fi
  sleep 1
done

# ── Copy files into container ──────────────────────────────────────────
echo "Copying files into container..."
docker exec "$CONTAINER_NAME" mkdir -p /app /app/scripts

docker cp "$(host_path "$REPO_ROOT/world-api/migrations")" "$CONTAINER_NAME:/app/migrations"
docker cp "$(host_path "$REPO_ROOT/seed")" "$CONTAINER_NAME:/app/seed"
docker cp "$(host_path "$REPO_ROOT/scripts/db-bootstrap.sh")" "$CONTAINER_NAME:/app/scripts/db-bootstrap.sh"
docker cp "$(host_path "$REPO_ROOT/scripts/seed-order.txt")" "$CONTAINER_NAME:/app/scripts/seed-order.txt"

docker exec "$CONTAINER_NAME" sh -c "sed -i 's/\r$//' /app/scripts/db-bootstrap.sh /app/scripts/seed-order.txt"
docker exec "$CONTAINER_NAME" chmod +x /app/scripts/db-bootstrap.sh

# ── Mode-specific setup ───────────────────────────────────────────────
if [ "$MODE" = "migrated" ]; then
  echo "Production-like mode: pre-applying first N-3 migrations..."

  # Count migration files and compute split point
  MIGRATION_COUNT=$(docker exec "$CONTAINER_NAME" sh -c 'ls /app/migrations/*.sql 2>/dev/null | wc -l')
  if [ "$MIGRATION_COUNT" -le 3 ]; then
    echo "WARNING: Only $MIGRATION_COUNT migrations found. Running fresh mode instead."
    MODE="fresh"
  else
    SPLIT=$((MIGRATION_COUNT - 3))

    # Create schema_migrations table
    docker exec "$CONTAINER_NAME" psql -U "$DB_USER" -d "$DB_NAME" -v ON_ERROR_STOP=1 -c \
      "CREATE TABLE IF NOT EXISTS schema_migrations (filename TEXT PRIMARY KEY, applied_at TIMESTAMPTZ NOT NULL DEFAULT NOW());"

    # Apply first N-3 migrations and record them
    APPLIED=0
    for migration_file in $(docker exec "$CONTAINER_NAME" sh -c 'ls /app/migrations/*.sql | sort'); do
      APPLIED=$((APPLIED + 1))
      if [ "$APPLIED" -gt "$SPLIT" ]; then
        break
      fi
      FILENAME=$(basename "$migration_file")
      echo "  Pre-applying: $FILENAME"
      docker exec "$CONTAINER_NAME" psql -U "$DB_USER" -d "$DB_NAME" -v ON_ERROR_STOP=1 -f "$migration_file"
      docker exec "$CONTAINER_NAME" psql -U "$DB_USER" -d "$DB_NAME" -v ON_ERROR_STOP=1 -c \
        "INSERT INTO schema_migrations (filename) VALUES ('$FILENAME');"
    done

    echo "Pre-applied $SPLIT of $MIGRATION_COUNT migrations."
  fi
fi

# ── Run db-bootstrap.sh ───────────────────────────────────────────────
echo "Running db-bootstrap.sh..."
docker exec \
  -e DATABASE_URL="postgres://$DB_USER:$DB_PASS@localhost:5432/$DB_NAME" \
  -e MIGRATIONS_DIR="/app/migrations" \
  -e SEED_DIR="/app/seed" \
  "$CONTAINER_NAME" \
  /app/scripts/db-bootstrap.sh

echo ""
echo "Bootstrap complete. Running post-seed assertions..."

# ── Post-seed SQL assertions ──────────────────────────────────────────
ASSERTION_FAILURES=0

run_assertion() {
  local name="$1"
  local query="$2"

  local result
  result=$(docker exec "$CONTAINER_NAME" psql -U "$DB_USER" -d "$DB_NAME" \
    -t -A -v ON_ERROR_STOP=1 -c "$query" 2>&1) || true

  if [ -z "$result" ]; then
    echo "PASS  [$name]"
  else
    echo "FAIL  [$name]"
    echo "  Violating rows:"
    echo "  $result" | head -20
    ASSERTION_FAILURES=$((ASSERTION_FAILURES + 1))
  fi
}

# 1. Dangling location references in adjacency
run_assertion "adjacency-locations" \
  "SELECT from_id FROM location_adjacency WHERE from_id NOT IN (SELECT id FROM locations) UNION SELECT to_id FROM location_adjacency WHERE to_id NOT IN (SELECT id FROM locations);"

# 2. Dangling location references in agents
run_assertion "agent-locations" \
  "SELECT id, current_location_id FROM agents WHERE current_location_id NOT IN (SELECT id FROM locations) UNION ALL SELECT id, home_location_id FROM agents WHERE home_location_id IS NOT NULL AND home_location_id NOT IN (SELECT id FROM locations);"

# 3. Dangling location references in world_objects
run_assertion "object-locations" \
  "SELECT id, location_id FROM world_objects WHERE location_id NOT IN (SELECT id FROM locations);"

# 4. Dangling agent/job references in agent_jobs
run_assertion "agent-jobs-refs" \
  "SELECT agent_id, job_id FROM agent_jobs WHERE agent_id NOT IN (SELECT id FROM agents) OR job_id NOT IN (SELECT id FROM jobs);"

# 5. Dangling jobs employer_id
run_assertion "jobs-employer" \
  "SELECT id, employer_id FROM jobs WHERE employer_id IS NOT NULL AND employer_id NOT IN (SELECT id FROM agents);"

# 6. Dangling shops references (nullable FKs checked only when set)
run_assertion "shops-refs" \
  "SELECT id, owner_id, shopkeeper_job_id FROM shops WHERE (owner_id IS NOT NULL AND owner_id NOT IN (SELECT id FROM agents)) OR (shopkeeper_job_id IS NOT NULL AND shopkeeper_job_id NOT IN (SELECT id FROM jobs));"

# 7. Dangling banks references (nullable FKs checked only when set)
run_assertion "banks-refs" \
  "SELECT id, banker_job_id, updated_by FROM banks WHERE (banker_job_id IS NOT NULL AND banker_job_id NOT IN (SELECT id FROM jobs)) OR (updated_by IS NOT NULL AND updated_by NOT IN (SELECT id FROM agents));"

# 8. Dangling location_roles references
run_assertion "location-roles-refs" \
  "SELECT location_id, agent_id FROM location_roles WHERE location_id NOT IN (SELECT id FROM locations) OR agent_id NOT IN (SELECT id FROM agents);"

# 9. Adjacency symmetry
run_assertion "adjacency-symmetry" \
  "SELECT a.from_id, a.to_id FROM location_adjacency a LEFT JOIN location_adjacency b ON a.from_id = b.to_id AND a.to_id = b.from_id WHERE b.from_id IS NULL;"

# 10. Inventory XOR violation
run_assertion "inventory-xor" \
  "SELECT id FROM inventory_items WHERE (held_by IS NULL) = (location_id IS NULL);"

# 11. Primary job uniqueness
run_assertion "primary-job-unique" \
  "SELECT agent_id, COUNT(*) FROM agent_jobs WHERE is_primary = TRUE GROUP BY agent_id HAVING COUNT(*) > 1;"

# 12. Consumable integrity
run_assertion "consumable-integrity" \
  "SELECT id, consumable_type, vital_value, quantity FROM inventory_items WHERE consumable_type IS NOT NULL AND (consumable_type NOT IN ('food','water','stamina','sleep','hygiene','appearance') OR vital_value IS NULL OR vital_value <= 0 OR quantity IS NULL OR quantity <= 0);"

# 13. Graph reachability from hub (only checks adjacency-graph nodes)
run_assertion "graph-reachability" \
  "WITH RECURSIVE reachable AS (SELECT 'notice_board'::text AS id UNION SELECT a.to_id FROM location_adjacency a JOIN reachable r ON a.from_id = r.id), adjacency_locations AS (SELECT DISTINCT id FROM (SELECT from_id AS id FROM location_adjacency UNION SELECT to_id AS id FROM location_adjacency) sub) SELECT al.id FROM adjacency_locations al WHERE al.id NOT IN (SELECT id FROM reachable);"

# 14. JSONB parse check — implicit: if seeds loaded with ON_ERROR_STOP=1, jsonb parsed OK
echo "PASS  [jsonb-implicit]  (seeds loaded with ON_ERROR_STOP=1)"

# ── Summary ───────────────────────────────────────────────────────────
echo ""
if [ "$ASSERTION_FAILURES" -eq 0 ]; then
  echo "All post-seed assertions passed ($MODE mode)."
  echo "Seed-data validation passed (strong merge safety signal)."
  exit 0
else
  echo "$ASSERTION_FAILURES assertion(s) FAILED ($MODE mode)."
  exit 1
fi
