//! # GCP Observability for Rust
//!
//! A lightweight, high-performance Google Cloud Platform observability library for Rust applications.
//! This crate provides easy-to-use APIs for Cloud Logging, Cloud Monitoring, and Cloud Trace
//! using the gcloud CLI for authentication and the Google Cloud REST APIs for data submission.
//!
//! ## Features
//!
//! - **Cloud Logging**: Send structured logs to Google Cloud Logging
//! - **Cloud Monitoring**: Create custom metrics in Google Cloud Monitoring
//! - **Cloud Trace**: Create distributed traces in Google Cloud Trace
//! - **Background Processing**: Fire-and-forget API with background thread processing
//! - **Async Support**: Optional async methods for awaiting operation completion
//! - **Automatic Token Refresh**: Handles gcloud token expiration and re-authentication
//! - **Error Resilience**: Automatic retry logic for authentication failures
//! - **Builder Pattern**: Fluent API for constructing observability data
//!
//! ## Architecture
//!
//! The library uses a channel-based architecture with a single background worker thread:
//!
//! - **Main Thread**: Your application code sends observability data to a channel
//! - **Worker Thread**: A dedicated `std::thread` processes queued items using async operations
//! - **No Rate Limiting**: The single-threaded model naturally prevents overwhelming the APIs
//! - **Silent Failures**: Background operations fail silently to avoid disrupting your application
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use gcp_observability_rs::{ObservabilityClient, LogEntry, MetricData, TraceSpan};
//! use std::collections::HashMap;
//! use std::time::{SystemTime, Duration};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Initialize the client (performs authentication)
//!     let client = ObservabilityClient::new(
//!         "your-project-id".to_string(),
//!         "/path/to/service-account.json".to_string(),
//!     ).await?;
//!
//!     // Fire-and-forget logging (returns immediately, processes in background)
//!     client.send_log(LogEntry::new("INFO", "Application started"))?;
//!     
//!     // With service name
//!     client.send_log(
//!         LogEntry::new("ERROR", "Database connection failed")
//!             .with_service_name("api-server")
//!     )?;
//!
//!     // Send metrics with labels
//!     let mut labels = HashMap::new();
//!     labels.insert("environment".to_string(), "production".to_string());
//!     
//!     client.send_metric(
//!         MetricData::new(
//!             "custom.googleapis.com/requests_total",
//!             42.0,
//!             "INT64",
//!             "GAUGE"
//!         ).with_labels(labels)
//!     )?;
//!
//!     // Create distributed traces
//!     let trace_id = ObservabilityClient::generate_trace_id();
//!     let span_id = ObservabilityClient::generate_span_id();
//!     
//!     client.send_trace_span(
//!         TraceSpan::new(
//!             trace_id,
//!             span_id,
//!             "HTTP Request",
//!             SystemTime::now(),
//!             Duration::from_millis(150)
//!         )
//!     )?;
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Async Operations
//!
//! When you need confirmation that an operation completed successfully, use the async methods:
//!
//! ```rust,no_run
//! # use gcp_observability_rs::{ObservabilityClient, LogEntry};
//! # async fn example(client: ObservabilityClient) -> Result<(), Box<dyn std::error::Error>> {
//! // Wait for the operation to complete
//! client.send_log_async(LogEntry::new("INFO", "Critical operation")).await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Convenience Macros
//!
//! For quick logging without constructing `LogEntry` objects:
//!
//! ```rust,no_run
//! # use gcp_observability_rs::{ObservabilityClient, gcp_info, gcp_warn, gcp_error};
//! # fn example(client: ObservabilityClient) -> Result<(), Box<dyn std::error::Error>> {
//! gcp_info!(client, "User {} logged in", user_id)?;
//! gcp_warn!(client, "High memory usage: {}%", usage)?;
//! gcp_error!(client, "Failed to process request: {}", error)?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Error Handling
//!
//! The library provides robust error handling:
//!
//! - **Authentication Errors**: Automatically detected and recovered via token refresh
//! - **API Errors**: Detailed error messages with HTTP status codes
//! - **Background Failures**: Silently handled to avoid disrupting your application
//! - **Setup Errors**: Returned immediately during client initialization
//!
//! ## Token Management
//!
//! The library automatically handles gcloud token expiration:
//!
//! 1. Detects expired tokens (401/403 responses)
//! 2. Re-authenticates using your service account
//! 3. Retries the failed operation with a fresh token
//! 4. All happens transparently without manual intervention
//!
//! ## Performance Considerations
//!
//! - **Non-blocking**: Fire-and-forget methods return immediately
//! - **Single Worker**: One background thread prevents API rate limit issues
//! - **Bounded Channel**: 1027-item buffer prevents memory overflow
//! - **Minimal Overhead**: No rate limiting logic or complex synchronization

pub mod helpers;
pub mod pubsub;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use crossbeam::channel::{bounded, Receiver, Sender};
use serde_json::json;
use std::collections::HashMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use uuid::Uuid;

/// Errors for observability operations
#[derive(Debug)]
pub enum ObservabilityError {
    AuthenticationError(String),
    ApiError(String),
    SetupError(String),
    /// Special error: used by SIGTERM to request shutdown of worker loop
    Shutdown,
}

impl std::fmt::Display for ObservabilityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ObservabilityError::AuthenticationError(msg) => {
                write!(f, "Authentication error: {}", msg)
            }
            ObservabilityError::ApiError(msg) => write!(f, "API error: {}", msg),
            ObservabilityError::SetupError(msg) => write!(f, "Setup error: {}", msg),
            ObservabilityError::Shutdown => write!(f, "Shutdown requested"),
        }
    }
}
impl std::error::Error for ObservabilityError {}

/// Each message type implements `Handle` to execute itself using the client.
#[async_trait]
pub trait Handle: Send {
    async fn handle(
        self: Box<Self>,
        client: &ObservabilityClient,
    ) -> Result<(), ObservabilityError>;
}

/// Log entry data for Cloud Logging
#[derive(Debug, Clone)]
pub struct LogEntry {
    pub severity: String,
    pub message: String,
    pub service_name: Option<String>,
    pub log_name: Option<String>,
}
impl LogEntry {
    pub fn new(severity: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            severity: severity.into(),
            message: message.into(),
            service_name: None,
            log_name: None,
        }
    }
    pub fn with_service_name(mut self, service_name: impl Into<String>) -> Self {
        self.service_name = Some(service_name.into());
        self
    }
    pub fn with_log_name(mut self, log_name: impl Into<String>) -> Self {
        self.log_name = Some(log_name.into());
        self
    }
}
#[async_trait]
impl Handle for LogEntry {
    async fn handle(
        self: Box<Self>,
        client: &ObservabilityClient,
    ) -> Result<(), ObservabilityError> {
        client.send_log_impl(*self).await
    }
}

/// Metric data for Cloud Monitoring
#[derive(Debug, Clone)]
pub struct MetricData {
    pub metric_type: String,
    pub value: f64,
    pub value_type: String,
    pub metric_kind: String,
    pub labels: Option<HashMap<String, String>>,
}
impl MetricData {
    pub fn new(
        metric_type: impl Into<String>,
        value: f64,
        value_type: impl Into<String>,
        metric_kind: impl Into<String>,
    ) -> Self {
        Self {
            metric_type: metric_type.into(),
            value,
            value_type: value_type.into(),
            metric_kind: metric_kind.into(),
            labels: None,
        }
    }
    pub fn with_labels(mut self, labels: HashMap<String, String>) -> Self {
        self.labels = Some(labels);
        self
    }
}
#[async_trait]
impl Handle for MetricData {
    async fn handle(
        self: Box<Self>,
        client: &ObservabilityClient,
    ) -> Result<(), ObservabilityError> {
        client.send_metric_impl(*self).await
    }
}

/// Trace span data for Cloud Trace
#[derive(Debug, Clone)]
pub struct TraceSpan {
    pub trace_id: String,
    pub span_id: String,
    pub display_name: String,
    pub start_time: SystemTime,
    pub duration: Duration,
    pub parent_span_id: Option<String>,
    pub attributes: HashMap<String, String>,
    pub status: Option<TraceStatus>,
}

#[derive(Debug, Clone)]
pub struct TraceStatus {
    pub code: i32, // 0=OK, 1=CANCELLED, 2=UNKNOWN, 3=INVALID_ARGUMENT... (using gRPC codes)
    pub message: Option<String>,
}

impl TraceSpan {
    pub fn new(
        trace_id: impl Into<String>,
        span_id: impl Into<String>,
        display_name: impl Into<String>,
        start_time: SystemTime,
        duration: Duration,
    ) -> Self {
        Self {
            trace_id: trace_id.into(),
            span_id: span_id.into(),
            display_name: display_name.into(),
            start_time,
            duration,
            parent_span_id: None,
            attributes: HashMap::new(),
            status: None,
        }
    }
    pub fn with_parent_span_id(mut self, parent_span_id: impl Into<String>) -> Self {
        self.parent_span_id = Some(parent_span_id.into());
        self
    }
    pub fn with_attribute(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.attributes.insert(key.into(), value.into());
        self
    }
    pub fn with_status_error(mut self, message: impl Into<String>) -> Self {
        self.status = Some(TraceStatus {
            code: 2, // UNKNOWN (generic error)
            message: Some(message.into()),
        });
        self
    }
    pub fn child(&self, name: impl Into<String>, start_time: SystemTime, duration: Duration) -> Self {
        Self {
            trace_id: self.trace_id.clone(), // Same trace ID
            span_id: ObservabilityClient::generate_span_id(), // New span ID
            parent_span_id: Some(self.span_id.clone()), // Parent is the current span
            display_name: name.into(),
            start_time,
            duration,
            attributes: HashMap::new(),
            status: None,
        }
    }
}
#[async_trait]
impl Handle for TraceSpan {
    async fn handle(
        self: Box<Self>,
        client: &ObservabilityClient,
    ) -> Result<(), ObservabilityError> {
        client.send_trace_span_impl(*self).await
    }
}

/// SIGTERM command—used to stop the worker loop
#[derive(Debug, Clone, Copy)]
pub struct SIGTERM;
#[async_trait]
impl Handle for SIGTERM {
    async fn handle(
        self: Box<Self>,
        _client: &ObservabilityClient,
    ) -> Result<(), ObservabilityError> {
        Err(ObservabilityError::Shutdown)
    }
}

/// Main client
#[derive(Clone)]
pub struct ObservabilityClient {
    project_id: String,
    service_account_path: String,
    service_name: Option<String>,
    tx: Sender<Box<dyn Handle>>,
}

impl ObservabilityClient {
    pub async fn new(
        project_id: String,
        service_account_path: String,
        service_name: Option<String>,
    ) -> Result<Self, ObservabilityError> {
        let (tx, rx): (Sender<Box<dyn Handle>>, Receiver<Box<dyn Handle>>) = bounded(1027);

        let client = Self {
            project_id,
            service_account_path,
            service_name,
            tx,
        };

        // Setup auth (left as-is from your original design)
        client.ensure_gcloud_installed().await?;
        client.setup_authentication().await?;
        client.verify_authentication().await?;

        // Worker thread that blocks on a Tokio runtime to run async handlers
        let client_clone = client.clone();
        let handle = tokio::runtime::Handle::current();
        std::thread::spawn(move || {
            while let Ok(msg) = rx.recv() {
                let result = handle.block_on(async { msg.handle(&client_clone).await });
                match result {
                    Ok(()) => {}
                    Err(ObservabilityError::Shutdown) => {
                        break;
                    }
                    Err(_e) => {
                        // Silently handle errors in background processing
                    }
                }
            }
        });

        Ok(client)
    }

    /// Public convenience API — callers never box manually

    pub fn send_log(
        &self,
        entry: LogEntry,
    ) -> Result<(), crossbeam::channel::SendError<Box<dyn Handle>>> {
        self.tx.send(Box::new(entry))
    }

    pub fn send_metric(
        &self,
        data: MetricData,
    ) -> Result<(), crossbeam::channel::SendError<Box<dyn Handle>>> {
        self.tx.send(Box::new(data))
    }

    pub fn send_trace(
        &self,
        span: TraceSpan,
    ) -> Result<(), crossbeam::channel::SendError<Box<dyn Handle>>> {
        self.tx.send(Box::new(span))
    }

    pub fn shutdown(&self) -> Result<(), crossbeam::channel::SendError<Box<dyn Handle>>> {
        self.tx.send(Box::new(SIGTERM))
    }

    /// ---------- Internal helpers below (mostly as you had them) ----------

    async fn ensure_gcloud_installed(&self) -> Result<(), ObservabilityError> {
        let output = tokio::process::Command::new("gcloud")
            .arg("version")
            .output()
            .await;
        match output {
            Ok(output) if output.status.success() => Ok(()),
            _ => self.install_gcloud().await,
        }
    }

    async fn install_gcloud(&self) -> Result<(), ObservabilityError> {
        let install_command = if cfg!(target_os = "macos") {
            "curl https://sdk.cloud.google.com | bash"
        } else {
            "curl https://sdk.cloud.google.com | bash"
        };
        let output = tokio::process::Command::new("sh")
            .arg("-c")
            .arg(install_command)
            .output()
            .await
            .map_err(|e| {
                ObservabilityError::SetupError(format!("Failed to install gcloud: {}", e))
            })?;
        if !output.status.success() {
            return Err(ObservabilityError::SetupError(
                "Failed to install gcloud CLI. Please install manually from https://cloud.google.com/sdk/docs/install".to_string(),
            ));
        }
        Ok(())
    }

    async fn setup_authentication(&self) -> Result<(), ObservabilityError> {
        let output = tokio::process::Command::new("gcloud")
            .args([
                "auth",
                "activate-service-account",
                "--key-file",
                &self.service_account_path,
            ])
            .output()
            .await
            .map_err(|e| {
                ObservabilityError::AuthenticationError(format!("Failed to run gcloud auth: {}", e))
            })?;
        if !output.status.success() {
            let error_msg = String::from_utf8_lossy(&output.stderr);
            return Err(ObservabilityError::AuthenticationError(format!(
                "Failed to authenticate with service account: {}",
                error_msg
            )));
        }
        let project_output = tokio::process::Command::new("gcloud")
            .args(["config", "set", "project", &self.project_id])
            .output()
            .await
            .map_err(|e| {
                ObservabilityError::AuthenticationError(format!("Failed to set project: {}", e))
            })?;
        if !project_output.status.success() {
            let error_msg = String::from_utf8_lossy(&project_output.stderr);
            return Err(ObservabilityError::AuthenticationError(format!(
                "Failed to set project: {}",
                error_msg
            )));
        }
        Ok(())
    }

    async fn verify_authentication(&self) -> Result<(), ObservabilityError> {
        let output = tokio::process::Command::new("gcloud")
            .args(["auth", "list", "--format=json"])
            .output()
            .await
            .map_err(|e| {
                ObservabilityError::AuthenticationError(format!("Failed to verify auth: {}", e))
            })?;
        if !output.status.success() {
            return Err(ObservabilityError::AuthenticationError(
                "Authentication verification failed".to_string(),
            ));
        }
        Ok(())
    }

    pub async fn get_identity_token(&self) -> Result<String, ObservabilityError> {
        match self.get_identity_token_internal().await {
            Ok(token) => Ok(token),
            Err(e) => {
                if e.to_string().contains("not logged in")
                    || e.to_string().contains("authentication")
                    || e.to_string().contains("expired")
                {
                    self.refresh_authentication().await?;
                    self.get_identity_token_internal().await
                } else {
                    Err(e)
                }
            }
        }
    }

    async fn get_identity_token_internal(&self) -> Result<String, ObservabilityError> {
        let output = tokio::process::Command::new("gcloud")
            .args(["auth", "print-identity-token"])
            .output()
            .await
            .map_err(|e| {
                ObservabilityError::ApiError(format!("Failed to run gcloud command: {}", e))
            })?;
        if !output.status.success() {
            let error_msg = String::from_utf8_lossy(&output.stderr);
            return Err(ObservabilityError::AuthenticationError(format!(
                "Failed to get identity token: {}",
                error_msg
            )));
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    async fn get_access_token_with_retry(&self) -> Result<String, ObservabilityError> {
        match self.get_access_token().await {
            Ok(token) => Ok(token),
            Err(e) => {
                if e.to_string().contains("not logged in")
                    || e.to_string().contains("authentication")
                    || e.to_string().contains("expired")
                {
                    self.refresh_authentication().await?;
                    self.get_access_token().await
                } else {
                    Err(e)
                }
            }
        }
    }

    async fn get_access_token(&self) -> Result<String, ObservabilityError> {
        let output = tokio::process::Command::new("gcloud")
            .args(["auth", "print-access-token"])
            .output()
            .await
            .map_err(|e| {
                ObservabilityError::ApiError(format!("Failed to run gcloud command: {}", e))
            })?;
        if !output.status.success() {
            let error_msg = String::from_utf8_lossy(&output.stderr);
            return Err(ObservabilityError::AuthenticationError(format!(
                "Failed to get access token: {}",
                error_msg
            )));
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    async fn refresh_authentication(&self) -> Result<(), ObservabilityError> {
        let output = tokio::process::Command::new("gcloud")
            .args([
                "auth",
                "activate-service-account",
                "--key-file",
                &self.service_account_path,
            ])
            .output()
            .await
            .map_err(|e| {
                ObservabilityError::AuthenticationError(format!("Failed to refresh auth: {}", e))
            })?;
        if !output.status.success() {
            let error_msg = String::from_utf8_lossy(&output.stderr);
            return Err(ObservabilityError::AuthenticationError(format!(
                "Failed to refresh authentication: {}",
                error_msg
            )));
        }
        Ok(())
    }

    async fn execute_api_request(
        &self,
        api_url: &str,
        payload: &str,
        operation_name: &str,
    ) -> Result<(), ObservabilityError> {
        let mut retries = 0;
        const MAX_RETRIES: u32 = 2;

        loop {
            let access_token = self.get_access_token_with_retry().await?;
            let output = tokio::process::Command::new("curl")
                .args([
                    "-X",
                    "POST",
                    api_url,
                    "-H",
                    "Content-Type: application/json",
                    "-H",
                    &format!("Authorization: Bearer {}", access_token),
                    "-d",
                    payload,
                    "-s",
                    "-w",
                    "%{http_code}",
                ])
                .output()
                .await
                .map_err(|e| {
                    ObservabilityError::ApiError(format!(
                        "Failed to execute {} request: {}",
                        operation_name, e
                    ))
                })?;

            let response_body = String::from_utf8_lossy(&output.stdout);
            let status_code = response_body
                .chars()
                .rev()
                .take(3)
                .collect::<String>()
                .chars()
                .rev()
                .collect::<String>();

            if output.status.success() && (status_code.starts_with("20") || status_code == "200") {
                return Ok(());
            }

            let error_msg = String::from_utf8_lossy(&output.stderr);
            if (status_code == "401" || status_code == "403") && retries < MAX_RETRIES {
                retries += 1;
                self.refresh_authentication().await?;
                continue;
            }

            return Err(ObservabilityError::ApiError(format!(
                "{} API call failed with status {}: {} - Response: {}",
                operation_name, status_code, error_msg, response_body
            )));
        }
    }

    // ---------- The three concrete senders ----------

    async fn send_log_impl(&self, log_entry: LogEntry) -> Result<(), ObservabilityError> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let mut labels = HashMap::new();
        
        // Use the entry's service name, fallback to client's default, or ignore
        if let Some(service) = log_entry.service_name.or(self.service_name.clone()) {
            labels.insert("service_name".to_string(), service);
        }
        
        let log_name = log_entry
            .log_name
            .clone()
            .unwrap_or_else(|| "gcp-observability-rs".to_string());

        let log_entry_json = json!({
            "entries": [{
                "logName": format!("projects/{}/logs/{}", self.project_id, log_name),
                "resource": { "type": "global" },
                "timestamp": DateTime::<Utc>::from(UNIX_EPOCH + std::time::Duration::from_secs(timestamp))
                    .format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
                "severity": log_entry.severity,
                "textPayload": log_entry.message,
                "labels": labels
            }]
        });
        let api_url = "https://logging.googleapis.com/v2/entries:write";
        self.execute_api_request(api_url, &log_entry_json.to_string(), "Logging")
            .await?;
        Ok(())
    }

    async fn send_metric_impl(&self, metric_data: MetricData) -> Result<(), ObservabilityError> {
        let timestamp = SystemTime::now();
        let timestamp_str = DateTime::<Utc>::from(timestamp)
            .format("%Y-%m-%dT%H:%M:%S%.3fZ")
            .to_string();

        let time_series = json!({
            "timeSeries": [{
                "metric": {
                    "type": metric_data.metric_type,
                    "labels": metric_data.labels.unwrap_or_default()
                },
                "resource": { "type": "global", "labels": {} },
                "points": [{
                    "interval": { "endTime": timestamp_str },
                    "value": {
                        &format!("{}Value", metric_data.value_type.to_lowercase()): metric_data.value
                    }
                }]
            }]
        });
        let api_url = &format!(
            "https://monitoring.googleapis.com/v3/projects/{}/timeSeries",
            self.project_id
        );
        self.execute_api_request(api_url, &time_series.to_string(), "Monitoring")
            .await?;
        Ok(())
    }

    async fn send_trace_span_impl(&self, trace_span: TraceSpan) -> Result<(), ObservabilityError> {
        let start_timestamp = DateTime::<Utc>::from(trace_span.start_time);
        let end_time = trace_span.start_time + trace_span.duration;
        let end_timestamp = DateTime::<Utc>::from(end_time);

        let mut attributes_json = json!({});
        if !trace_span.attributes.is_empty() {
            let mut attribute_map = serde_json::Map::new();
            for (k, v) in trace_span.attributes {
                attribute_map.insert(k, json!({ "string_value": { "value": v } }));
            }
            attributes_json = json!({ "attributeMap": attribute_map });
        }

        let mut span = json!({
            "name": format!("projects/{}/traces/{}/spans/{}", self.project_id, trace_span.trace_id, trace_span.span_id),
            "spanId": trace_span.span_id,
            "displayName": { "value": trace_span.display_name },
            "startTime": start_timestamp.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
            "endTime": end_timestamp.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
            "attributes": attributes_json
        });

        if let Some(parent_id) = &trace_span.parent_span_id {
            span["parentSpanId"] = json!(parent_id);
        }

        if let Some(status) = &trace_span.status {
            span["status"] = json!({
                "code": status.code,
                "message": status.message
            });
        }

        let spans_payload = json!({ "spans": [span] });
        let api_url = &format!(
            "https://cloudtrace.googleapis.com/v2/projects/{}/traces:batchWrite",
            self.project_id
        );
        self.execute_api_request(api_url, &spans_payload.to_string(), "Tracing")
            .await?;
        Ok(())
    }

    /// Convenience IDs
    pub fn generate_trace_id() -> String {
        format!("{:032x}", Uuid::new_v4().as_u128())
    }
    pub fn generate_span_id() -> String {
        format!("{:016x}", Uuid::new_v4().as_u128() & 0xFFFFFFFFFFFFFFFF)
    }
}

