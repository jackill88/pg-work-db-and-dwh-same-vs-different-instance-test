use crate::metrics::Metrics;
use rand::Rng;
use sqlx::PgPool;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use tokio::time::{self, Duration};

const ANALYTICAL_QUERIES: [&str; 4] = [
    "SELECT region, event_type, COUNT(*), SUM(amount) FROM fact_events WHERE event_time > now() - interval '24 hours' GROUP BY region, event_type ORDER BY COUNT(*) DESC LIMIT 20",
    "SELECT d.segment, COUNT(*), AVG(f.amount) FROM fact_events f JOIN dim_accounts d ON d.account_id = f.account_id WHERE f.event_time > now() - interval '7 days' GROUP BY d.segment ORDER BY COUNT(*) DESC",
    "SELECT account_id, COUNT(*) AS events, SUM(amount) FROM fact_events WHERE event_time > now() - interval '30 days' GROUP BY account_id HAVING COUNT(*) > 5 ORDER BY events DESC LIMIT 50",
    "SELECT date_trunc('hour', event_time) AS bucket, COUNT(*) FROM fact_events WHERE event_time > now() - interval '48 hours' GROUP BY bucket ORDER BY bucket",
];

pub async fn run_bulk_loader(
    pool: PgPool,
    metrics: Arc<Metrics>,
    batch_size: i64,
    interval: Duration,
    stop: Arc<AtomicBool>,
) {
    let mut ticker = time::interval(interval);
    ticker.set_missed_tick_behavior(time::MissedTickBehavior::Delay);

    while !stop.load(Ordering::Relaxed) {
        ticker.tick().await;

        let started = Instant::now();
        let result = sqlx::query(
            r#"
            INSERT INTO fact_events (event_time, account_id, event_type, amount, region, payload)
            SELECT
                now() - (g || ' seconds')::interval,
                (random() * 99999 + 1)::bigint,
                CASE (g % 6)
                    WHEN 0 THEN 'purchase'
                    WHEN 1 THEN 'refund'
                    WHEN 2 THEN 'login'
                    WHEN 3 THEN 'page_view'
                    WHEN 4 THEN 'subscription'
                    ELSE 'support_ticket'
                END,
                (random() * 1000)::numeric(12, 2),
                CASE (g % 5)
                    WHEN 0 THEN 'US'
                    WHEN 1 THEN 'EU'
                    WHEN 2 THEN 'APAC'
                    WHEN 3 THEN 'LATAM'
                    ELSE 'MEA'
                END,
                jsonb_build_object('source', 'bulk_load', 'seq', g)
            FROM generate_series(1, $1) AS g
            "#,
        )
        .bind(batch_size)
        .execute(&pool)
        .await;

        let elapsed = started.elapsed().as_secs_f64();
        metrics
            .dwh_op_duration_seconds
            .with_label_values(&["bulk_insert"])
            .observe(elapsed);

        match result {
            Ok(_) => metrics
                .dwh_ops_total
                .with_label_values(&["bulk_insert", "ok"])
                .inc(),
            Err(error) => {
                tracing::warn!(?error, "dwh bulk insert failed");
                metrics
                    .dwh_ops_total
                    .with_label_values(&["bulk_insert", "error"])
                    .inc();
            }
        }
    }
}

pub async fn run_read_workers(
    pool: PgPool,
    metrics: Arc<Metrics>,
    workers: usize,
    stop: Arc<AtomicBool>,
) {
    let mut handles = Vec::with_capacity(workers);

    for worker_id in 0..workers {
        let pool = pool.clone();
        let metrics = metrics.clone();
        let stop = stop.clone();

        handles.push(tokio::spawn(async move {
            while !stop.load(Ordering::Relaxed) {
                let (query, sleep_ms) = {
                    let mut rng = rand::thread_rng();
                    (
                        ANALYTICAL_QUERIES[rng.gen_range(0..ANALYTICAL_QUERIES.len())],
                        rng.gen_range(100..500),
                    )
                };
                let started = Instant::now();
                let result = sqlx::query(query).fetch_all(&pool).await;
                let elapsed = started.elapsed().as_secs_f64();

                metrics
                    .dwh_op_duration_seconds
                    .with_label_values(&["analytical_read"])
                    .observe(elapsed);

                match result {
                    Ok(_) => metrics
                        .dwh_ops_total
                        .with_label_values(&["analytical_read", "ok"])
                        .inc(),
                    Err(error) => {
                        tracing::debug!(worker_id, ?error, "dwh read failed");
                        metrics
                            .dwh_ops_total
                            .with_label_values(&["analytical_read", "error"])
                            .inc();
                    }
                }

                time::sleep(Duration::from_millis(sleep_ms)).await;
            }
        }));
    }

    for handle in handles {
        let _ = handle.await;
    }
}
