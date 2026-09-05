#!/usr/bin/env bash
set -euo pipefail

wait_for_postgres_url() {
  local label="$1"
  local url="$2"
  local timeout_seconds="${3:-120}"
  local elapsed=0

  echo "==> Waiting for ${label} (${url})"
  until docker run --rm --network host postgres:16-alpine \
    psql "${url}" -c "SELECT 1" >/dev/null 2>&1; do
    if (( elapsed >= timeout_seconds )); then
      echo "Timed out after ${timeout_seconds}s waiting for ${label}" >&2
      return 1
    fi
    sleep 2
    elapsed=$((elapsed + 2))
  done
  echo "==> ${label} is ready"
}
