//! Initialize `tracing` + OpenTelemetry (OTLP) when configured.

use llmos::telemetry::{init_stdout_only, init_subscriber, OtelHandle};

/// Call once at process start. Keeps [`OtelHandle`] alive for flush on exit when OTLP is enabled.
pub fn init() -> Option<OtelHandle> {
    match init_subscriber() {
        Ok(h) => h,
        Err(e) => {
            eprintln!("llmos: OpenTelemetry init failed ({e}); logging to stdout only (see OTEL_* env).");
            let _ = init_stdout_only();
            None
        }
    }
}
