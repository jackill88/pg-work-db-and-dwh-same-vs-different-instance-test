#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

if [[ "${1:-}" == "same" ]]; then
  docker compose -f "${ROOT_DIR}/docker-compose.same-instance.yml" down -v
elif [[ "${1:-}" == "separate" ]]; then
  docker compose -f "${ROOT_DIR}/docker-compose.separate-instances.yml" down -v
else
  docker compose -f "${ROOT_DIR}/docker-compose.same-instance.yml" down -v || true
  docker compose -f "${ROOT_DIR}/docker-compose.separate-instances.yml" down -v || true
fi

echo "Stacks stopped and volumes removed."
