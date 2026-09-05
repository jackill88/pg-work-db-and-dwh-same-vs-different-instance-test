#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCENARIO="${1:-unspecified}"
PROMETHEUS_URL="${PROMETHEUS_URL:-http://localhost:9090}"
OUTPUT_DIR="${RESULTS_DIR:-${ROOT_DIR}/results/${SCENARIO}}"
QUERIES_FILE="${ROOT_DIR}/prometheus/queries.txt"
EXPORT_START_UNIX="${EXPORT_START_UNIX:-}"
EXPORT_END_UNIX="${EXPORT_END_UNIX:-}"
EXPORT_STEP="${EXPORT_STEP:-5s}"
TIMESTAMP="$(date -u +"%Y%m%dT%H%M%SZ")"

mkdir -p "${OUTPUT_DIR}"

if [[ ! -f "${QUERIES_FILE}" ]]; then
  echo "Missing queries file: ${QUERIES_FILE}" >&2
  exit 1
fi

use_range=false
if [[ -n "${EXPORT_START_UNIX}" && -n "${EXPORT_END_UNIX}" ]]; then
  use_range=true
fi

manifest_queries=()

while IFS= read -r query || [[ -n "${query}" ]]; do
  [[ -z "${query}" || "${query}" =~ ^# ]] && continue

  safe_name="$(echo "${query}" | tr ' /:(){}[],=' '___________' | tr -cd '[:alnum:]_-' | cut -c1-120)"

  if [[ "${use_range}" == true ]]; then
    output_file="${OUTPUT_DIR}/range-${safe_name}.json"
    echo "Exporting range: ${query}"
    curl -fsS -G "${PROMETHEUS_URL}/api/v1/query_range" \
      --data-urlencode "query=${query}" \
      --data-urlencode "start=${EXPORT_START_UNIX}" \
      --data-urlencode "end=${EXPORT_END_UNIX}" \
      --data-urlencode "step=${EXPORT_STEP}" \
      -o "${output_file}"
  else
    output_file="${OUTPUT_DIR}/instant-${safe_name}.json"
    echo "Exporting instant: ${query}"
    curl -fsS -G "${PROMETHEUS_URL}/api/v1/query" \
      --data-urlencode "query=${query}" \
      -o "${output_file}"
  fi

  manifest_queries+=("${output_file}")
done < "${QUERIES_FILE}"

manifest_path="${OUTPUT_DIR}/manifest.json"

if command -v jq >/dev/null 2>&1; then
  queries_json="$(printf '%s\n' "${manifest_queries[@]}" | jq -R . | jq -s .)"
  jq -n \
    --arg scenario "${SCENARIO}" \
    --arg exported_at "${TIMESTAMP}" \
    --arg prometheus_url "${PROMETHEUS_URL}" \
    --arg queries_file "${QUERIES_FILE}" \
    --arg export_mode "$(if [[ "${use_range}" == true ]]; then echo range; else echo instant; fi)" \
    --arg export_start_unix "${EXPORT_START_UNIX}" \
    --arg export_end_unix "${EXPORT_END_UNIX}" \
    --arg export_step "${EXPORT_STEP}" \
    --argjson queries "${queries_json}" \
    '{
      scenario: $scenario,
      exported_at: $exported_at,
      prometheus_url: $prometheus_url,
      queries_file: $queries_file,
      export_mode: $export_mode,
      export_start_unix: (if $export_start_unix == "" then null else ($export_start_unix | tonumber) end),
      export_end_unix: (if $export_end_unix == "" then null else ($export_end_unix | tonumber) end),
      export_step: $export_step,
      query_results: $queries
    }' > "${manifest_path}"
else
  {
    echo "{"
    echo "  \"scenario\": \"${SCENARIO}\","
    echo "  \"exported_at\": \"${TIMESTAMP}\","
    echo "  \"prometheus_url\": \"${PROMETHEUS_URL}\","
    echo "  \"queries_file\": \"${QUERIES_FILE}\","
    echo "  \"export_mode\": \"$(if [[ "${use_range}" == true ]]; then echo range; else echo instant; fi)\","
    echo "  \"export_start_unix\": ${EXPORT_START_UNIX:-null},"
    echo "  \"export_end_unix\": ${EXPORT_END_UNIX:-null},"
    echo "  \"export_step\": \"${EXPORT_STEP}\","
    echo "  \"query_results\": ["
    for i in "${!manifest_queries[@]}"; do
      if [[ "${i}" -gt 0 ]]; then
        echo ","
      fi
      printf '    "%s"' "${manifest_queries[$i]}"
    done
    echo
    echo "  ]"
    echo "}"
  } > "${manifest_path}"
fi

echo "Prometheus snapshots written to ${OUTPUT_DIR}"
echo "Manifest: ${manifest_path}"
