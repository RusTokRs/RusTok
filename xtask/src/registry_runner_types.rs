use super::*;

#[derive(Debug, Serialize)]
pub(crate) struct RegistryRunnerClaimHttpRequest {
    pub(crate) schema_version: u32,
    pub(crate) runner_id: String,
    pub(crate) supported_stages: Vec<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct RegistryRunnerHeartbeatHttpRequest {
    pub(crate) schema_version: u32,
    pub(crate) runner_id: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct RegistryRunnerCompletionHttpRequest {
    pub(crate) schema_version: u32,
    pub(crate) runner_id: String,
    pub(crate) detail: Option<String>,
    pub(crate) reason_code: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RegistryRunnerClaimHttpResponse {
    pub(crate) accepted: bool,
    pub(crate) claim: Option<RegistryRunnerClaimHttpPayload>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RegistryRunnerClaimHttpPayload {
    pub(crate) claim_id: String,
    pub(crate) request_id: String,
    pub(crate) slug: String,
    pub(crate) version: String,
    pub(crate) stage_key: String,
    pub(crate) execution_mode: String,
    pub(crate) artifact_download_url: String,
    pub(crate) runnable: bool,
    #[serde(default)]
    pub(crate) allowed_terminal_reason_codes: Vec<String>,
    pub(crate) suggested_pass_reason_code: Option<String>,
    pub(crate) suggested_failure_reason_code: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RegistryRunnerMutationHttpResponse {
    pub(crate) accepted: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct ModuleRunnerPreview {
    pub(crate) action: String,
    pub(crate) runner_id: String,
    pub(crate) supported_stages: Vec<String>,
    pub(crate) poll_interval_ms: u64,
    pub(crate) heartbeat_interval_ms: u64,
    pub(crate) once: bool,
    pub(crate) confirm_manual_review: bool,
}

pub(crate) const REMOTE_RUNNER_TOKEN_ENV: &str = "RUSTOK_MODULE_RUNNER_TOKEN";
pub(crate) const DEFAULT_REMOTE_RUNNER_POLL_INTERVAL_MS: u64 = 5_000;
pub(crate) const DEFAULT_REMOTE_RUNNER_HEARTBEAT_INTERVAL_MS: u64 = 5_000;
