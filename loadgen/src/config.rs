use clap::Parser;
use humantime::Duration;
use std::path::PathBuf;

#[derive(Debug, Clone, Parser)]
#[command(
    name = "pg-bench-loadgen",
    about = "PostgreSQL live + DWH load generator for same-instance vs separate-instance benchmarks"
)]
pub struct Config {
    /// Scenario label written into exported results (e.g. same_instance, separate_instances)
    #[arg(long, env = "SCENARIO", default_value = "unspecified")]
    pub scenario: String,

    #[arg(long, env = "LIVE_DATABASE_URL")]
    pub live_database_url: String,

    #[arg(long, env = "DWH_DATABASE_URL")]
    pub dwh_database_url: String,

    /// Target live-database operations per second across all workers
    #[arg(long, env = "LIVE_TARGET_RPS", default_value_t = 1750)]
    pub live_target_rps: u64,

    /// How long to run the benchmark
    #[arg(long, env = "TEST_DURATION", default_value = "5m")]
    #[arg(value_parser = parse_duration)]
    pub test_duration: std::time::Duration,

    /// Concurrent live DB workers
    #[arg(long, env = "LIVE_WORKERS", default_value_t = 128)]
    pub live_workers: usize,

    /// Max connections in the live DB pool
    #[arg(long, env = "LIVE_POOL_SIZE", default_value_t = 200)]
    pub live_pool_size: u32,

    /// Concurrent analytical read workers against the DWH
    #[arg(long, env = "DWH_READ_WORKERS", default_value_t = 15)]
    pub dwh_read_workers: usize,

    /// Rows inserted per bulk-load batch
    #[arg(long, env = "DWH_BULK_BATCH_SIZE", default_value_t = 5000)]
    pub dwh_bulk_batch_size: i64,

    /// Pause between bulk-load batches
    #[arg(long, env = "DWH_BULK_INTERVAL", default_value = "2s")]
    #[arg(value_parser = parse_duration)]
    pub dwh_bulk_interval: std::time::Duration,

    /// Max connections in the DWH pool
    #[arg(long, env = "DWH_POOL_SIZE", default_value_t = 32)]
    pub dwh_pool_size: u32,

    /// Port for Prometheus metrics exposed by the load generator
    #[arg(long, env = "METRICS_PORT", default_value_t = 9100)]
    pub metrics_port: u16,

    /// Directory for JSON summary output
    #[arg(long, env = "RESULTS_DIR", default_value = "results")]
    pub results_dir: PathBuf,

    /// Prometheus server used for post-run metric export
    #[arg(long, env = "PROMETHEUS_URL", default_value = "http://localhost:9090")]
    pub prometheus_url: String,

    /// Snapshot Prometheus metrics automatically when the benchmark finishes
    #[arg(long, env = "EXPORT_METRICS", default_value = "true", value_parser = parse_bool)]
    pub export_metrics: bool,

    /// How long to retry database connections before failing
    #[arg(long, env = "DB_CONNECT_TIMEOUT", default_value = "2m")]
    #[arg(value_parser = parse_duration)]
    pub db_connect_timeout: std::time::Duration,

    /// Compose scenario name (e.g. same_instance, separate_instances)
    #[arg(long, env = "COMPOSE_SCENARIO")]
    pub compose_scenario: Option<String>,

    /// Path to the docker compose file used for this run
    #[arg(long, env = "COMPOSE_FILE")]
    pub compose_file: Option<String>,
}

fn parse_bool(value: &str) -> Result<bool, String> {
    match value.to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "0" | "false" | "no" | "off" => Ok(false),
        other => Err(format!("invalid boolean value: {other}")),
    }
}

fn parse_duration(value: &str) -> Result<std::time::Duration, String> {
    value
        .parse::<Duration>()
        .map(|duration| duration.into())
        .map_err(|error| error.to_string())
}
