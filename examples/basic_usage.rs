use gcp_observability_rs::{ObservabilityClient, LogEntry, MetricData, TraceSpan};
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

    // Example 1: Simple logging (fire-and-forget)
    client.send_log(LogEntry::new("INFO", "Application started successfully"))?;
    client.send_log(LogEntry::new("WARNING", "This is a warning message"))?;
    client.send_log(LogEntry::new("ERROR", "This is an error message"))?;

    // Example 2: Custom log with service name using struct
    client.send_log(
        LogEntry::new("INFO", "Processing user request")
            .with_service_name("user-service")
    )?;

    // Example 3: Custom metrics using struct
    let mut labels = HashMap::new();
    labels.insert("environment".to_string(), "development".to_string());
    labels.insert("service".to_string(), "example-service".to_string());

    client.send_metric(
        MetricData::new(
            "custom.googleapis.com/example/requests_total",
            42.0,
            "INT64",
            "GAUGE"
        ).with_labels(labels)
    )?;

    client.send_metric(
        MetricData::new(
            "custom.googleapis.com/example/response_time_ms",
            125.5,
            "DOUBLE",
            "GAUGE"
        )
    )?;

    // Example 4: Distributed tracing using struct
    let trace_id = ObservabilityClient::generate_trace_id();
    let span_id = ObservabilityClient::generate_span_id();
    let child_span_id = ObservabilityClient::generate_span_id();

    // Parent span
    client.send_trace(
        TraceSpan::new(
            trace_id.clone(),
            span_id.clone(),
            "HTTP Request",
            SystemTime::now(),
            Duration::from_millis(150)
        )
    )?;

    // Child span
    client.send_trace(
        TraceSpan::new(
            trace_id.clone(),
            child_span_id,
            "Database Query",
            SystemTime::now(),
            Duration::from_millis(50)
        ).with_parent_span_id(span_id)
    )?;

    println!("✅ All observability examples queued!");
    println!("📊 Check your Google Cloud Console:");
    println!("   - Logs: https://console.cloud.google.com/logs");
    println!("   - Metrics: https://console.cloud.google.com/monitoring");
    println!("   - Traces: https://console.cloud.google.com/traces");

    // Give the background worker time to process
    tokio::time::sleep(Duration::from_secs(2)).await;

    Ok(())
}