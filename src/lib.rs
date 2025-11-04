//! # GCP Observability for Rust
//! 
//! A lightweight Google Cloud Platform observability library for Rust applications.
//! This crate provides easy-to-use APIs for Cloud Logging, Cloud Monitoring, and Cloud Trace
//! using the gcloud CLI instead of heavy SDK dependencies.
//!
//! ## Features
//! 
//! - **Cloud Logging**: Send structured logs to Google Cloud Logging
//! - **Cloud Monitoring**: Create custom metrics in Google Cloud Monitoring
//! - **Cloud Trace**: Create distributed traces in Google Cloud Trace
//! - **Automatic Authentication**: Handles gcloud CLI setup and service account authentication
//! - **Rate Limiting**: Built-in rate limiting for API calls
//! - **Lightweight**: Uses gcloud CLI instead of heavy Google Cloud SDK dependencies
//!
//! ## Example
//!
//! ```rust
//! use gcp_observability_rs::ObservabilityClient;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let client = ObservabilityClient::new(
//!         "your-project-id".to_string(),
//!         "/path/to/service-account.json".to_string(),
//!     ).await?;
//!
//!     // Send a log
//!     client.send_log(
//!         "INFO".to_string(),
//!         "Application started".to_string(),
//!         Some("my-service".to_string()),
//!     ).await?;
//!
//!     // Create a metric
//!     client.send_metric(
//!         "custom.googleapis.com/requests".to_string(),
//!         1.0,
//!         "INT64".to_string(),
//!         "GAUGE".to_string(),
//!         None,
//!     ).await?;
//!
//!     // Create a trace span
//!     client.send_trace_span(
//!         "trace-123".to_string(),
//!         "span-456".to_string(),
//!         "HTTP Request".to_string(),
//!         std::time::SystemTime::now(),
//!         std::time::Duration::from_millis(100),
//!         None,
//!     ).await?;
//!
//!     Ok(())
//! }
//! ```

use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH, Duration};
use std::collections::HashMap;
use lazy_static::lazy_static;
use serde_json::json;
use uuid::Uuid;
use chrono::{DateTime, Utc};

lazy_static! {
    static ref RATE_LIMITER: std::sync::Mutex<HashMap<String, u64>> = std::sync::Mutex::new(HashMap::new());
}

/// Custom error type for observability operations
#[derive(Debug)]
pub enum ObservabilityError {
    /// Authentication failed
    AuthenticationError(String),
    /// API call failed
    ApiError(String),
    /// Setup error (e.g., gcloud not installed)
    SetupError(String),
    /// Rate limiting error
    RateLimitError(String),
}

impl std::fmt::Display for ObservabilityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ObservabilityError::AuthenticationError(msg) => write!(f, "Authentication error: {}", msg),
            ObservabilityError::ApiError(msg) => write!(f, "API error: {}", msg),
            ObservabilityError::SetupError(msg) => write!(f, "Setup error: {}", msg),
            ObservabilityError::RateLimitError(msg) => write!(f, "Rate limit error: {}", msg),
        }
    }
}

impl std::error::Error for ObservabilityError {}

/// Main client for Google Cloud Platform observability services
pub struct ObservabilityClient {
    project_id: String,
    service_account_path: String,
}

impl ObservabilityClient {
    /// Create a new observability client
    /// 
    /// # Arguments
    /// 
    /// * `project_id` - Your Google Cloud Project ID
    /// * `service_account_path` - Path to your service account JSON file
    /// 
    /// # Returns
    /// 
    /// A new `ObservabilityClient` instance after verifying authentication
    pub async fn new(
        project_id: String,
        service_account_path: String,
    ) -> Result<Self, ObservabilityError> {
        let client = Self {
            project_id,
            service_account_path,
        };

        // Ensure gcloud is installed
        client.ensure_gcloud_installed().await?;

        // Setup authentication
        client.setup_authentication().await?;

        // Verify authentication
        client.verify_authentication().await?;

        Ok(client)
    }

    /// Ensure gcloud CLI is installed
    async fn ensure_gcloud_installed(&self) -> Result<(), ObservabilityError> {
        println!("🔍 Checking if gcloud is installed...");
        
        let output = Command::new("gcloud")
            .arg("version")
            .output();

        match output {
            Ok(output) if output.status.success() => {
                let version_info = String::from_utf8_lossy(&output.stdout);
                println!("✅ gcloud is installed: {}", version_info.lines().next().unwrap_or("Unknown version"));
                Ok(())
            }
            _ => {
                println!("❌ gcloud is not installed. Installing...");
                self.install_gcloud().await
            }
        }
    }

    /// Install gcloud CLI
    async fn install_gcloud(&self) -> Result<(), ObservabilityError> {
        println!("📦 Installing gcloud CLI...");
        
        // For macOS, we'll use the installer
        let install_command = if cfg!(target_os = "macos") {
            "curl https://sdk.cloud.google.com | bash"
        } else {
            // For Linux
            "curl https://sdk.cloud.google.com | bash"
        };

        let output = Command::new("sh")
            .arg("-c")
            .arg(install_command)
            .output()
            .map_err(|e| ObservabilityError::SetupError(format!("Failed to install gcloud: {}", e)))?;

        if !output.status.success() {
            return Err(ObservabilityError::SetupError(
                "Failed to install gcloud CLI. Please install manually from https://cloud.google.com/sdk/docs/install".to_string()
            ));
        }

        println!("✅ gcloud CLI installed successfully");
        println!("ℹ️  You may need to restart your terminal and run 'gcloud init' to complete setup");
        
        Ok(())
    }

    /// Setup authentication using service account
    async fn setup_authentication(&self) -> Result<(), ObservabilityError> {
        println!("🔐 Setting up authentication...");
        
        let output = Command::new("gcloud")
            .args([
                "auth",
                "activate-service-account",
                "--key-file",
                &self.service_account_path,
            ])
            .output()
            .map_err(|e| ObservabilityError::AuthenticationError(format!("Failed to run gcloud auth: {}", e)))?;

        if !output.status.success() {
            let error_msg = String::from_utf8_lossy(&output.stderr);
            return Err(ObservabilityError::AuthenticationError(format!(
                "Failed to authenticate with service account: {}", error_msg
            )));
        }

        // Set the project
        let project_output = Command::new("gcloud")
            .args(["config", "set", "project", &self.project_id])
            .output()
            .map_err(|e| ObservabilityError::AuthenticationError(format!("Failed to set project: {}", e)))?;

        if !project_output.status.success() {
            let error_msg = String::from_utf8_lossy(&project_output.stderr);
            return Err(ObservabilityError::AuthenticationError(format!(
                "Failed to set project: {}", error_msg
            )));
        }

        println!("✅ Authentication setup complete");
        Ok(())
    }

    /// Verify authentication is working
    async fn verify_authentication(&self) -> Result<(), ObservabilityError> {
        println!("🔍 Verifying authentication...");
        
        let output = Command::new("gcloud")
            .args(["auth", "list", "--format=json"])
            .output()
            .map_err(|e| ObservabilityError::AuthenticationError(format!("Failed to verify auth: {}", e)))?;

        if !output.status.success() {
            return Err(ObservabilityError::AuthenticationError(
                "Authentication verification failed".to_string()
            ));
        }

        println!("✅ Authentication verified");
        Ok(())
    }

    /// Check rate limiting for API calls
    fn check_rate_limit(&self, api_type: &str) -> Result<(), ObservabilityError> {
        let mut limiter = RATE_LIMITER.lock().unwrap();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        
        let last_call = limiter.get(api_type).unwrap_or(&0);
        
        // Allow up to 1 call per 200ms (5 calls per second)
        if now - last_call < 200 && (last_call != &0) {
            return Err(ObservabilityError::RateLimitError(
                format!("Rate limit exceeded for {}", api_type)
            ));
        }
        
        limiter.insert(api_type.to_string(), now);
        Ok(())
    }

    /// Send a log entry to Cloud Logging
    pub async fn send_log(
        &self,
        severity: String,
        message: String,
        service_name: Option<String>,
    ) -> Result<(), ObservabilityError> {
        self.check_rate_limit("logging")?;

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let mut labels = HashMap::new();
        if let Some(service) = service_name {
            labels.insert("service_name".to_string(), service);
        }

        let log_entry = json!({
            "entries": [{
                "logName": format!("projects/{}/logs/gcp-observability-rs", self.project_id),
                "resource": {
                    "type": "global"
                },
                "timestamp": DateTime::<Utc>::from(UNIX_EPOCH + std::time::Duration::from_secs(timestamp))
                    .format("%Y-%m-%dT%H:%M:%S%.3fZ")
                    .to_string(),
                "severity": severity,
                "textPayload": message,
                "labels": labels
            }]
        });

        let token_output = Command::new("gcloud")
            .args(["auth", "print-access-token"])
            .output()
            .map_err(|e| ObservabilityError::ApiError(format!("Failed to get access token: {}", e)))?;

        let access_token = String::from_utf8_lossy(&token_output.stdout).trim().to_string();

        let curl_output = Command::new("curl")
            .args([
                "-X", "POST",
                &format!("https://logging.googleapis.com/v2/entries:write"),
                "-H", "Content-Type: application/json",
                "-H", &format!("Authorization: Bearer {}", access_token),
                "-d", &log_entry.to_string(),
            ])
            .output()
            .map_err(|e| ObservabilityError::ApiError(format!("Failed to send log: {}", e)))?;

        if !curl_output.status.success() {
            let error_msg = String::from_utf8_lossy(&curl_output.stderr);
            return Err(ObservabilityError::ApiError(format!(
                "Log API call failed: {}", error_msg
            )));
        }

        println!("📝 Log sent: {} - {}", severity, message);
        Ok(())
    }

    /// Send a metric to Cloud Monitoring
    pub async fn send_metric(
        &self,
        metric_type: String,
        value: f64,
        value_type: String,
        _metric_kind: String,
        labels: Option<HashMap<String, String>>,
    ) -> Result<(), ObservabilityError> {
        self.check_rate_limit("monitoring")?;

        let timestamp = SystemTime::now();
        let timestamp_str = DateTime::<Utc>::from(timestamp)
            .format("%Y-%m-%dT%H:%M:%S%.3fZ")
            .to_string();

        let time_series = json!({
            "timeSeries": [{
                "metric": {
                    "type": metric_type,
                    "labels": labels.unwrap_or_default()
                },
                "resource": {
                    "type": "global",
                    "labels": {}
                },
                "points": [{
                    "interval": {
                        "endTime": timestamp_str
                    },
                    "value": {
                        &format!("{}Value", value_type.to_lowercase()): value
                    }
                }]
            }]
        });

        let token_output = Command::new("gcloud")
            .args(["auth", "print-access-token"])
            .output()
            .map_err(|e| ObservabilityError::ApiError(format!("Failed to get access token: {}", e)))?;

        let access_token = String::from_utf8_lossy(&token_output.stdout).trim().to_string();

        let curl_output = Command::new("curl")
            .args([
                "-X", "POST",
                &format!("https://monitoring.googleapis.com/v3/projects/{}/timeSeries", self.project_id),
                "-H", "Content-Type: application/json",
                "-H", &format!("Authorization: Bearer {}", access_token),
                "-d", &time_series.to_string(),
            ])
            .output()
            .map_err(|e| ObservabilityError::ApiError(format!("Failed to send metric: {}", e)))?;

        if !curl_output.status.success() {
            let error_msg = String::from_utf8_lossy(&curl_output.stderr);
            return Err(ObservabilityError::ApiError(format!(
                "Metric API call failed: {}", error_msg
            )));
        }

        println!("📊 Metric sent: {} = {}", metric_type, value);
        Ok(())
    }

    /// Send a trace span to Cloud Trace
    pub async fn send_trace_span(
        &self,
        trace_id: String,
        span_id: String,
        display_name: String,
        start_time: SystemTime,
        duration: Duration,
        parent_span_id: Option<String>,
    ) -> Result<(), ObservabilityError> {
        self.check_rate_limit("tracing")?;

        let start_timestamp = DateTime::<Utc>::from(start_time);
        let end_time = start_time + duration;
        let end_timestamp = DateTime::<Utc>::from(end_time);

        let mut span = json!({
            "name": format!("projects/{}/traces/{}/spans/{}", self.project_id, trace_id, span_id),
            "spanId": span_id,
            "displayName": {
                "value": display_name
            },
            "startTime": start_timestamp.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
            "endTime": end_timestamp.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
        });

        if let Some(parent_id) = parent_span_id {
            span["parentSpanId"] = json!(parent_id);
        }

        let spans_payload = json!({
            "spans": [span]
        });

        let token_output = Command::new("gcloud")
            .args(["auth", "print-access-token"])
            .output()
            .map_err(|e| ObservabilityError::ApiError(format!("Failed to get access token: {}", e)))?;

        let access_token = String::from_utf8_lossy(&token_output.stdout).trim().to_string();

        let curl_output = Command::new("curl")
            .args([
                "-X", "POST",
                &format!("https://cloudtrace.googleapis.com/v2/projects/{}/traces:batchWrite", self.project_id),
                "-H", "Content-Type: application/json",
                "-H", &format!("Authorization: Bearer {}", access_token),
                "-d", &spans_payload.to_string(),
            ])
            .output()
            .map_err(|e| ObservabilityError::ApiError(format!("Failed to send trace: {}", e)))?;

        if !curl_output.status.success() {
            let error_msg = String::from_utf8_lossy(&curl_output.stderr);
            return Err(ObservabilityError::ApiError(format!(
                "Trace API call failed: {}", error_msg
            )));
        }

        println!("🔍 Trace span sent: {} ({})", display_name, span_id);
        Ok(())
    }

    /// Convenience method to generate a new trace ID
    pub fn generate_trace_id() -> String {
        format!("{:032x}", Uuid::new_v4().as_u128())
    }

    /// Convenience method to generate a new span ID
    pub fn generate_span_id() -> String {
        format!("{:016x}", Uuid::new_v4().as_u128() & 0xFFFFFFFFFFFFFFFF)
    }
}

/// Convenience macros for logging
#[macro_export]
macro_rules! gcp_log {
    ($client:expr, $level:expr, $($arg:tt)*) => {
        $client.send_log(
            $level.to_string(),
            format!($($arg)*),
            None,
        ).await
    };
}

#[macro_export]
macro_rules! gcp_info {
    ($client:expr, $($arg:tt)*) => {
        gcp_log!($client, "INFO", $($arg)*)
    };
}

#[macro_export]
macro_rules! gcp_warn {
    ($client:expr, $($arg:tt)*) => {
        gcp_log!($client, "WARNING", $($arg)*)
    };
}

#[macro_export]
macro_rules! gcp_error {
    ($client:expr, $($arg:tt)*) => {
        gcp_log!($client, "ERROR", $($arg)*)
    };
}
