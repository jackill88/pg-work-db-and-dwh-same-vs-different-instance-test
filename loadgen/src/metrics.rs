use prometheus::{
    Encoder, HistogramOpts, HistogramVec, IntCounterVec, Opts, Registry, TextEncoder,
};
use std::sync::Arc;
use warp::Filter;

#[derive(Clone)]
pub struct Metrics {
    registry: Registry,
    pub live_ops_total: IntCounterVec,
    pub live_op_duration_seconds: HistogramVec,
    pub dwh_ops_total: IntCounterVec,
    pub dwh_op_duration_seconds: HistogramVec,
}

impl Metrics {
    pub fn new(scenario: &str) -> Self {
        let registry = Registry::new();

        let live_ops_total = IntCounterVec::new(
            Opts::new("loadgen_live_ops_total", "Total live DB operations executed")
                .const_label("scenario", scenario),
            &["operation", "status"],
        )
        .expect("live ops counter");

        let live_op_duration_seconds = HistogramVec::new(
            HistogramOpts::new(
                "loadgen_live_op_duration_seconds",
                "Live DB operation latency in seconds",
            )
            .const_label("scenario", scenario)
            .buckets(prometheus::exponential_buckets(0.0005, 2.0, 16).expect("buckets")),
            &["operation"],
        )
        .expect("live latency histogram");

        let dwh_ops_total = IntCounterVec::new(
            Opts::new("loadgen_dwh_ops_total", "Total DWH operations executed")
                .const_label("scenario", scenario),
            &["operation", "status"],
        )
        .expect("dwh ops counter");

        let dwh_op_duration_seconds = HistogramVec::new(
            HistogramOpts::new(
                "loadgen_dwh_op_duration_seconds",
                "DWH operation latency in seconds",
            )
            .const_label("scenario", scenario)
            .buckets(prometheus::exponential_buckets(0.001, 2.0, 16).expect("buckets")),
            &["operation"],
        )
        .expect("dwh latency histogram");

        registry
            .register(Box::new(live_ops_total.clone()))
            .expect("register live ops");
        registry
            .register(Box::new(live_op_duration_seconds.clone()))
            .expect("register live latency");
        registry
            .register(Box::new(dwh_ops_total.clone()))
            .expect("register dwh ops");
        registry
            .register(Box::new(dwh_op_duration_seconds.clone()))
            .expect("register dwh latency");

        Self {
            registry,
            live_ops_total,
            live_op_duration_seconds,
            dwh_ops_total,
            dwh_op_duration_seconds,
        }
    }

    pub fn gather(&self) -> String {
        let metric_families = self.registry.gather();
        let mut buffer = Vec::new();
        TextEncoder::new()
            .encode(&metric_families, &mut buffer)
            .expect("encode metrics");
        String::from_utf8(buffer).expect("utf8 metrics")
    }
}

pub async fn serve(metrics: Arc<Metrics>, port: u16) {
    let route = warp::path("metrics").map(move || {
        warp::reply::with_header(metrics.gather(), "Content-Type", "text/plain; charset=utf-8")
    });

    tracing::info!(port, "load generator metrics endpoint ready at /metrics");
    warp::serve(route).run(([0, 0, 0, 0], port)).await;
}
