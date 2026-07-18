use rustok_modules::{
    ModuleControlPlane, ModuleRemoteValidationClaim, ModuleRemoteValidationClaimCommand,
};
use sea_orm::DatabaseConnection;

use crate::services::registry_governance::RegistryRemoteValidationClaim;

/// Server transport adapter for the owner-owned remote validation claim
/// transaction. Runner authentication and the artifact download route stay at
/// the host boundary; selection, CAS claim, and governance audit facts do not.
pub async fn claim_remote_validation_stage_atomic(
    db: &DatabaseConnection,
    runner_id: &str,
    supported_stages: &[String],
    lease_ttl_ms: u64,
) -> anyhow::Result<Option<RegistryRemoteValidationClaim>> {
    ModuleControlPlane::new(db.clone())
        .publication()
        .claim_remote_validation_stage(ModuleRemoteValidationClaimCommand {
            runner_id: runner_id.to_string(),
            supported_stages: supported_stages.to_vec(),
            lease_ttl_ms,
        })
        .await
        .map_err(anyhow::Error::new)
        .map(|claim| claim.map(adapt_claim))
}

fn adapt_claim(claim: ModuleRemoteValidationClaim) -> RegistryRemoteValidationClaim {
    RegistryRemoteValidationClaim {
        artifact_download_url: format!(
            "/v2/catalog/publish/{}/artifact/download",
            claim.request_id
        ),
        claim_id: claim.claim_id,
        request_id: claim.request_id,
        slug: claim.slug,
        version: claim.version,
        stage_key: claim.stage_key,
        execution_mode: claim.execution_mode,
        runnable: true,
        requires_manual_confirmation: claim.requires_manual_confirmation,
        allowed_terminal_reason_codes: claim.allowed_terminal_reason_codes,
        suggested_pass_reason_code: Some(claim.suggested_pass_reason_code),
        suggested_failure_reason_code: Some(claim.suggested_failure_reason_code),
        suggested_blocked_reason_code: Some(claim.suggested_blocked_reason_code),
        artifact_checksum_sha256: claim.artifact_checksum_sha256,
        crate_name: claim.crate_name,
    }
}
