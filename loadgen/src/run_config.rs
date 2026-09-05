use crate::config::Config;
use crate::export;
use anyhow::Context;
use chrono::{DateTime, Utc};
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize)]
pub struct RunConfigDocument {
    pub captured_at: DateTime<Utc>,
    pub compose: ComposeConfig,
    pub loadgen: LoadgenConfig,
    pub postgres: PostgresConfigSnapshot,
}

#[derive(Debug, Serialize)]
pub struct ComposeConfig {
    pub scenario: String,
    pub compose_file: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct LoadgenConfig {
    pub scenario: String,
    pub live_database_url: String,
    pub dwh_database_url: String,
    pub live_target_rps: u64,
    pub test_duration_secs: f64,
    pub live_workers: usize,
    pub live_pool_size: u32,
    pub dwh_read_workers: usize,
    pub dwh_bulk_batch_size: i64,
    pub dwh_bulk_interval_secs: f64,
    pub dwh_pool_size: u32,
    pub metrics_port: u16,
    pub prometheus_url: String,
    pub export_metrics: bool,
    pub db_connect_timeout_secs: f64,
    pub results_dir: String,
}

#[derive(Debug, Serialize)]
pub struct PostgresConfigSnapshot {
    pub fragments: BTreeMap<String, String>,
    pub source_paths: BTreeMap<String, String>,
}

pub fn write_run_config(
    config: &Config,
    run_dir: &Path,
    captured_at: DateTime<Utc>,
) -> anyhow::Result<PathBuf> {
    std::fs::create_dir_all(run_dir)?;

    let compose_scenario = config
        .compose_scenario
        .clone()
        .unwrap_or_else(|| infer_compose_scenario(&config.scenario));

    let postgres = load_postgres_config_snapshot(&compose_scenario)?;

    let document = RunConfigDocument {
        captured_at,
        compose: ComposeConfig {
            scenario: compose_scenario,
            compose_file: config.compose_file.clone(),
        },
        loadgen: LoadgenConfig {
            scenario: config.scenario.clone(),
            live_database_url: redact_password(&config.live_database_url),
            dwh_database_url: redact_password(&config.dwh_database_url),
            live_target_rps: config.live_target_rps,
            test_duration_secs: config.test_duration.as_secs_f64(),
            live_workers: config.live_workers,
            live_pool_size: config.live_pool_size,
            dwh_read_workers: config.dwh_read_workers,
            dwh_bulk_batch_size: config.dwh_bulk_batch_size,
            dwh_bulk_interval_secs: config.dwh_bulk_interval.as_secs_f64(),
            dwh_pool_size: config.dwh_pool_size,
            metrics_port: config.metrics_port,
            prometheus_url: config.prometheus_url.clone(),
            export_metrics: config.export_metrics,
            db_connect_timeout_secs: config.db_connect_timeout.as_secs_f64(),
            results_dir: config.results_dir.display().to_string(),
        },
        postgres,
    };

    let output_path = run_dir.join("config.json");
    let payload = serde_json::to_string_pretty(&document)?;
    std::fs::write(&output_path, payload)?;
    Ok(output_path)
}

fn infer_compose_scenario(scenario: &str) -> String {
    match scenario {
        "same_instance" => "same_instance".to_string(),
        "separate_instances" => "separate_instances".to_string(),
        other => other.to_string(),
    }
}

fn postgres_fragment_names(compose_scenario: &str) -> &'static [&'static str] {
    match compose_scenario {
        "separate_instances" => &["common.conf", "live.conf", "dwh.conf"],
        _ => &["common.conf", "same-instance.conf"],
    }
}

fn load_postgres_config_snapshot(compose_scenario: &str) -> anyhow::Result<PostgresConfigSnapshot> {
    let repo_root = export::locate_repo_root()?;
    let postgres_dir = repo_root.join("docker/postgres");
    let mut fragments = BTreeMap::new();
    let mut source_paths = BTreeMap::new();

    for name in postgres_fragment_names(compose_scenario) {
        let path = postgres_dir.join(name);
        let content = std::fs::read_to_string(&path).with_context(|| {
            format!("read postgres config fragment {}", path.display())
        })?;
        fragments.insert(name.to_string(), content);
        source_paths.insert(
            name.to_string(),
            path.strip_prefix(&repo_root)
                .unwrap_or(&path)
                .display()
                .to_string(),
        );
    }

    Ok(PostgresConfigSnapshot {
        fragments,
        source_paths,
    })
}

fn redact_password(url: &str) -> String {
    url.split('@')
        .nth(1)
        .map(|host_part| format!("postgres://***:***@{host_part}"))
        .unwrap_or_else(|| "postgres://***:***".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn postgres_fragments_for_same_instance() {
        assert_eq!(
            postgres_fragment_names("same_instance"),
            ["common.conf", "same-instance.conf"]
        );
    }

    #[test]
    fn postgres_fragments_for_separate_instances() {
        assert_eq!(
            postgres_fragment_names("separate_instances"),
            ["common.conf", "live.conf", "dwh.conf"]
        );
    }
}
