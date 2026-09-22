use super::*;

#[derive(Debug, Serialize)]
pub(crate) struct ModulePublishGovernanceDryRunPreview {
    pub(crate) action: String,
    pub(crate) request_id: String,
    pub(crate) actor: Option<String>,
    pub(crate) reason: Option<String>,
    pub(crate) reason_code: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct ModuleYankDryRunPreview {
    pub(crate) action: String,
    pub(crate) slug: String,
    pub(crate) version: String,
    pub(crate) crate_name: String,
    pub(crate) current_local_version: String,
    pub(crate) matches_local_version: bool,
    pub(crate) package_manifest_path: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct ModuleOwnerTransferDryRunPreview {
    pub(crate) action: String,
    pub(crate) slug: String,
    pub(crate) crate_name: String,
    pub(crate) current_local_version: String,
    pub(crate) package_manifest_path: String,
    pub(crate) new_owner_user_id: String,
    pub(crate) reason: Option<String>,
    pub(crate) reason_code: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct ModuleValidationStageDryRunPreview {
    pub(crate) action: String,
    pub(crate) request_id: String,
    pub(crate) stage: String,
    pub(crate) status: String,
    pub(crate) detail: Option<String>,
    pub(crate) reason_code: Option<String>,
    pub(crate) requeue: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct ModuleValidationStageRunPreview {
    pub(crate) action: String,
    pub(crate) slug: String,
    pub(crate) request_id: String,
    pub(crate) stage: String,
    pub(crate) requires_manual_confirmation: bool,
    pub(crate) running_detail: String,
    pub(crate) success_detail: String,
    pub(crate) failure_detail_prefix: String,
    pub(crate) commands: Vec<ModuleCommandPreview>,
}

#[derive(Debug, Serialize)]
pub(crate) struct ModuleTestPlanPreview {
    pub(crate) slug: String,
    pub(crate) version: String,
    pub(crate) commands: Vec<ModuleCommandPreview>,
}

#[derive(Debug, Serialize)]
pub(crate) struct ModuleCommandPreview {
    pub(crate) label: String,
    pub(crate) argv: Vec<String>,
}
