use crate::metrics::Metrics;
use rand::Rng;
use sqlx::PgPool;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use tokio::time::{self, Duration, MissedTickBehavior};

#[derive(Debug, Clone, Copy)]
enum LiveOperation {
    ReadAccount,
    ReadRecentOrders,
    CreateOrder,
    UpdateBalance,
}

impl LiveOperation {
    fn name(self) -> &'static str {
        match self {
            Self::ReadAccount => "read_account",
            Self::ReadRecentOrders => "read_recent_orders",
            Self::CreateOrder => "create_order",
            Self::UpdateBalance => "update_balance",
        }
    }

    fn pick(rng: &mut impl Rng) -> Self {
        match rng.gen_range(0..100) {
            0..=39 => Self::ReadAccount,
            40..=69 => Self::ReadRecentOrders,
            70..=89 => Self::CreateOrder,
            _ => Self::UpdateBalance,
        }
    }
}

pub async fn run_live_pool(
    pool: PgPool,
    metrics: Arc<Metrics>,
    workers: usize,
    target_rps: u64,
    stop: Arc<AtomicBool>,
) {
    let workers = workers.max(1);
    let tick_interval =
        Duration::from_nanos(1_000_000_000 * workers as u64 / target_rps.max(1));

    let mut handles = Vec::with_capacity(workers);
    for worker_id in 0..workers {
        let pool = pool.clone();
        let metrics = metrics.clone();
        let stop = stop.clone();

        handles.push(tokio::spawn(async move {
            let mut interval = time::interval(tick_interval);
            interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

            while !stop.load(Ordering::Relaxed) {
                interval.tick().await;
                execute_live_op(&pool, &metrics, worker_id).await;
            }
        }));
    }

    for handle in handles {
        let _ = handle.await;
    }
}

async fn execute_live_op(pool: &PgPool, metrics: &Metrics, worker_id: usize) {
    let (operation, account_id, amount, delta) = {
        let mut rng = rand::thread_rng();
        (
            LiveOperation::pick(&mut rng),
            rng.gen_range(1..=10_000),
            rng.gen_range(1.0..500.0),
            rng.gen_range(-25.0..25.0),
        )
    };
    let started = Instant::now();

    let result = match operation {
        LiveOperation::ReadAccount => sqlx::query("SELECT 1 FROM accounts WHERE id = $1")
            .bind(account_id)
            .execute(pool)
            .await
            .map(|_| ()),
        LiveOperation::ReadRecentOrders => sqlx::query(
            "SELECT id, amount FROM orders WHERE created_at > now() - interval '1 hour' ORDER BY created_at DESC LIMIT 25",
        )
        .fetch_all(pool)
        .await
        .map(|_| ()),
        LiveOperation::CreateOrder => sqlx::query(
            "INSERT INTO orders (account_id, amount, status) VALUES ($1, $2, 'pending')",
        )
        .bind(account_id)
        .bind(amount)
        .execute(pool)
        .await
        .map(|_| ()),
        LiveOperation::UpdateBalance => sqlx::query(
            "UPDATE accounts SET balance = balance + $2, updated_at = now() WHERE id = $1",
        )
        .bind(account_id)
        .bind(delta)
        .execute(pool)
        .await
        .map(|_| ()),
    };

    let elapsed = started.elapsed().as_secs_f64();
    metrics
        .live_op_duration_seconds
        .with_label_values(&[operation.name()])
        .observe(elapsed);

    match result {
        Ok(()) => metrics
            .live_ops_total
            .with_label_values(&[operation.name(), "ok"])
            .inc(),
        Err(error) => {
            tracing::debug!(worker_id, ?error, operation = operation.name(), "live op failed");
            metrics
                .live_ops_total
                .with_label_values(&[operation.name(), "error"])
                .inc();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn per_worker_interval_for_1750_rps_with_32_workers() {
        let workers = 32;
        let interval = Duration::from_nanos(1_000_000_000 * workers as u64 / 1750);
        assert_eq!(interval, Duration::from_nanos(18_285_714));
    }
}
