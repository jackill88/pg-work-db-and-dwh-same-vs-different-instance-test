use chrono::{DateTime, Utc};
use serde::Serialize;
use std::path::Path;

#[derive(Debug, Serialize)]
pub struct RunSummary {
    pub scenario: String,
    pub started_at: DateTime<Utc>,
    pub finished_at: DateTime<Utc>,
    pub duration_seconds: f64,
    pub live_target_rps: u64,
    pub live_workers: usize,
    pub dwh_read_workers: usize,
    pub dwh_bulk_batch_size: i64,
    pub live_database_url: String,
    pub dwh_database_url: String,
    pub prometheus_url: String,
    pub loadgen_metrics_url: String,
    pub export_dir: String,
    pub exported_files: Vec<String>,
    pub notes: Vec<String>,
}

impl RunSummary {
    pub fn write_json(&self, path: &Path) -> anyhow::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let payload = serde_json::to_string_pretty(self)?;
        std::fs::write(path, payload)?;
        Ok(())
    }
}
