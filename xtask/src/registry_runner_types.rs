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
    #[serde(rename = "claimId")]
    pub(crate) claim_id: String,
    #[serde(rename = "requestId")]
    pub(crate) request_id: String,
    pub(crate) slug: String,
    pub(crate) version: String,
    #[serde(rename = "stageKey")]
    pub(crate) stage_key: String,
    #[serde(rename = "executionMode")]
    pub(crate) execution_mode: String,
    #[serde(rename = "artifactChecksumSha256")]
    pub(crate) artifact_checksum_sha256: String,
    #[serde(rename = "crateName")]
    pub(crate) crate_name: String,
    pub(crate) runnable: bool,
    #[serde(rename = "requiresManualConfirmation")]
    pub(crate) requires_manual_confirmation: bool,
    #[serde(default, rename = "allowedTerminalReasonCodes")]
    pub(crate) allowed_terminal_reason_codes: Vec<String>,
    #[serde(rename = "suggestedPassReasonCode")]
    pub(crate) suggested_pass_reason_code: Option<String>,
    #[serde(rename = "suggestedFailureReasonCode")]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runner_claim_deserializes_the_server_wire_contract() {
        let claim: RegistryRunnerClaimHttpPayload = serde_json::from_value(serde_json::json!({
            "claimId": "rvc_claim",
            "requestId": "rpr_request",
            "slug": "sample-module",
            "version": "1.2.3",
            "stageKey": "compile_smoke",
            "executionMode": "local_workspace",
            "runnable": true,
            "requiresManualConfirmation": false,
            "allowedTerminalReasonCodes": ["local_runner_passed"],
            "suggestedPassReasonCode": "local_runner_passed",
            "suggestedFailureReasonCode": "build_failure",
            "suggestedBlockedReasonCode": "manual_override",
            "artifactDownloadUrl": "/v2/catalog/requests/rpr_request/artifact",
            "artifactChecksumSha256": "a".repeat(64),
            "crateName": "sample_module"
        }))
        .expect("server claim payload must deserialize");

        assert_eq!(claim.claim_id, "rvc_claim");
        assert_eq!(claim.request_id, "rpr_request");
        assert_eq!(claim.stage_key, "compile_smoke");
        assert_eq!(claim.artifact_checksum_sha256, "a".repeat(64));
        assert_eq!(claim.crate_name, "sample_module");
    }
}
