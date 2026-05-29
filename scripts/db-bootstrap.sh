#!/bin/sh
set -eu

: "${DATABASE_URL:?DATABASE_URL is required}"

MIGRATIONS_DIR="${MIGRATIONS_DIR:-/app/migrations}"
SEED_DIR="${SEED_DIR:-/app/seed}"
DB_WAIT_ATTEMPTS="${DB_WAIT_ATTEMPTS:-60}"
DB_WAIT_SECONDS="${DB_WAIT_SECONDS:-2}"

wait_for_db() {
  attempt=1
  while [ "$attempt" -le "$DB_WAIT_ATTEMPTS" ]; do
    if psql "$DATABASE_URL" -X -v ON_ERROR_STOP=1 -c "SELECT 1;" >/dev/null 2>&1; then
      return 0
    fi

    echo "Waiting for database (${attempt}/${DB_WAIT_ATTEMPTS})..."
    sleep "$DB_WAIT_SECONDS"
    attempt=$((attempt + 1))
  done

  echo "Database did not become ready in time." >&2
  exit 1
}

ensure_migration_table() {
  psql "$DATABASE_URL" -X -v ON_ERROR_STOP=1 <<'SQL'
CREATE TABLE IF NOT EXISTS schema_migrations (
  filename TEXT PRIMARY KEY,
  applied_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
SQL
}

apply_migrations() {
  for file in "$MIGRATIONS_DIR"/*.sql; do
    [ -e "$file" ] || continue

    filename=$(basename "$file")
    already_applied=$(psql "$DATABASE_URL" -X -t -A -v ON_ERROR_STOP=1 -c "SELECT 1 FROM schema_migrations WHERE filename = '$filename' LIMIT 1;")

    if [ "$already_applied" = "1" ]; then
      echo "Skipping already applied migration: $filename"
      continue
    fi

    echo "Applying migration: $filename"
    {
      echo "BEGIN;"
      cat "$file"
      echo
      echo "INSERT INTO schema_migrations (filename) VALUES ('$filename');"
      echo "COMMIT;"
    } | psql "$DATABASE_URL" -X -v ON_ERROR_STOP=1
  done
}

apply_seed_file() {
  file="$1"
  echo "Applying seed: $(basename "$file")"
  psql "$DATABASE_URL" -X -v ON_ERROR_STOP=1 -f "$file"
}

apply_seeds() {
  SEED_ORDER_FILE="$(dirname "$0")/seed-order.txt"
  if [ ! -f "$SEED_ORDER_FILE" ]; then
    echo "ERROR: seed-order.txt not found at $SEED_ORDER_FILE" >&2
    exit 1
  fi

  while IFS= read -r seed_file || [ -n "$seed_file" ]; do
    # Skip empty lines and comments
    case "$seed_file" in
      ''|\#*) continue ;;
    esac

    seed_path="$SEED_DIR/$seed_file"
    if [ -f "$seed_path" ]; then
      apply_seed_file "$seed_path"
    else
      echo "ERROR: seed file listed in seed-order.txt but not found: $seed_file" >&2
      exit 1
    fi
  done < "$SEED_ORDER_FILE"
}

echo "Bootstrapping database..."
wait_for_db
ensure_migration_table
apply_migrations
apply_seeds
echo "Database bootstrap complete."
