#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
COMPOSE_FILE="${ROOT_DIR}/docker-compose.separate-instances.yml"
SCENARIO="separate_instances"

export LIVE_DATABASE_URL="${LIVE_DATABASE_URL:-postgresql://bench:bench@localhost:5433/live}"
export DWH_DATABASE_URL="${DWH_DATABASE_URL:-postgresql://bench:bench@localhost:5434/dwh}"
export SCENARIO
export COMPOSE_SCENARIO="separate_instances"
export COMPOSE_FILE="${COMPOSE_FILE:-${ROOT_DIR}/docker-compose.separate-instances.yml}"
export RESULTS_DIR="${RESULTS_DIR:-${ROOT_DIR}/results/separate-instances}"

echo "==> Starting separate-instances stack"
docker compose -f "${COMPOSE_FILE}" up -d

# shellcheck source=wait-for-postgres.sh
source "${ROOT_DIR}/scripts/wait-for-postgres.sh"
wait_for_postgres_url "live database" "${LIVE_DATABASE_URL}"
wait_for_postgres_url "dwh database" "${DWH_DATABASE_URL}"

echo "==> Building load generator"
cargo build --release --manifest-path "${ROOT_DIR}/loadgen/Cargo.toml"

echo "==> Running benchmark (${TEST_DURATION:-5m}, target ${LIVE_TARGET_RPS:-1750} live RPS)"
cargo run --release --manifest-path "${ROOT_DIR}/loadgen/Cargo.toml"

echo "Done. Prometheus UI: http://localhost:9090"
echo "Results: ${RESULTS_DIR} (latest run in timestamped subdirectory)"
