mod config;
mod db;
mod dwh;
mod export;
mod live;
mod metrics;
mod run_config;
mod stats;

use anyhow::Context;
use chrono::Utc;
use clap::Parser;
use config::Config;
use metrics::{Metrics, serve};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use tokio::time;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("pg_bench_loadgen=info".parse()?))
        .init();

    let config = Config::parse();
    if std::env::var_os("LIVE_WORKERS").is_some() || std::env::var_os("LIVE_POOL_SIZE").is_some() {
        tracing::info!(
            live_workers = config.live_workers,
            live_pool_size = config.live_pool_size,
            "using LIVE_WORKERS / LIVE_POOL_SIZE from environment"
        );
    }
    tracing::info!(scenario = %config.scenario, live_target_rps = config.live_target_rps, "starting benchmark");

    let live_pool = db::connect_with_retry(
        &config.live_database_url,
        config.live_pool_size,
        "live database",
        config.db_connect_timeout,
    )
    .await?;

    let dwh_pool = db::connect_with_retry(
        &config.dwh_database_url,
        config.dwh_pool_size,
        "dwh database",
        config.db_connect_timeout,
    )
    .await?;

    sqlx::query("SELECT 1")
        .fetch_one(&live_pool)
        .await
        .context("live database health check failed")?;
    sqlx::query("SELECT 1")
        .fetch_one(&dwh_pool)
        .await
        .context("dwh database health check failed")?;

    let metrics = Arc::new(Metrics::new(&config.scenario));
    let stop = Arc::new(AtomicBool::new(false));

    let metrics_server = {
        let metrics = metrics.clone();
        tokio::spawn(async move {
            serve(metrics, config.metrics_port).await;
        })
    };

    let live_task = {
        let pool = live_pool.clone();
        let metrics = metrics.clone();
        let stop = stop.clone();
        let workers = config.live_workers;
        let target_rps = config.live_target_rps;
        tokio::spawn(async move {
            live::run_live_pool(pool, metrics, workers, target_rps, stop).await;
        })
    };

    let bulk_task = {
        let pool = dwh_pool.clone();
        let metrics = metrics.clone();
        let stop = stop.clone();
        let batch_size = config.dwh_bulk_batch_size;
        let interval = config.dwh_bulk_interval;
        tokio::spawn(async move {
            dwh::run_bulk_loader(pool, metrics, batch_size, interval, stop).await;
        })
    };

    let dwh_reads_task = {
        let pool = dwh_pool.clone();
        let metrics = metrics.clone();
        let stop = stop.clone();
        let workers = config.dwh_read_workers;
        tokio::spawn(async move {
            dwh::run_read_workers(pool, metrics, workers, stop).await;
        })
    };

    let started_at = Utc::now();
    let run_dir = export::run_directory(&config.results_dir, started_at);
    let config_path = run_config::write_run_config(&config, &run_dir, started_at)?;
    tracing::info!(path = %config_path.display(), "wrote run config");

    let wall_start = Instant::now();
    tracing::info!(
        duration = ?config.test_duration,
        live_target_rps = config.live_target_rps,
        dwh_read_workers = config.dwh_read_workers,
        "benchmark running"
    );

    time::sleep(config.test_duration).await;
    stop.store(true, Ordering::Relaxed);

    live_task.await.context("live workload task failed")?;
    bulk_task.await.context("dwh bulk task failed")?;
    dwh_reads_task.await.context("dwh read task failed")?;
    metrics_server.abort();

    let finished_at = Utc::now();
    let duration_seconds = wall_start.elapsed().as_secs_f64();

    let loadgen_metrics_path = run_dir.join("loadgen-metrics.prom");
    export::write_loadgen_metrics(&metrics, &loadgen_metrics_path)?;
    tracing::info!(path = %loadgen_metrics_path.display(), "wrote loadgen metrics");

    let mut exported_files = vec![
        config_path.display().to_string(),
        loadgen_metrics_path.display().to_string(),
    ];

    if config.export_metrics {
        match export::export_prometheus(
            &config,
            &run_dir,
            started_at,
            finished_at,
            config.test_duration,
        ) {
            Ok(export_dir) => {
                tracing::info!(path = %export_dir.display(), "exported Prometheus metrics");
                exported_files.push(export_dir.join("manifest.json").display().to_string());
            }
            Err(error) => {
                tracing::warn!(?error, "Prometheus export failed; loadgen metrics were still saved");
            }
        }
    }

    let summary = stats::RunSummary {
        scenario: config.scenario.clone(),
        started_at,
        finished_at,
        duration_seconds,
        live_target_rps: config.live_target_rps,
        live_workers: config.live_workers,
        dwh_read_workers: config.dwh_read_workers,
        dwh_bulk_batch_size: config.dwh_bulk_batch_size,
        live_database_url: redact_password(&config.live_database_url),
        dwh_database_url: redact_password(&config.dwh_database_url),
        prometheus_url: config.prometheus_url.clone(),
        loadgen_metrics_url: format!("http://localhost:{}/metrics", config.metrics_port),
        export_dir: run_dir.display().to_string(),
        exported_files,
        notes: vec![
            "Each run is stored under results/<scenario>/<timestamp>/.".to_string(),
            "Prometheus range queries cover the full benchmark window.".to_string(),
        ],
    };

    let output_path = run_dir.join("summary.json");
    summary.write_json(&output_path)?;
    tracing::info!(path = %output_path.display(), "wrote run summary");

    live_pool.close().await;
    dwh_pool.close().await;

    Ok(())
}

fn redact_password(url: &str) -> String {
    url.split('@')
        .nth(1)
        .map(|host_part| format!("postgres://***:***@{host_part}"))
        .unwrap_or_else(|| "postgres://***:***".to_string())
}
