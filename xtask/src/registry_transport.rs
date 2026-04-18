use super::*;

pub(crate) fn build_publish_registry_request(
    preview: &ModulePublishDryRunPreview,
) -> RegistryPublishHttpRequest {
    build_publish_registry_request_with_dry_run(preview, true)
}

pub(crate) fn build_live_publish_registry_request(
    preview: &ModulePublishDryRunPreview,
) -> RegistryPublishHttpRequest {
    build_publish_registry_request_with_dry_run(preview, false)
}

fn build_publish_registry_request_with_dry_run(
    preview: &ModulePublishDryRunPreview,
    dry_run: bool,
) -> RegistryPublishHttpRequest {
    RegistryPublishHttpRequest {
        schema_version: REGISTRY_MUTATION_SCHEMA_VERSION,
        dry_run,
        module: RegistryPublishModuleHttpRequest {
            slug: preview.slug.clone(),
            version: preview.version.clone(),
            crate_name: preview.crate_name.clone(),
            default_locale: preview.module_default_locale.clone(),
            name: preview.module_name.clone(),
            description: preview.module_description.clone(),
            ownership: preview.ownership.clone(),
            trust_level: preview.trust_level.clone(),
            license: preview.license.clone(),
            entry_type: preview.module_entry_type.clone(),
            marketplace: RegistryPublishMarketplaceHttpRequest {
                category: preview.marketplace.category.clone(),
                tags: preview.marketplace.tags.clone(),
            },
            ui_packages: RegistryPublishUiPackagesHttpRequest {
                admin: preview.ui_packages.admin.as_ref().map(|ui| {
                    RegistryPublishUiPackageHttpRequest {
                        crate_name: ui.crate_name.clone(),
                    }
                }),
                storefront: preview.ui_packages.storefront.as_ref().map(|ui| {
                    RegistryPublishUiPackageHttpRequest {
                        crate_name: ui.crate_name.clone(),
                    }
                }),
            },
        },
    }
}

pub(crate) fn build_yank_registry_request(
    preview: &ModuleYankDryRunPreview,
    reason: Option<String>,
    reason_code: Option<String>,
) -> RegistryYankHttpRequest {
    build_yank_registry_request_with_dry_run(preview, reason, reason_code, true)
}

pub(crate) fn build_live_yank_registry_request(
    preview: &ModuleYankDryRunPreview,
    reason: Option<String>,
    reason_code: Option<String>,
) -> RegistryYankHttpRequest {
    build_yank_registry_request_with_dry_run(preview, reason, reason_code, false)
}

fn build_yank_registry_request_with_dry_run(
    preview: &ModuleYankDryRunPreview,
    reason: Option<String>,
    reason_code: Option<String>,
    dry_run: bool,
) -> RegistryYankHttpRequest {
    RegistryYankHttpRequest {
        schema_version: REGISTRY_MUTATION_SCHEMA_VERSION,
        dry_run,
        slug: preview.slug.clone(),
        version: preview.version.clone(),
        reason,
        reason_code,
    }
}

pub(crate) fn build_owner_transfer_registry_request(
    preview: &ModuleOwnerTransferDryRunPreview,
    reason: Option<String>,
    reason_code: Option<String>,
) -> RegistryOwnerTransferHttpRequest {
    build_owner_transfer_registry_request_with_dry_run(preview, reason, reason_code, true)
}

pub(crate) fn build_live_owner_transfer_registry_request(
    preview: &ModuleOwnerTransferDryRunPreview,
    reason: Option<String>,
    reason_code: Option<String>,
) -> RegistryOwnerTransferHttpRequest {
    build_owner_transfer_registry_request_with_dry_run(preview, reason, reason_code, false)
}

fn build_owner_transfer_registry_request_with_dry_run(
    preview: &ModuleOwnerTransferDryRunPreview,
    reason: Option<String>,
    reason_code: Option<String>,
    dry_run: bool,
) -> RegistryOwnerTransferHttpRequest {
    RegistryOwnerTransferHttpRequest {
        schema_version: REGISTRY_MUTATION_SCHEMA_VERSION,
        dry_run,
        slug: preview.slug.clone(),
        new_owner_user_id: preview.new_owner_user_id.clone(),
        reason,
        reason_code,
    }
}

pub(crate) fn build_validation_stage_registry_request(
    preview: &ModuleValidationStageDryRunPreview,
) -> RegistryValidationStageHttpRequest {
    build_validation_stage_registry_request_with_dry_run(preview, true)
}

pub(crate) fn build_live_validation_stage_registry_request(
    preview: &ModuleValidationStageDryRunPreview,
) -> RegistryValidationStageHttpRequest {
    build_validation_stage_registry_request_with_dry_run(preview, false)
}

fn build_validation_stage_registry_request_with_dry_run(
    preview: &ModuleValidationStageDryRunPreview,
    dry_run: bool,
) -> RegistryValidationStageHttpRequest {
    RegistryValidationStageHttpRequest {
        schema_version: REGISTRY_MUTATION_SCHEMA_VERSION,
        dry_run,
        stage: preview.stage.clone(),
        status: preview.status.clone(),
        detail: preview.detail.clone(),
        reason_code: preview.reason_code.clone(),
        requeue: preview.requeue,
    }
}
