//! OpenTelemetry: OTLP **traces**, **metrics**, and **logs** (logs via `tracing` → OTLP).
//!
//! Standard OTel env vars apply, e.g. `OTEL_EXPORTER_OTLP_ENDPOINT`, `OTEL_SERVICE_NAME`,
//! `OTEL_SDK_DISABLED=true`. Use `RUST_LOG` for level filters.

use std::sync::OnceLock;
use std::time::Duration;

use opentelemetry::global;
use opentelemetry::metrics::{Counter, Histogram};
use opentelemetry::trace::TracerProvider;
use opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge;
use opentelemetry_otlp::{LogExporter, MetricExporter, SpanExporter};
use opentelemetry_sdk::logs::SdkLoggerProvider;
use opentelemetry_sdk::metrics::{PeriodicReader, SdkMeterProvider};
use opentelemetry_sdk::propagation::TraceContextPropagator;
use opentelemetry_sdk::trace::SdkTracerProvider;
use opentelemetry_sdk::Resource;
use tracing_subscriber::prelude::*;
use tracing_subscriber::{EnvFilter, Registry};

/// Shutdown guard (flushes OTLP).
#[derive(Debug)]
pub struct OtelHandle {
    tracer: SdkTracerProvider,
    meter: SdkMeterProvider,
    logger: SdkLoggerProvider,
}

impl Drop for OtelHandle {
    fn drop(&mut self) {
        let _ = self.logger.shutdown();
        let _ = self.meter.shutdown();
        let _ = self.tracer.shutdown();
    }
}

fn sdk_disabled() -> bool {
    std::env::var("OTEL_SDK_DISABLED")
        .map(|v| {
            let v = v.trim();
            matches!(v, "1" | "true" | "yes")
        })
        .unwrap_or(false)
}

fn resource() -> Resource {
    let name = std::env::var("OTEL_SERVICE_NAME").unwrap_or_else(|_| "llmos".into());
    Resource::builder().with_service_name(name).build()
}

/// Stdout-only `tracing` (no OTLP). Used when OTel is disabled or failed to init.
pub fn init_stdout_only() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let _ = Registry::default()
        .with(filter)
        .with(tracing_subscriber::fmt::layer().with_target(true))
        .try_init();
    Ok(())
}

/// Install OTLP for traces, metrics, and logs. Returns [`OtelHandle`] for flush on exit.
pub fn init_subscriber() -> Result<Option<OtelHandle>, Box<dyn std::error::Error + Send + Sync>> {
    if sdk_disabled() {
        init_stdout_only()?;
        return Ok(None);
    }

    let res = resource();
    let span_exporter = SpanExporter::builder().with_tonic().build()?;
    let metric_exporter = MetricExporter::builder().with_tonic().build()?;
    let log_exporter = LogExporter::builder().with_tonic().build()?;

    global::set_text_map_propagator(TraceContextPropagator::new());

    let tracer_provider = SdkTracerProvider::builder()
        .with_batch_exporter(span_exporter)
        .with_resource(res.clone())
        .build();

    let reader = PeriodicReader::builder(metric_exporter)
        .with_interval(Duration::from_secs(15))
        .build();
    let meter_provider = SdkMeterProvider::builder()
        .with_reader(reader)
        .with_resource(res.clone())
        .build();

    let logger_provider = SdkLoggerProvider::builder()
        .with_batch_exporter(log_exporter)
        .with_resource(res)
        .build();

    let _ = global::set_tracer_provider(tracer_provider.clone());
    let _ = global::set_meter_provider(meter_provider.clone());

    let tracer = tracer_provider.tracer("llmos");
    let trace_layer = tracing_opentelemetry::layer().with_tracer(tracer);
    let logs_layer = OpenTelemetryTracingBridge::new(&logger_provider);

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let fmt_layer = tracing_subscriber::fmt::layer().with_target(true);

    let _ = Registry::default()
        .with(filter)
        .with(trace_layer)
        .with(logs_layer)
        .with(fmt_layer)
        .try_init();

    init_metrics();

    Ok(Some(OtelHandle {
        tracer: tracer_provider,
        meter: meter_provider,
        logger: logger_provider,
    }))
}

/// OTLP metrics for the harness RPC path.
pub struct HarnessMetrics {
    pub run_turn_total: Counter<u64>,
    pub run_turn_duration_ms: Histogram<f64>,
}

static HARNESS_METRICS: OnceLock<HarnessMetrics> = OnceLock::new();

fn init_metrics() {
    let _ = HARNESS_METRICS.get_or_init(|| {
        let m = global::meter("llmos");
        HarnessMetrics {
            run_turn_total: m
                .u64_counter("llmos.run_turn.total")
                .with_description("Harness RunTurn RPC completed")
                .build(),
            run_turn_duration_ms: m
                .f64_histogram("llmos.run_turn.duration_ms")
                .with_description("RunTurn processing time (ms)")
                .build(),
        }
    });
}

pub fn harness_metrics() -> &'static HarnessMetrics {
    HARNESS_METRICS.get_or_init(|| {
        let m = global::meter("llmos");
        HarnessMetrics {
            run_turn_total: m.u64_counter("llmos.run_turn.total").build(),
            run_turn_duration_ms: m.f64_histogram("llmos.run_turn.duration_ms").build(),
        }
    })
}
