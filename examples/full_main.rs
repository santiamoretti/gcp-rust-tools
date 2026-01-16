use gcp_rust_tools::{pubsub::create_pubsub_client, LogEntry, ObservabilityClient};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // No credentials/project logic in main.
    // The crate resolves:
    // - credentials from GOOGLE_APPLICATION_CREDENTIALS (or GOOGLE_CREDENTIALS)
    // - project id from (in order): provided value, GOOGLE_CLOUD_PROJECT, or `gcloud config get-value project`

    let observability = ObservabilityClient::new(None, Some("example-service".to_string())).await?;

    // Pub/Sub (also resolves credentials + project internally)
    let topics: Arc<[&'static str]> = Arc::from(["events"]);
    let subs: Arc<[&'static str]> = Arc::from(["events-sub"]);

    let pubsub = create_pubsub_client(None, "dev", topics, subs).await?;

    // Fire-and-forget queueing into the background worker.
    // If the channel is closed, we just continue in this example.
    let _ = observability.send_log(LogEntry::new("INFO", "Example started"));

    // Publish a simple message (fire-and-forget)
    pubsub
        .publish_fire_and_forget("events", serde_json::json!({"hello": "world"}), None)
        .await;

    // Give background work a moment (optional for examples)
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    Ok(())
}
