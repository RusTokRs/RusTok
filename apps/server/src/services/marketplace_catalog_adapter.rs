use std::collections::BTreeMap;

use async_trait::async_trait;
use rustok_api::{
    MarketplaceModule, MarketplaceModuleVersion, MarketplaceRegistryFreshness,
    MarketplaceRegistryStatus, ModuleSettingField, RegistryAutomatedCheckLifecycle,
    RegistryFollowUpGateLifecycle, RegistryGovernanceActionLifecycle,
    RegistryGovernanceEventLifecycle, RegistryGovernanceEventPayloadLifecycle,
    RegistryModerationPolicyLifecycle, RegistryModuleLifecycle, RegistryOwnerLifecycle,
    RegistryOwnerTransitionLifecycle, RegistryPublishRequestLifecycle, RegistryReleaseLifecycle,
    RegistryValidationStageLifecycle,
};
use rustok_core::ModuleRegistry;
use rustok_modules::{
    MODULE_MARKETPLACE_MAX_LIMIT, ModuleMarketplaceCatalog, ModuleMarketplaceEntry,
    ModuleMarketplaceError, ModuleMarketplaceQuery,
    ModuleMarketplaceVersion as OwnerMarketplaceModuleVersion, normalize_module_marketplace_slug,
};
use semver::{Version, VersionReq};

use crate::modules::{CatalogManifestModule, ManifestManager};
use crate::services::marketplace_catalog::{
    MarketplaceCatalogQuery, MarketplaceProviderHealthStatus, marketplace_catalog_from_context,
};
use crate::services::platform_composition::PlatformCompositionService;
use crate::services::registry_principal::RegistryPrincipalRef;
use crate::services::server_runtime_context::ServerRuntimeContext;

#[derive(Clone)]
pub struct ServerMarketplaceCatalog {
    runtime: ServerRuntimeContext,
    registry: ModuleRegistry,
}

impl ServerMarketplaceCatalog {
    pub fn new(runtime: ServerRuntimeContext, registry: ModuleRegistry) -> Self {
        Self { runtime, registry }
    }

    async fn projected_entries(
        &self,
        query: &ModuleMarketplaceQuery,
    ) -> Result<Vec<ModuleMarketplaceEntry>, ModuleMarketplaceError> {
        let manifest = PlatformCompositionService::active_manifest(self.runtime.db())
            .await
            .map_err(|_| ModuleMarketplaceError::Unavailable)?;
        let provider_query = MarketplaceCatalogQuery {
            search: query.search.clone(),
            category: query.category.clone(),
            tag: query.tag.clone(),
        };
        project_marketplace_catalog_entries(
            &self.runtime,
            &manifest,
            &self.registry,
            &provider_query,
            query.preferred_locale.as_deref(),
            query.fallback_locale.as_deref(),
        )
        .await
    }
}

pub(crate) async fn project_marketplace_catalog_entries(
    runtime: &ServerRuntimeContext,
    manifest: &crate::modules::ModulesManifest,
    registry: &ModuleRegistry,
    query: &MarketplaceCatalogQuery,
    preferred_locale: Option<&str>,
    fallback_locale: Option<&str>,
) -> Result<Vec<ModuleMarketplaceEntry>, ModuleMarketplaceError> {
    let installed = ManifestManager::installed_modules(manifest);
    let modules = marketplace_catalog_from_context(runtime)
        .list_modules(manifest, registry, query)
        .await
        .map_err(|_| ModuleMarketplaceError::Unavailable)?;
    let entries = modules
        .into_iter()
        .map(|module| map_catalog_entry(module, registry, &installed))
        .collect::<Result<Vec<_>, _>>()?;
    rustok_modules::ModuleControlPlane::new(runtime.db_clone())
        .release()
        .apply_marketplace_projection(entries, preferred_locale, fallback_locale)
        .await
        .map_err(|_| ModuleMarketplaceError::Unavailable)
}

#[async_trait]
impl ModuleMarketplaceCatalog for ServerMarketplaceCatalog {
    async fn list(
        &self,
        query: ModuleMarketplaceQuery,
    ) -> Result<Vec<MarketplaceModule>, ModuleMarketplaceError> {
        let modules = self.projected_entries(&query).await?;
        let source = normalized_filter(query.source.as_deref());
        let trust_level = normalized_filter(query.trust_level.as_deref());
        let limit = query.limit.clamp(1, MODULE_MARKETPLACE_MAX_LIMIT) as usize;
        let mut entries = Vec::new();
        for entry in modules {
            if entry.kind != "optional"
                || (query.only_compatible && !entry.compatible && !entry.installed)
                || (query.installed_only && !entry.installed)
                || source
                    .as_ref()
                    .is_some_and(|value| !entry.source.eq_ignore_ascii_case(value))
                || trust_level
                    .as_ref()
                    .is_some_and(|value| !entry.trust_level.eq_ignore_ascii_case(value))
            {
                continue;
            }
            entries.push(entry);
            if entries.len() == limit {
                break;
            }
        }
        entries.into_iter().map(map_marketplace_entry).collect()
    }

    async fn get(
        &self,
        slug: &str,
        preferred_locale: Option<String>,
        fallback_locale: Option<String>,
    ) -> Result<Option<MarketplaceModule>, ModuleMarketplaceError> {
        let slug =
            normalize_module_marketplace_slug(slug).ok_or(ModuleMarketplaceError::InvalidQuery)?;
        let manifest = PlatformCompositionService::active_manifest(self.runtime.db())
            .await
            .map_err(|_| ModuleMarketplaceError::Unavailable)?;
        let installed = ManifestManager::installed_modules(&manifest);
        let provider_query = MarketplaceCatalogQuery::default();
        let Some(module) = marketplace_catalog_from_context(&self.runtime)
            .get_module(&manifest, &self.registry, &provider_query, &slug)
            .await
            .map_err(|_| ModuleMarketplaceError::Unavailable)?
        else {
            return Ok(None);
        };
        let entry = map_catalog_entry(module, &self.registry, &installed)?;
        let mut entry = rustok_modules::ModuleControlPlane::new(self.runtime.db_clone())
            .release()
            .apply_marketplace_projection(
                vec![entry],
                preferred_locale.as_deref(),
                fallback_locale.as_deref(),
            )
            .await
            .map_err(|_| ModuleMarketplaceError::Unavailable)?
            .pop()
            .ok_or(ModuleMarketplaceError::InvalidContract)?;
        if entry.kind != "optional" {
            return Ok(None);
        }
        entry.registry_lifecycle = rustok_modules::ModuleControlPlane::new(self.runtime.db_clone())
            .release()
            .lifecycle_snapshot(&entry.slug)
            .await
            .map_err(|_| ModuleMarketplaceError::Unavailable)?;
        map_marketplace_entry(entry).map(Some)
    }

    fn registry_freshness(&self) -> Vec<MarketplaceRegistryFreshness> {
        marketplace_catalog_from_context(&self.runtime)
            .provider_health()
            .into_iter()
            .filter_map(map_registry_freshness)
            .collect()
    }
}

/// Maps one owner-private marketplace entry to the browser-safe contract once,
/// before GraphQL or native Admin obtains it.
fn map_marketplace_entry(
    entry: ModuleMarketplaceEntry,
) -> Result<MarketplaceModule, ModuleMarketplaceError> {
    Ok(MarketplaceModule {
        slug: entry.slug,
        name: entry.name,
        latest_version: entry.latest_version,
        description: entry.description,
        source: entry.source,
        kind: entry.kind,
        category: entry.category,
        tags: entry.tags,
        icon_url: entry.icon_url,
        banner_url: entry.banner_url,
        screenshots: entry.screenshots,
        crate_name: entry.crate_name,
        dependencies: entry.dependencies,
        ownership: entry.ownership,
        trust_level: entry.trust_level,
        rustok_min_version: entry.rustok_min_version,
        rustok_max_version: entry.rustok_max_version,
        publisher: entry.publisher,
        checksum_sha256: entry.checksum_sha256,
        signature_present: entry.signature_present,
        versions: entry
            .versions
            .into_iter()
            .map(|version| MarketplaceModuleVersion {
                version: version.version,
                changelog: version.changelog,
                yanked: version.yanked,
                published_at: version.published_at,
                checksum_sha256: version.checksum_sha256,
                signature_present: version.signature_present,
            })
            .collect(),
        has_admin_ui: entry.has_admin_ui,
        has_storefront_ui: entry.has_storefront_ui,
        ui_classification: entry.ui_classification,
        registry_lifecycle: entry.registry_lifecycle.map(map_registry_lifecycle),
        compatible: entry.compatible,
        recommended_admin_surfaces: entry.recommended_admin_surfaces,
        showcase_admin_surfaces: entry.showcase_admin_surfaces,
        settings_schema: map_setting_fields(entry.settings_schema)?,
        installed: entry.installed,
        installed_version: entry.installed_version,
        update_available: entry.update_available,
    })
}

fn map_registry_lifecycle(
    snapshot: rustok_modules::ModuleGovernanceLifecycleSnapshot,
) -> RegistryModuleLifecycle {
    RegistryModuleLifecycle {
        moderation_policy: RegistryModerationPolicyLifecycle {
            mode: snapshot.moderation_policy.mode,
            live_publish_supported: snapshot.moderation_policy.live_publish_supported,
            live_governance_supported: snapshot.moderation_policy.live_governance_supported,
            manual_review_required: snapshot.moderation_policy.manual_review_required,
            restriction_reason_code: snapshot.moderation_policy.restriction_reason_code,
            restriction_reason: snapshot.moderation_policy.restriction_reason,
        },
        owner_binding: snapshot.owner_binding.map(|owner| RegistryOwnerLifecycle {
            owner: principal_label(&owner.owner_principal),
            bound_by: principal_label(&owner.bound_by_principal),
            bound_at: owner.bound_at,
            updated_at: owner.updated_at,
        }),
        latest_request: snapshot
            .latest_request
            .map(|request| RegistryPublishRequestLifecycle {
                id: request.id,
                revision: request.revision,
                status: request.status,
                requested_by: principal_label(&request.requested_by_principal),
                publisher: optional_principal_label(request.publisher_principal.as_ref()),
                approved_by: optional_principal_label(request.approved_by_principal.as_ref()),
                rejected_by: optional_principal_label(request.rejected_by_principal.as_ref()),
                rejection_reason: request.rejection_reason,
                changes_requested_by: optional_principal_label(
                    request.changes_requested_by_principal.as_ref(),
                ),
                changes_requested_reason: request.changes_requested_reason,
                changes_requested_reason_code: request.changes_requested_reason_code,
                changes_requested_at: request.changes_requested_at,
                held_by: optional_principal_label(request.held_by_principal.as_ref()),
                held_reason: request.held_reason,
                held_reason_code: request.held_reason_code,
                held_at: request.held_at,
                held_from_status: request.held_from_status,
                warnings: request.warnings,
                errors: request.errors,
                created_at: request.created_at,
                updated_at: request.updated_at,
                published_at: request.published_at,
            }),
        latest_release: snapshot
            .latest_release
            .map(|release| RegistryReleaseLifecycle {
                version: release.version,
                status: release.status,
                publisher: principal_label(&release.publisher_principal),
                checksum_sha256: release.checksum_sha256,
                published_at: release.published_at,
                yanked_reason: release.yanked_reason,
                yanked_by: optional_principal_label(release.yanked_by_principal.as_ref()),
                yanked_at: release.yanked_at,
            }),
        recent_events: snapshot
            .recent_events
            .into_iter()
            .map(|event| RegistryGovernanceEventLifecycle {
                id: event.id,
                event_type: event.event_type,
                actor: principal_label(&event.actor_principal),
                publisher: optional_principal_label(event.publisher_principal.as_ref()),
                payload: map_registry_event_payload(event.payload),
                created_at: event.created_at,
            })
            .collect(),
        follow_up_gates: snapshot
            .follow_up_gates
            .into_iter()
            .map(map_registry_follow_up_gate)
            .collect(),
        validation_stages: snapshot
            .validation_stages
            .into_iter()
            .map(map_registry_validation_stage)
            .collect(),
        governance_actions: snapshot
            .governance_actions
            .into_iter()
            .map(map_registry_governance_action)
            .collect(),
    }
}

fn map_registry_event_payload(
    payload: rustok_modules::ModuleGovernanceEventPayload,
) -> RegistryGovernanceEventPayloadLifecycle {
    RegistryGovernanceEventPayloadLifecycle {
        reason: payload.reason,
        reason_code: payload.reason_code,
        detail: payload.detail,
        version: payload.version,
        stage_key: payload.stage_key,
        attempt_number: payload.attempt_number,
        owner_transition: payload.owner_transition.map(|transition| {
            RegistryOwnerTransitionLifecycle {
                previous_owner: optional_principal_label(
                    transition.previous_owner_principal.as_ref(),
                ),
                new_owner: optional_principal_label(transition.new_owner_principal.as_ref()),
                bound_by: optional_principal_label(transition.bound_by_principal.as_ref()),
            }
        }),
        warnings: payload.warnings,
        errors: payload.errors,
        mode: payload.mode,
        automated_checks: payload
            .automated_checks
            .into_iter()
            .map(|check| RegistryAutomatedCheckLifecycle {
                key: check.key,
                status: check.status,
                detail: check.detail,
            })
            .collect(),
    }
}

pub(crate) fn map_registry_follow_up_gate(
    gate: rustok_modules::ModuleGovernanceGateSnapshot,
) -> RegistryFollowUpGateLifecycle {
    RegistryFollowUpGateLifecycle {
        key: gate.key,
        status: gate.status,
        detail: gate.detail,
        updated_at: gate.updated_at,
    }
}

pub(crate) fn map_registry_validation_stage(
    stage: rustok_modules::ModuleGovernanceValidationStageSnapshot,
) -> RegistryValidationStageLifecycle {
    RegistryValidationStageLifecycle {
        key: stage.key,
        status: stage.status,
        detail: stage.detail,
        attempt_number: stage.attempt_number,
        updated_at: stage.updated_at,
        started_at: stage.started_at,
        finished_at: stage.finished_at,
        execution_mode: stage.execution_mode,
        runnable: stage.runnable,
        requires_manual_confirmation: stage.requires_manual_confirmation,
        allowed_terminal_reason_codes: stage.allowed_terminal_reason_codes,
        suggested_pass_reason_code: stage.suggested_pass_reason_code,
        suggested_failure_reason_code: stage.suggested_failure_reason_code,
        suggested_blocked_reason_code: stage.suggested_blocked_reason_code,
    }
}

pub(crate) fn map_registry_governance_action(
    action: rustok_modules::ModuleGovernanceAction,
) -> RegistryGovernanceActionLifecycle {
    RegistryGovernanceActionLifecycle {
        key: action.key,
        reason_required: action.reason_required,
        reason_code_required: action.reason_code_required,
        reason_codes: action.reason_codes,
        destructive: action.destructive,
    }
}

#[cfg(test)]
#[test]
fn validation_stage_mapping_preserves_execution_policy() {
    let stage =
        map_registry_validation_stage(rustok_modules::ModuleGovernanceValidationStageSnapshot {
            key: "artifact_scan".to_string(),
            status: "queued".to_string(),
            detail: "Awaiting the remote runner".to_string(),
            attempt_number: 3,
            updated_at: "2026-09-09T10:00:00Z".to_string(),
            started_at: Some("2026-09-09T09:55:00Z".to_string()),
            finished_at: None,
            execution_mode: "remote".to_string(),
            runnable: true,
            requires_manual_confirmation: true,
            allowed_terminal_reason_codes: vec!["malware".to_string()],
            suggested_pass_reason_code: Some("verified".to_string()),
            suggested_failure_reason_code: Some("malware".to_string()),
            suggested_blocked_reason_code: Some("runner_unavailable".to_string()),
        });

    assert_eq!(stage.execution_mode, "remote");
    assert!(stage.runnable);
    assert!(stage.requires_manual_confirmation);
    assert_eq!(stage.allowed_terminal_reason_codes, ["malware"]);
    assert_eq!(
        stage.suggested_pass_reason_code.as_deref(),
        Some("verified")
    );
    assert_eq!(
        stage.suggested_failure_reason_code.as_deref(),
        Some("malware")
    );
    assert_eq!(
        stage.suggested_blocked_reason_code.as_deref(),
        Some("runner_unavailable")
    );
}

#[cfg(test)]
#[test]
fn governance_event_payload_mapping_preserves_automated_checks() {
    let payload = map_registry_event_payload(rustok_modules::ModuleGovernanceEventPayload {
        automated_checks: vec![rustok_modules::ModuleGovernanceAutomatedCheck {
            key: "artifact_contract".to_string(),
            status: "passed".to_string(),
            detail: Some("Artifact contract validation passed.".to_string()),
        }],
        ..Default::default()
    });

    assert_eq!(payload.automated_checks.len(), 1);
    assert_eq!(payload.automated_checks[0].key, "artifact_contract");
    assert_eq!(payload.automated_checks[0].status, "passed");
    assert_eq!(
        payload.automated_checks[0].detail.as_deref(),
        Some("Artifact contract validation passed.")
    );
}

fn principal_label(value: &serde_json::Value) -> String {
    RegistryPrincipalRef::from_json_value(value).display_label
}

fn optional_principal_label(value: Option<&serde_json::Value>) -> Option<String> {
    value.map(principal_label)
}

fn map_setting_fields(
    schema: BTreeMap<String, rustok_modules::ModuleSettingSpec>,
) -> Result<Vec<ModuleSettingField>, ModuleMarketplaceError> {
    schema
        .into_iter()
        .map(|(key, spec)| {
            let object_keys = if spec.properties.is_empty() {
                spec.object_keys.clone()
            } else {
                spec.properties.keys().cloned().collect()
            };
            let item_type = spec
                .items
                .as_deref()
                .map(|item| item.value_type.trim().to_string())
                .filter(|value| !value.is_empty())
                .or(spec.item_type.clone());
            let mut shape = serde_json::Map::new();
            if !spec.properties.is_empty() {
                shape.insert(
                    "properties".to_string(),
                    serde_json::to_value(&spec.properties)
                        .map_err(|_| ModuleMarketplaceError::InvalidContract)?,
                );
            }
            if let Some(items) = spec.items.as_deref() {
                shape.insert(
                    "items".to_string(),
                    serde_json::to_value(items)
                        .map_err(|_| ModuleMarketplaceError::InvalidContract)?,
                );
            }
            Ok(ModuleSettingField {
                key,
                value_type: spec.value_type,
                required: spec.required,
                default_value: spec.default,
                description: spec.description,
                min: spec.min,
                max: spec.max,
                options: spec.options,
                object_keys,
                item_type,
                shape: (!shape.is_empty()).then_some(serde_json::Value::Object(shape)),
            })
        })
        .collect()
}

fn map_registry_freshness(
    snapshot: crate::services::marketplace_catalog::MarketplaceProviderHealthSnapshot,
) -> Option<MarketplaceRegistryFreshness> {
    if snapshot.provider == "local-manifest" {
        return None;
    }
    let status = match snapshot.status {
        MarketplaceProviderHealthStatus::Ready => MarketplaceRegistryStatus::Ready,
        MarketplaceProviderHealthStatus::Degraded => MarketplaceRegistryStatus::Degraded,
        MarketplaceProviderHealthStatus::Unknown => MarketplaceRegistryStatus::Unknown,
        MarketplaceProviderHealthStatus::Disabled => return None,
    };
    Some(MarketplaceRegistryFreshness {
        registry_id: snapshot.provider,
        status,
        last_success_unix_ms: snapshot.last_success_unix_ms,
        consecutive_failures: snapshot.consecutive_failures,
    })
}

fn map_catalog_entry(
    entry: CatalogManifestModule,
    registry: &ModuleRegistry,
    installed_modules: &[crate::modules::InstalledManifestModule],
) -> Result<ModuleMarketplaceEntry, ModuleMarketplaceError> {
    let catalog_version_fallback = entry
        .versions
        .first()
        .map(|version| version.version.clone());
    let compatible = is_compatible(&entry);
    let signature_present = entry.signature.is_some();
    let runtime_module = registry.get(&entry.slug);
    let installed_module = installed_modules
        .iter()
        .find(|module| module.slug == entry.slug);
    let latest_version = runtime_module
        .map(|module| module.version().to_string())
        .or_else(|| entry.version.clone())
        .or(catalog_version_fallback)
        .unwrap_or_else(|| "workspace".to_string());
    let installed_version = installed_module.and_then(|module| module.version.clone());
    let dependencies = runtime_module
        .map(|module| {
            module
                .dependencies()
                .iter()
                .map(|dependency| dependency.to_string())
                .collect::<Vec<_>>()
        })
        .unwrap_or_else(|| entry.depends_on.clone());
    let versions = if entry.versions.is_empty() {
        vec![OwnerMarketplaceModuleVersion {
            version: latest_version.clone(),
            changelog: None,
            yanked: false,
            published_at: None,
            checksum_sha256: entry.checksum_sha256.clone(),
            signature_present,
            artifact: None,
        }]
    } else {
        entry
            .versions
            .iter()
            .map(|version| OwnerMarketplaceModuleVersion {
                version: version.version.clone(),
                changelog: version.changelog.clone(),
                yanked: version.yanked,
                published_at: version.published_at.clone(),
                checksum_sha256: version.checksum_sha256.clone(),
                signature_present: version.signature.is_some(),
                artifact: version.artifact.clone(),
            })
            .collect()
    };
    let settings_schema = serde_json::from_value(
        serde_json::to_value(entry.settings_schema)
            .map_err(|_| ModuleMarketplaceError::InvalidContract)?,
    )
    .map_err(|_| ModuleMarketplaceError::InvalidContract)?;

    Ok(ModuleMarketplaceEntry {
        slug: entry.slug.clone(),
        name: entry
            .name
            .or_else(|| runtime_module.map(|module| module.name().to_string()))
            .unwrap_or_else(|| humanize_slug(&entry.slug)),
        latest_version: latest_version.clone(),
        description: entry
            .description
            .or_else(|| runtime_module.map(|module| module.description().to_string()))
            .unwrap_or_else(|| {
                format!(
                    "{} module from {} source",
                    humanize_slug(&entry.slug),
                    entry.source
                )
            }),
        source: entry.source,
        kind: if entry.required || registry.is_core(&entry.slug) {
            "core".to_string()
        } else {
            "optional".to_string()
        },
        category: entry
            .category
            .unwrap_or_else(|| fallback_category(&entry.slug).to_string()),
        tags: entry.tags,
        icon_url: entry.icon_url,
        banner_url: entry.banner_url,
        screenshots: entry.screenshots,
        crate_name: entry.crate_name,
        dependencies,
        ownership: entry.ownership,
        trust_level: entry.trust_level,
        rustok_min_version: entry.rustok_min_version,
        rustok_max_version: entry.rustok_max_version,
        publisher: entry.publisher,
        checksum_sha256: entry.checksum_sha256,
        signature_present,
        versions,
        has_admin_ui: entry.has_admin_ui,
        has_storefront_ui: entry.has_storefront_ui,
        ui_classification: entry.ui_classification,
        registry_lifecycle: None,
        compatible,
        recommended_admin_surfaces: entry.recommended_admin_surfaces,
        showcase_admin_surfaces: entry.showcase_admin_surfaces,
        settings_schema,
        installed: installed_module.is_some(),
        installed_version: installed_version.clone(),
        update_available: installed_version
            .as_ref()
            .is_some_and(|version| version != &latest_version),
    })
}

fn normalized_filter(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_ascii_lowercase)
}

fn is_compatible(entry: &CatalogManifestModule) -> bool {
    let Ok(platform_version) = Version::parse(env!("CARGO_PKG_VERSION")) else {
        return false;
    };
    let min_ok = entry
        .rustok_min_version
        .as_deref()
        .and_then(|raw| VersionReq::parse(&normalize_version_req(raw, false)).ok())
        .is_none_or(|req| req.matches(&platform_version));
    let max_ok = entry
        .rustok_max_version
        .as_deref()
        .and_then(|raw| VersionReq::parse(&normalize_version_req(raw, true)).ok())
        .is_none_or(|req| req.matches(&platform_version));
    min_ok && max_ok
}

fn normalize_version_req(value: &str, is_max: bool) -> String {
    let wildcard = value.trim().replace(".x", ".*").replace(".X", ".*");
    let has_operator = wildcard.contains('<')
        || wildcard.contains('>')
        || wildcard.contains('=')
        || wildcard.contains('~')
        || wildcard.contains('^')
        || wildcard.contains('*')
        || wildcard.contains(',');
    if has_operator {
        wildcard
    } else if is_max {
        format!("<= {wildcard}")
    } else {
        format!(">= {wildcard}")
    }
}

fn humanize_slug(slug: &str) -> String {
    slug.split('-')
        .map(|part| {
            let mut chars = part.chars();
            chars
                .next()
                .map(|first| first.to_uppercase().collect::<String>() + chars.as_str())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn fallback_category(slug: &str) -> &'static str {
    match slug {
        "content" | "blog" | "forum" | "pages" => "content",
        "commerce" | "pricing" | "product" | "inventory" => "commerce",
        "tenant" | "rbac" | "index" | "outbox" => "platform",
        _ => "extensions",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::marketplace_catalog::MarketplaceProviderHealthSnapshot;

    fn snapshot(
        provider: &str,
        status: MarketplaceProviderHealthStatus,
    ) -> MarketplaceProviderHealthSnapshot {
        MarketplaceProviderHealthSnapshot {
            provider: provider.to_string(),
            status,
            last_success_unix_ms: Some(1_725_000_000_000),
            consecutive_failures: 3,
        }
    }

    #[test]
    fn registry_projection_excludes_non_remote_sources() {
        assert!(
            map_registry_freshness(snapshot(
                "local-manifest",
                MarketplaceProviderHealthStatus::Ready
            ))
            .is_none()
        );
        assert!(
            map_registry_freshness(snapshot(
                "disabled",
                MarketplaceProviderHealthStatus::Disabled
            ))
            .is_none()
        );
    }

    #[test]
    fn registry_projection_preserves_bounded_operator_evidence() {
        let projected = map_registry_freshness(snapshot(
            "community.eu",
            MarketplaceProviderHealthStatus::Degraded,
        ))
        .expect("configured registry");

        assert_eq!(projected.registry_id, "community.eu");
        assert_eq!(projected.status, MarketplaceRegistryStatus::Degraded);
        assert_eq!(projected.last_success_unix_ms, Some(1_725_000_000_000));
        assert_eq!(projected.consecutive_failures, 3);
    }
}
