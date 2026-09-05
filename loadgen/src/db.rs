use anyhow::Context;
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use std::time::Duration;
use tracing::warn;

pub async fn connect_with_retry(
    database_url: &str,
    max_connections: u32,
    label: &str,
    timeout: Duration,
) -> anyhow::Result<PgPool> {
    let retry_interval = Duration::from_secs(2);
    let max_attempts = (timeout.as_secs() / retry_interval.as_secs()).max(1);
    let mut last_error: Option<anyhow::Error> = None;

    for attempt in 1..=max_attempts {
        match PgPoolOptions::new()
            .max_connections(max_connections)
            .connect(database_url)
            .await
        {
            Ok(pool) => match sqlx::query("SELECT 1").fetch_one(&pool).await {
                Ok(_) => {
                    if attempt > 1 {
                        tracing::info!(attempt, "{label} connection ready");
                    }
                    return Ok(pool);
                }
                Err(error) => {
                    last_error = Some(error.into());
                    pool.close().await;
                }
            },
            Err(error) => {
                last_error = Some(error.into());
            }
        }

        if attempt < max_attempts {
            warn!(
                attempt,
                max_attempts,
                "{label} not ready yet; retrying in {}s",
                retry_interval.as_secs()
            );
            tokio::time::sleep(retry_interval).await;
        }
    }

    Err(last_error.unwrap_or_else(|| anyhow::anyhow!("{label} connection failed")))
        .with_context(|| format!("{label} not reachable after {max_attempts} attempts"))
}
