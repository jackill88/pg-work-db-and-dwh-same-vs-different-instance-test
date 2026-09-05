use crate::config::Config;
use crate::metrics::Metrics;
use anyhow::Context;
use chrono::{DateTime, Utc};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;
use tracing::{info, warn};

pub fn run_directory(results_dir: &Path, started_at: DateTime<Utc>) -> PathBuf {
    results_dir.join(started_at.format("%Y%m%dT%H%M%SZ").to_string())
}

pub fn write_loadgen_metrics(metrics: &Metrics, path: &Path) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, metrics.gather())?;
    Ok(())
}

pub fn export_prometheus(
    config: &Config,
    run_dir: &Path,
    started_at: DateTime<Utc>,
    finished_at: DateTime<Utc>,
    duration: Duration,
) -> anyhow::Result<PathBuf> {
    let repo_root = locate_repo_root()?;
    let script = repo_root.join("scripts/export-prometheus.sh");

    if !script.is_file() {
        anyhow::bail!("export script not found at {}", script.display());
    }

    let step = export_step(duration);
    let start_unix = started_at.timestamp().to_string();
    let end_unix = finished_at.timestamp().to_string();

    info!(
        run_dir = %run_dir.display(),
        step,
        "exporting Prometheus metrics for benchmark window"
    );

    let status = Command::new("bash")
        .arg(&script)
        .arg(&config.scenario)
        .env("RESULTS_DIR", run_dir)
        .env("PROMETHEUS_URL", &config.prometheus_url)
        .env("EXPORT_START_UNIX", start_unix)
        .env("EXPORT_END_UNIX", end_unix)
        .env("EXPORT_STEP", step)
        .status()
        .with_context(|| format!("run {}", script.display()))?;

    if !status.success() {
        anyhow::bail!("Prometheus export script failed with status {status}");
    }

    Ok(run_dir.to_path_buf())
}

fn export_step(duration: Duration) -> String {
    let seconds = duration.as_secs().max(1);
    let step = (seconds / 120).clamp(1, 15);
    format!("{step}s")
}

fn locate_repo_root() -> anyhow::Result<PathBuf> {
    if let Ok(manifest) = std::env::var("CARGO_MANIFEST_DIR") {
        let loadgen_dir = PathBuf::from(manifest);
        if let Some(root) = loadgen_dir.parent() {
            return Ok(root.to_path_buf());
        }
    }

    warn!("CARGO_MANIFEST_DIR unavailable; assuming current directory is repo root");
    Ok(std::env::current_dir()?)
}
