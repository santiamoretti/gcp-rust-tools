use gcp_observability_rs::{ObservabilityClient, gcp_info, gcp_warn, gcp_error, gcp_log};
use std::collections::HashMap;
use std::time::{SystemTime, Duration};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create observability client
    let client = ObservabilityClient::new(
        "your-gcp-project-id".to_string(),
        "/path/to/service-account.json".to_string(),
    ).await?;

    println!("🚀 Starting observability example...");

    // Example 1: Simple logging
    gcp_info!(client, "Application started successfully")?;
    gcp_warn!(client, "This is a warning message")?;
    gcp_error!(client, "This is an error message")?;

    // Example 2: Custom log with service name
    client.send_log(
        "INFO".to_string(),
        "Processing user request".to_string(),
        Some("user-service".to_string()),
    ).await?;

    // Example 3: Custom metrics
    let mut labels = HashMap::new();
    labels.insert("environment".to_string(), "development".to_string());
    labels.insert("service".to_string(), "example-service".to_string());

    client.send_metric(
        "custom.googleapis.com/example/requests_total".to_string(),
        42.0,
        "INT64".to_string(),
        "GAUGE".to_string(),
        Some(labels),
    ).await?;

    client.send_metric(
        "custom.googleapis.com/example/response_time_ms".to_string(),
        125.5,
        "DOUBLE".to_string(),
        "GAUGE".to_string(),
        None,
    ).await?;

    // Example 4: Distributed tracing
    let trace_id = ObservabilityClient::generate_trace_id();
    let span_id = ObservabilityClient::generate_span_id();
    let child_span_id = ObservabilityClient::generate_span_id();

    // Parent span
    client.send_trace_span(
        trace_id.clone(),
        span_id.clone(),
        "HTTP Request".to_string(),
        SystemTime::now(),
        Duration::from_millis(150),
        None,
    ).await?;

    // Child span
    client.send_trace_span(
        trace_id.clone(),
        child_span_id,
        "Database Query".to_string(),
        SystemTime::now(),
        Duration::from_millis(50),
        Some(span_id),
    ).await?;

    println!("✅ All observability examples completed!");
    println!("📊 Check your Google Cloud Console:");
    println!("   - Logs: https://console.cloud.google.com/logs");
    println!("   - Metrics: https://console.cloud.google.com/monitoring");
    println!("   - Traces: https://console.cloud.google.com/traces");

    Ok(())
}