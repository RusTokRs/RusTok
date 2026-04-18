use super::*;

#[derive(Debug, Serialize)]
pub(crate) struct RegistryPublishHttpRequest {
    pub(crate) schema_version: u32,
    pub(crate) dry_run: bool,
    pub(crate) module: RegistryPublishModuleHttpRequest,
}

#[derive(Debug, Serialize)]
pub(crate) struct RegistryPublishModuleHttpRequest {
    pub(crate) slug: String,
    pub(crate) version: String,
    pub(crate) crate_name: String,
    pub(crate) default_locale: String,
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) ownership: String,
    pub(crate) trust_level: String,
    pub(crate) license: String,
    pub(crate) entry_type: Option<String>,
    pub(crate) marketplace: RegistryPublishMarketplaceHttpRequest,
    pub(crate) ui_packages: RegistryPublishUiPackagesHttpRequest,
}

#[derive(Debug, Serialize)]
pub(crate) struct RegistryPublishMarketplaceHttpRequest {
    pub(crate) category: Option<String>,
    pub(crate) tags: Vec<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct RegistryPublishUiPackagesHttpRequest {
    pub(crate) admin: Option<RegistryPublishUiPackageHttpRequest>,
    pub(crate) storefront: Option<RegistryPublishUiPackageHttpRequest>,
}

#[derive(Debug, Serialize)]
pub(crate) struct RegistryPublishUiPackageHttpRequest {
    pub(crate) crate_name: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct RegistryYankHttpRequest {
    pub(crate) schema_version: u32,
    pub(crate) dry_run: bool,
    pub(crate) slug: String,
    pub(crate) version: String,
    pub(crate) reason: Option<String>,
    pub(crate) reason_code: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct RegistryOwnerTransferHttpRequest {
    pub(crate) schema_version: u32,
    pub(crate) dry_run: bool,
    pub(crate) slug: String,
    pub(crate) new_owner_user_id: String,
    pub(crate) reason: Option<String>,
    pub(crate) reason_code: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct RegistryValidationStageHttpRequest {
    pub(crate) schema_version: u32,
    pub(crate) dry_run: bool,
    pub(crate) stage: String,
    pub(crate) status: String,
    pub(crate) detail: Option<String>,
    pub(crate) reason_code: Option<String>,
    pub(crate) requeue: bool,
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct RegistryMutationHttpResponse {
    pub(crate) accepted: bool,
    pub(crate) request_id: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) next_step: Option<String>,
    #[serde(default)]
    pub(crate) errors: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct RegistryPublishStatusHttpResponse {
    pub(crate) request_id: String,
    pub(crate) slug: String,
    pub(crate) version: String,
    pub(crate) status: String,
    pub(crate) accepted: bool,
    #[serde(default)]
    pub(crate) warnings: Vec<String>,
    #[serde(default)]
    pub(crate) errors: Vec<String>,
    #[serde(default)]
    pub(crate) follow_up_gates: Vec<RegistryPublishStatusFollowUpGate>,
    #[serde(default)]
    pub(crate) validation_stages: Vec<RegistryPublishStatusValidationStage>,
    #[serde(default)]
    pub(crate) approval_override_required: bool,
    #[serde(default)]
    pub(crate) approval_override_reason_codes: Vec<String>,
    #[serde(default)]
    pub(crate) governance_actions: Vec<RegistryGovernanceActionHttpResponse>,
    pub(crate) next_step: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct RegistryGovernanceActionHttpResponse {
    pub(crate) key: String,
    pub(crate) reason_required: bool,
    pub(crate) reason_code_required: bool,
    #[serde(default)]
    pub(crate) reason_codes: Vec<String>,
    #[serde(default)]
    pub(crate) destructive: bool,
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct RegistryPublishStatusFollowUpGate {
    pub(crate) key: String,
    pub(crate) status: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct RegistryPublishStatusValidationStage {
    pub(crate) key: String,
    pub(crate) status: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct RegistryPublishValidationHttpRequest {
    pub(crate) schema_version: u32,
    pub(crate) dry_run: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct RegistryPublishDecisionHttpRequest {
    pub(crate) schema_version: u32,
    pub(crate) dry_run: bool,
    pub(crate) reason: Option<String>,
    pub(crate) reason_code: Option<String>,
}

#[derive(Debug)]
pub(crate) struct ResolvedCargoPackage {
    pub(crate) name: String,
    pub(crate) version: String,
    pub(crate) license: Option<String>,
    pub(crate) manifest_path: PathBuf,
}

pub(crate) const REGISTRY_MUTATION_SCHEMA_VERSION: u32 = 1;
