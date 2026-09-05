# PostgreSQL same-instance vs separate-instance benchmark

This repo compares two deployment models under identical application load:

1. **Same instance** — `live` and `dwh` databases on one PostgreSQL server
2. **Separate instances** — dedicated PostgreSQL servers for live OLTP and DWH analytics

A Rust load generator drives:

- **Live DB:** ~1750 mixed read/write ops/sec (point reads, recent-order scans, inserts, balance updates)
- **DWH:** periodic bulk inserts (5k rows/batch) plus 15 concurrent analytical readers

Prometheus collects:

- `postgres_exporter` database metrics
- `cadvisor` container CPU/memory
- load-generator latency and throughput (`:9100/metrics`)

## Prerequisites

- Docker + Docker Compose
- Rust toolchain (`cargo`)
- Enough CPU/RAM for the configured load (default 1750 ops/sec; adjust `LIVE_TARGET_RPS` if needed)

## PostgreSQL tuning

Settings live under `docker/postgres/` and are mounted into each container:

| File | Used by |
|---|---|
| `common.conf` | Shared fragment (WAL, checkpoints, NVMe `random_page_cost`) |
| `same-instance.conf` | `docker-compose.same-instance.yml` |
| `live.conf` | Separate-instances live server |
| `dwh.conf` | Separate-instances DWH server |

Each scenario file `include`s `common.conf`, then sets memory and connection limits tuned for a ~16 GB RAM host.

**Override per scenario** without editing tracked files:

```bash
cp docker/postgres/same-instance.override.conf.example docker/postgres/same-instance.local.conf
# edit same-instance.local.conf, then in docker-compose.same-instance.yml swap:
#   ./docker/postgres/same-instance.conf  →  ./docker/postgres/same-instance.local.conf
```

`*.local.conf` files are gitignored. After changing PG settings, recreate containers (`docker compose up -d --force-recreate postgres`). Existing data volumes keep their data; only runtime config changes.

Verify active settings inside a running container:

```bash
docker compose -f docker-compose.same-instance.yml exec postgres psql -U bench -c "SHOW shared_buffers; SHOW max_connections;"
```

## Quick start

### Scenario A — shared PostgreSQL instance

```bash
./scripts/run-same-instance.sh
```

Connects to:

- Live: `postgresql://bench:bench@localhost:5432/live`
- DWH:  `postgresql://bench:bench@localhost:5432/dwh`

### Scenario B — separate PostgreSQL instances

```bash
./scripts/teardown.sh same    # stop scenario A first (same Prometheus port)
./scripts/run-separate-instances.sh
```

Connects to:

- Live: `postgresql://bench:bench@localhost:5433/live`
- DWH:  `postgresql://bench:bench@localhost:5434/dwh`

## Manual load-generator run

```bash
export SCENARIO=same_instance
export LIVE_DATABASE_URL=postgresql://bench:bench@localhost:5432/live
export DWH_DATABASE_URL=postgresql://bench:bench@localhost:5432/dwh
export LIVE_TARGET_RPS=1750
export TEST_DURATION=5m
export DWH_READ_WORKERS=15

cargo run --release --manifest-path loadgen/Cargo.toml
```

Useful tuning env vars:

| Variable | Default | Purpose |
|---|---|---|
| `LIVE_TARGET_RPS` | `1750` | Target live DB ops/sec |
| `LIVE_WORKERS` | `32` | Concurrent live workers (pacing slots) |
| `LIVE_POOL_SIZE` | `48` | Live connection pool size (must fit within PG `max_connections` alongside DWH) |

If you have `LIVE_WORKERS` or `LIVE_POOL_SIZE` exported in your shell from earlier runs, unset them or the run scripts will honor those values instead of the tuned defaults.
| `DWH_READ_WORKERS` | `15` | Analytical read connections |
| `DWH_BULK_BATCH_SIZE` | `5000` | Rows per bulk insert batch |
| `DWH_BULK_INTERVAL` | `2s` | Pause between bulk batches |
| `TEST_DURATION` | `5m` | Benchmark duration |
| `METRICS_PORT` | `9100` | Loadgen Prometheus endpoint |
| `EXPORT_METRICS` | `true` | Auto-export metrics when the run finishes |
| `PROMETHEUS_URL` | `http://localhost:9090` | Prometheus for post-run export |

## Metrics and results

When a benchmark finishes, the load generator **automatically exports metrics** (enabled by default via `EXPORT_METRICS=true`):

1. Creates a timestamped run directory, e.g. `results/same-instance/20250905T170500Z/`
2. Writes `config.json` — full loadgen params, compose scenario, and PostgreSQL conf fragments
3. Writes `loadgen-metrics.prom` — load generator latency/throughput counters
4. Queries Prometheus for the **full test window** (`query_range`) using `prometheus/queries.txt`
5. Writes `summary.json` and `manifest.json` listing all exported query files

Disable auto-export:

```bash
EXPORT_METRICS=false cargo run --release --manifest-path loadgen/Cargo.toml
```

After each run:

- **Run directory:** `results/<scenario>/<timestamp>/`
- **Config snapshot:** `results/<scenario>/<timestamp>/config.json`
- **Summary:** `results/<scenario>/<timestamp>/summary.json`
- **Prometheus series:** `results/<scenario>/<timestamp>/range-*.json`
- **Live Prometheus UI:** http://localhost:9090

Example PromQL queries are in `prometheus/queries.txt`. Useful comparisons:

```promql
# Live p95 latency from the load generator
histogram_quantile(0.95, sum by (le, operation) (rate(loadgen_live_op_duration_seconds_bucket[5m])))

# Shared-instance CPU contention (same-instance run)
sum(rate(container_cpu_usage_seconds_total{container_label_com_docker_compose_service="postgres"}[1m]))

# Separate instances — live vs dwh CPU independently
sum by (container_label_com_docker_compose_service) (rate(container_cpu_usage_seconds_total[1m]))
```

## Architecture

```mermaid
flowchart LR
  subgraph loadgen [Rust loadgen]
    LiveOps[Live OLTP workload]
    DwhOps[DWH bulk + reads]
  end

  subgraph same [Same instance compose]
    PG1[(PostgreSQL shared)]
    PE1[postgres_exporter]
  end

  subgraph separate [Separate instances compose]
    PGL[(PostgreSQL live)]
    PGD[(PostgreSQL dwh)]
    PEL[exporter live]
    PED[exporter dwh]
  end

  Prom[Prometheus]
  LiveOps --> PG1
  DwhOps --> PG1
  LiveOps --> PGL
  DwhOps --> PGD
  PE1 --> Prom
  PEL --> Prom
  PED --> Prom
  loadgen --> Prom
```

## Teardown

```bash
./scripts/teardown.sh          # both stacks
./scripts/teardown.sh same     # same-instance only
./scripts/teardown.sh separate # separate-instances only
```

## Next phase

This phase covers direct DB connectivity only. The next step is adding an API gateway in front of the live database to model a more realistic application path.
