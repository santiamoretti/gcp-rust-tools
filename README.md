# GCP Observability for Rust

[![Crates.io](https://img.shields.io/crates/v/gcp-observability-rs.svg)](https://crates.io/crates/gcp-observability-rs)
[![Documentation](https://docs.rs/gcp-observability-rs/badge.svg)](https://docs.rs/gcp-observability-rs)

A lightweight Google Cloud Platform observability library for Rust applications. This crate provides easy-to-use APIs for Cloud Logging, Cloud Monitoring, and Cloud Trace using the gcloud CLI instead of heavy SDK dependencies.

## Features

- **🪶 Lightweight**: Uses gcloud CLI instead of heavy Google Cloud SDK dependencies
- **📝 Cloud Logging**: Send structured logs to Google Cloud Logging
- **📊 Cloud Monitoring**: Create custom metrics in Google Cloud Monitoring  
- **🔍 Cloud Trace**: Create distributed traces in Google Cloud Trace
- **🔐 Automatic Authentication**: Handles gcloud CLI setup and service account authentication
- **⚡ Rate Limiting**: Built-in rate limiting for API calls
- **🎯 Simple API**: Clean, intuitive API with convenience macros

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
gcp-observability-rs = "0.1.0"
```

## Prerequisites

1. **Google Cloud Project** with APIs enabled:
   - Cloud Logging API
   - Cloud Monitoring API
   - Cloud Trace API

2. **Service Account** with roles:
   - `roles/logging.logWriter`
   - `roles/monitoring.metricWriter`
   - `roles/cloudtrace.agent`

3. **gcloud CLI** (automatically installed if missing)

## Quick Start

```rust
use gcp_observability_rs::{ObservabilityClient, gcp_info, gcp_warn, gcp_error};
use std::collections::HashMap;
use std::time::{SystemTime, Duration};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create observability client
    let client = ObservabilityClient::new(
        "your-project-id".to_string(),
        "/path/to/service-account.json".to_string(),
    ).await?;

    // Send logs using convenience macros
    gcp_info!(client, "Application started successfully")?;
    gcp_warn!(client, "This is a warning message")?;
    gcp_error!(client, "This is an error message")?;

    // Send custom metrics
    let mut labels = HashMap::new();
    labels.insert("environment".to_string(), "production".to_string());
    
    client.send_metric(
        "custom.googleapis.com/requests_total".to_string(),
        42.0,
        "INT64".to_string(),
        "GAUGE".to_string(),
        Some(labels),
    ).await?;

    // Create trace spans
    let trace_id = ObservabilityClient::generate_trace_id();
    let span_id = ObservabilityClient::generate_span_id();
    
    client.send_trace_span(
        trace_id,
        span_id,
        "HTTP Request".to_string(),
        SystemTime::now(),
        Duration::from_millis(150),
        None,
    ).await?;

    Ok(())
}
```

## API Reference

### ObservabilityClient

- `new(project_id, service_account_path)` - Creates a new client with authentication
- `send_log(severity, message, service_name)` - Send a log entry
- `send_metric(metric_type, value, value_type, metric_kind, labels)` - Send a metric
- `send_trace_span(trace_id, span_id, display_name, start_time, duration, parent_span_id)` - Send a trace span
- `generate_trace_id()` - Generate a new trace ID
- `generate_span_id()` - Generate a new span ID

### Convenience Macros

- `gcp_info!(client, "message")` - Send an INFO log
- `gcp_warn!(client, "message")` - Send a WARNING log  
- `gcp_error!(client, "message")` - Send an ERROR log
- `gcp_log!(client, "LEVEL", "message")` - Send a log with custom severity

## Features

```toml
[dependencies]
gcp-observability-rs = { version = "0.1.0", features = ["logging", "monitoring"] }
```

Available features:
- `logging` - Cloud Logging functionality
- `monitoring` - Cloud Monitoring functionality  
- `tracing` - Cloud Trace functionality
- `default` - Includes all features

## Examples

Run the example:

```bash
cargo run --example basic_usage
```

## Architecture

This library uses a unique approach that leverages the gcloud CLI:

- **Lightweight**: No heavy protobuf or gRPC dependencies
- **Simple**: Uses standard HTTP calls via curl
- **Reliable**: Leverages battle-tested gcloud CLI for authentication
- **Fast**: Minimal overhead and fast compilation times

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.