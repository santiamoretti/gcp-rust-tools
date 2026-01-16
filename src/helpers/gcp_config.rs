use std::env;

/// Standard env var used by Google SDKs to locate the service account JSON.
pub const GOOGLE_APPLICATION_CREDENTIALS: &str = "GOOGLE_APPLICATION_CREDENTIALS";

/// Non-standard alias some teams use. If set, we accept it as a fallback.
pub const GOOGLE_CREDENTIALS: &str = "GOOGLE_CREDENTIALS";

/// Standard env var used by many GCP libraries/runtimes.
pub const GOOGLE_CLOUD_PROJECT: &str = "GOOGLE_CLOUD_PROJECT";

pub fn credentials_path_from_env() -> Result<String, String> {
    let candidates = [GOOGLE_APPLICATION_CREDENTIALS, GOOGLE_CREDENTIALS];

    for key in candidates {
        if let Ok(val) = env::var(key) {
            let trimmed = val.trim();
            if !trimmed.is_empty() {
                return Ok(trimmed.to_string());
            }
        }
    }

    Err(format!(
        "Missing credentials env var: set '{}' (or '{}') to the service-account JSON path",
        GOOGLE_APPLICATION_CREDENTIALS, GOOGLE_CREDENTIALS
    ))
}

fn project_id_from_env() -> Option<String> {
    match env::var(GOOGLE_CLOUD_PROJECT) {
        Ok(val) => {
            let trimmed = val.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        }
        Err(_) => None,
    }
}

pub async fn project_id_from_gcloud() -> Result<String, String> {
    let output = tokio::process::Command::new("gcloud")
        .args(["config", "get-value", "project", "--quiet"])
        .output()
        .await
        .map_err(|e| format!("Failed to run 'gcloud config get-value project': {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "gcloud failed to read project (is gcloud installed/logged in?): {}",
            stderr.trim()
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let project_id = stdout.trim();
    if project_id.is_empty() {
        return Err("gcloud returned an empty project id".to_string());
    }

    Ok(project_id.to_string())
}

pub async fn resolve_project_id(provided: Option<String>) -> Result<String, String> {
    if let Some(project_id) = provided {
        let trimmed = project_id.trim();
        if !trimmed.is_empty() {
            return Ok(trimmed.to_string());
        }
    }

    if let Some(project_id) = project_id_from_env() {
        return Ok(project_id);
    }

    project_id_from_gcloud().await
}
