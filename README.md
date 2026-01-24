# GCP Rust Tools

[![Crates.io](https://img.shields.io/crates/v/gcp-rust-tools.svg)](https://crates.io/crates/gcp-rust-tools)
[![Documentation](https://docs.rs/gcp-rust-tools/badge.svg)](https://docs.rs/gcp-rust-tools)

A comprehensive Rust toolset for Google Cloud Platform, combining simplified Observability (Logs, Metrics, Traces) with robust Pub/Sub wrappers.

## Builder notes

This crate was developed by [Santiago Amoretti](https://github.com/santiamoretti) in the context of the development of Genevabm(https://genevabm.com).

## Features

- **🚀 Pub/Sub Integration**: Easy wrapper around official Google Cloud Pub/Sub crates.
- **📝 Cloud Logging**: Send structured logs to Google Cloud Logging.
- **📊 Cloud Monitoring**: Create custom metrics in Google Cloud Monitoring.
- **🔍 Cloud Trace**: Create distributed traces in Google Cloud Trace.
- **⚡ High Performance**: Designed for efficiency.
- **🛡️ Error Resilient**: Automatic retry logic.

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
gcp-rust-tools = "0.2.4"
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

## Prerequisites

1. **Google Cloud Project** with APIs enabled:
   - Cloud Logging API
   - Cloud Monitoring API
   - Cloud Trace API
    - Pub/Sub API

2. **Service Account JSON** with roles:
   - `roles/logging.logWriter`
   - `roles/monitoring.metricWriter`
   - `roles/cloudtrace.agent`
    - `roles/pubsub.publisher` (for publishing)
    - `roles/pubsub.subscriber` (for pulling/streaming subscriptions)

3. **gcloud CLI** (automatically installed if missing)

## Architecture

The library uses a channel-based architecture for optimal performance:

```
┌─────────────┐         ┌─────────┐         ┌──────────────┐
│ Your App    │────────▶│ Channel │────────▶│ Worker Thread│
│ (main)      │ queue   │ (1027)  │ process │ (async ops)  │
└─────────────┘         └─────────┘         └──────────────┘
                                                    │
                                                    ▼
                                            ┌──────────────┐
                                            │  GCP APIs    │
                                            └──────────────┘
```

## Quick Start

### Fire-and-Forget (Recommended)

```rust
use gcp_rust_tools::{ObservabilityClient, LogEntry, MetricData, TraceSpan};
use std::collections::HashMap;
use std::time::{SystemTime, Duration};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize client (performs authentication)
    // Credentials are resolved internally from GOOGLE_APPLICATION_CREDENTIALS.
    // Project id can be provided, or inferred via GOOGLE_CLOUD_PROJECT / gcloud.
    let client = ObservabilityClient::new(
        Some("your-project-id".to_string()),
        None,
    ).await?;

    // Fire-and-forget logging (returns immediately)
    client.send_log(LogEntry::new("INFO", "App started"))?;
    
    // With service name
    client.send_log(
        LogEntry::new("ERROR", "DB connection failed")
            .with_service_name("api-server")
    )?;

    // Send metrics with labels
    let mut labels = HashMap::new();
    labels.insert("environment".to_string(), "production".to_string());
    
    client.send_metric(
        MetricData::new(
            "custom.googleapis.com/requests_total",
            42.0,
            "INT64",
            "GAUGE"
        ).with_labels(labels)
    )?;

    // Create distributed traces
    let trace_id = ObservabilityClient::generate_trace_id();
    let span_id = ObservabilityClient::generate_span_id();
    
    client.send_trace(
        TraceSpan::new(
            trace_id,
            span_id,
            "HTTP Request",
            SystemTime::now(),
            Duration::from_millis(150)
        )
    )?;

    Ok(())
}
```

### Async (Wait for Completion)

When you need confirmation that an operation succeeded:

```rust
// Wait for operation to complete
client.send_log_async(LogEntry::new("INFO", "Critical log")).await?;
client.send_metric_async(MetricData::new("metric", 1.0, "INT64", "GAUGE")).await?;
client.send_trace_async(TraceSpan::new(...)).await?;
```

### Using Convenience Macros

```rust
use gcp_rust_tools::{gcp_info, gcp_warn, gcp_error};

gcp_info!(client, "User {} logged in", user_id)?;
gcp_warn!(client, "High memory usage: {}%", usage)?;
gcp_error!(client, "Failed to process: {}", error)?;
```

## API Reference

### ObservabilityClient

#### Initialization
- `new(project_id, service_account_path)` → `Result<Self, ObservabilityError>`
  - Creates and authenticates a new client
  - Starts background worker thread

#### Fire-and-Forget Methods
- `send_log(log_entry: LogEntry)` → `Result<(), ObservabilityError>`
- `send_metric(metric_data: MetricData)` → `Result<(), ObservabilityError>`
- `send_trace(trace_span: TraceSpan)` → `Result<(), ObservabilityError>`

#### Async Methods (Wait for Completion)
- `send_log_async(log_entry: LogEntry)` → `Future<Result<(), ObservabilityError>>`
- `send_metric_async(metric_data: MetricData)` → `Future<Result<(), ObservabilityError>>`
- `send_trace_async(trace_span: TraceSpan)` → `Future<Result<(), ObservabilityError>>`

#### Utility Methods
- `generate_trace_id()` → `String` - Generate a 32-character hex trace ID
- `generate_span_id()` → `String` - Generate a 16-character hex span ID

### Data Structures

#### LogEntry
```rust
LogEntry::new(severity: impl Into<String>, message: impl Into<String>)
    .with_service_name(name: impl Into<String>)
```

#### MetricData
```rust
MetricData::new(
    metric_type: impl Into<String>,
    value: f64,
    value_type: impl Into<String>,  // "INT64" | "DOUBLE"
    metric_kind: impl Into<String>  // "GAUGE" | "CUMULATIVE"
)
    .with_labels(labels: HashMap<String, String>)
```

#### TraceSpan
```rust
TraceSpan::new(
    trace_id: impl Into<String>,
    span_id: impl Into<String>,
    display_name: impl Into<String>,
    start_time: SystemTime,
    duration: Duration
)
    .with_parent_span_id(parent_span_id: impl Into<String>)
```

### Convenience Macros

- `gcp_info!(client, "message")` - Send an INFO log (fire-and-forget)
- `gcp_warn!(client, "message")` - Send a WARNING log (fire-and-forget)
- `gcp_error!(client, "message")` - Send an ERROR log (fire-and-forget)
- `gcp_log!(client, "LEVEL", "message")` - Send a log with custom severity

## Error Handling

The library provides comprehensive error handling:

### Error Types

- `AuthenticationError` - Failed to authenticate with gcloud
- `ApiError` - Google Cloud API request failed
- `SetupError` - Failed to setup/install gcloud CLI
- `Shutdown` - Special internal error for worker shutdown

### Token Expiration

The library automatically handles token expiration:

1. Detects expired tokens (401/403 HTTP responses)
2. Re-authenticates using your service account
3. Retries the failed operation with a fresh token
4. All happens transparently in the background

### Silent Failures

Background operations fail silently to avoid disrupting your application. If you need error feedback, use the async methods:

```rust
match client.send_log_async(LogEntry::new("INFO", "Important")).await {
    Ok(()) => println!("Log sent successfully"),
    Err(e) => eprintln!("Failed to send log: {}", e),
}
```

## Pub/Sub

This crate includes a small convenience wrapper over the official `google-cloud-pubsub` client.

### Naming conventions

- Topics passed in `topics` are expanded to: `projects/{project_id}/topics/{name}-{instance_id}`
- Subscriptions passed in `subs` are expanded to: `projects/{project_id}/subscriptions/{name}`

### Publish (fire-and-forget)

```rust
use gcp_rust_tools::pubsub::create_pubsub_client;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let topics: Arc<[&'static str]> = Arc::from(["events"]);
    let subs: Arc<[&'static str]> = Arc::from(["events-sub"]);

    // Credentials are resolved from GOOGLE_APPLICATION_CREDENTIALS.
    // Project id is resolved from (in order): provided value, GOOGLE_CLOUD_PROJECT,
    // or `gcloud config get-value project`.
    let pubsub = create_pubsub_client(None, "dev", topics, subs).await?;

    pubsub
        .publish_fire_and_forget(
            "events",
            serde_json::json!({"hello": "world"}),
            None,
        )
        .await;

    Ok(())
}
```

Notes:

- `publish_fire_and_forget` intentionally does not surface publish errors; it spawns a task and logs failures via `log`.
- Subscriptions are currently treated primarily as *lookups* (and may need to exist already in GCP with the correct topic binding).

## Performance

### Characteristics

- **Non-blocking**: Fire-and-forget operations return immediately
- **Bounded Channel**: 1027-item buffer prevents memory overflow  
- **Single Worker**: One background thread prevents API rate limiting
- **No Synchronization Overhead**: Minimal locking and contention
- **Fast Compilation**: No heavy protobuf or gRPC dependencies

### Benchmarks

On a typical development machine:
- Fire-and-forget operation: < 1µs
- Background processing: ~50-200ms per operation (network dependent)
- Channel capacity: 1027 items before blocking

## Features

```toml
[dependencies]
gcp-rust-tools = { version = "0.2.4", features = ["logging", "monitoring"] }
```

Available features:
- `logging` - Cloud Logging functionality
- `monitoring` - Cloud Monitoring functionality  
- `tracing` - Cloud Trace functionality
- `default` - Includes all features

## Examples

Run the example:

```bash
# Set your project ID and service account path
export GCP_PROJECT_ID="your-project-id"
export GCP_SERVICE_ACCOUNT="/path/to/service-account.json"

cargo run --example basic_usage
```

## Architecture

This library uses a unique approach that balances simplicity with performance:

- **Lightweight**: No heavy protobuf or gRPC dependencies
- **Simple**: Uses standard HTTP/REST APIs via curl
- **Reliable**: Leverages battle-tested gcloud CLI for authentication
- **Fast**: Minimal overhead and fast compilation times
- **Resilient**: Automatic token refresh and retry logic

### Background Worker

The single-threaded worker model provides natural rate limiting:
- One thread processes all operations sequentially
- Prevents overwhelming GCP APIs
- No complex rate limiting logic needed
- Predictable memory usage

## Troubleshooting

### Authentication Issues

```bash
# Verify gcloud is installed
gcloud version

# Verify service account works
gcloud auth activate-service-account --key-file=/path/to/key.json
gcloud auth list
```

### API Not Enabled

Enable required APIs in your GCP project:
```bash
gcloud services enable logging.googleapis.com
gcloud services enable monitoring.googleapis.com
gcloud services enable cloudtrace.googleapis.com
```

### Permission Issues

Ensure your service account has the required roles:
- `roles/logging.logWriter`
- `roles/monitoring.metricWriter`
- `roles/cloudtrace.agent`

## License

Licensed under the Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0).

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.