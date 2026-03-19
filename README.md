# llmos

Rust boilerplate for an **agent** (trait + types) and a **`llmos` CLI** binary, ready to publish on [crates.io](https://crates.io/).

## Run locally

One binary, subcommands (like `git`):

```bash
cargo run -- run --input "hello"
llmos run --input "hello"
```

Read from stdin:

```bash
echo "hello" | cargo run -- run
```

Start the gRPC harness server (default: `ping` → `pong`, otherwise `echo: …`):

```bash
cargo run -- server
RUST_LOG=info cargo run -- server
LLMOS_LISTEN=0.0.0.0:50051 llmos server
```

Talk to it from another terminal:

```bash
RUST_LOG=info cargo run -- client
RUST_LOG=debug cargo run -- client --message hello
```

Logging uses `RUST_LOG` (see [`tracing-subscriber` env filter](https://docs.rs/tracing-subscriber/latest/tracing_subscriber/filter/struct.EnvFilter.html)). For the previous tool-heavy harness, set `LLMOS_FULL_HARNESS=1`.

## OpenTelemetry (logs, traces, metrics)

`llmos` exports **traces**, **metrics**, and **logs** (from `tracing`) over **OTLP/gRPC** using the usual `OTEL_*` variables ([environment spec](https://opentelemetry.io/docs/specs/otel/configuration/sdk-environment-variables/)).

- `OTEL_EXPORTER_OTLP_ENDPOINT` — gRPC endpoint (e.g. `http://127.0.0.1:4317`)
- `OTEL_SERVICE_NAME` — defaults to `llmos`
- `OTEL_SDK_DISABLED=true` — no OTLP; stdout logging only

Metrics emitted by the harness include `llmos.run_turn.total` and `llmos.run_turn.duration_ms` (see `src/telemetry/mod.rs`).

### Store telemetry on disk (this repo)

`llmos` does not write files itself; use a **local OpenTelemetry Collector** that receives OTLP and appends **JSON lines** under `otel/data/`:

```bash
# repo root — start collector
docker compose -f otel/docker-compose.yaml up -d

# point llmos at it
OTEL_EXPORTER_OTLP_ENDPOINT=http://127.0.0.1:4317 RUST_LOG=info cargo run -- server
```

Files (created after traffic):

- `otel/data/traces.json`
- `otel/data/metrics.json`
- `otel/data/logs.json`

`otel/data/` is gitignored. Config lives in `otel/collector-local.yaml`. For a binary install of [otelcol-contrib](https://github.com/open-telemetry/opentelemetry-collector-releases), copy that YAML and change `/data/...` paths to a writable directory on your machine.

## Library

Use the `Agent` trait and `EchoAgent` from the `llmos` crate, or swap in your own `Agent` implementation.

## Publish to crates.io

1. Create an account on [crates.io](https://crates.io/) and an API token.
2. Run `cargo login` and paste the token.
3. Ensure the package name `llmos` is available (or change `name` in `Cargo.toml`).
4. `cargo publish --dry-run` then `cargo publish`.

After the first publish, docs appear on [docs.rs](https://docs.rs/llmos) automatically.

## License

MIT. See [LICENSE](LICENSE).
