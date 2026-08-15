use fly_ui::CapabilityState;
use rustok_page_builder::health::ProviderHealthSnapshot;
use rustok_page_builder::rollout::BuilderCapabilityFlags;
use rustok_page_builder_admin::PageBuilderAdminProviderStatus;

#[derive(Debug, Clone, PartialEq)]
pub struct PagesBuilderRolloutSnapshot {
    pub flags: BuilderCapabilityFlags,
    pub tenant_slug: String,
    pub provider_health: Option<ProviderHealthSnapshot>,
}

impl PagesBuilderRolloutSnapshot {
    pub fn provider_status(&self) -> PageBuilderAdminProviderStatus {
        match self.provider_health.clone() {
            Some(health) => PageBuilderAdminProviderStatus::observed(self.flags.clone(), health),
            None => PageBuilderAdminProviderStatus::unobserved(self.flags.clone()),
        }
    }

    pub fn effective_runtime_flags(&self) -> BuilderCapabilityFlags {
        self.provider_status().effective_runtime_flags()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PagesBuilderRolloutSnapshotError {
    #[error("Pages builder rollout tenant is missing")]
    MissingTenant,
    #[error("Pages builder rollout transport failed: {0}")]
    Transport(String),
    #[error("Pages builder rollout tenant `{requested}` does not match routed tenant `{routed}`")]
    TenantMismatch { requested: String, routed: String },
}

pub async fn fetch_pages_builder_rollout_snapshot(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<PagesBuilderRolloutSnapshot, PagesBuilderRolloutSnapshotError> {
    let requested_tenant = tenant_slug
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .ok_or(PagesBuilderRolloutSnapshotError::MissingTenant)?;
    let (routed_tenant, flags, provider_health) =
        crate::transport::fetch_page_builder_rollout_snapshot(
            token,
            Some(requested_tenant.clone()),
        )
        .await
        .map_err(|error| PagesBuilderRolloutSnapshotError::Transport(error.to_string()))?;
    if routed_tenant != requested_tenant {
        return Err(PagesBuilderRolloutSnapshotError::TenantMismatch {
            requested: requested_tenant,
            routed: routed_tenant,
        });
    }
    Ok(PagesBuilderRolloutSnapshot {
        flags,
        tenant_slug: routed_tenant,
        provider_health,
    })
}

pub fn pages_editor_capabilities_for_rollout(
    capabilities: CapabilityState,
    flags: &BuilderCapabilityFlags,
) -> CapabilityState {
    PageBuilderAdminProviderStatus::unobserved(flags.clone()).limit_capabilities(capabilities)
}

/// Apply the validated provider-health snapshot carried by the server-owned rollout snapshot.
/// Missing health remains explicitly unobserved; observed health may only narrow the configured
/// rollout and the already evaluated host tenant/RBAC capabilities.
pub fn pages_editor_capabilities_for_snapshot(
    capabilities: CapabilityState,
    snapshot: &PagesBuilderRolloutSnapshot,
) -> CapabilityState {
    snapshot.provider_status().limit_capabilities(capabilities)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustok_page_builder::health::ProviderSloObservations;
    use rustok_page_builder::rollout::BuilderToggleProfile;

    #[test]
    fn rollout_profiles_only_narrow_browser_capabilities() {
        let full = CapabilityState::full();
        let publish_off =
            pages_editor_capabilities_for_rollout(full, &BuilderToggleProfile::PublishOff.flags());
        assert!(publish_off.edit);
        assert!(publish_off.properties);
        assert!(!publish_off.publish);

        let preview_off =
            pages_editor_capabilities_for_rollout(full, &BuilderToggleProfile::PreviewOff.flags());
        assert!(preview_off.edit);
        assert!(preview_off.properties);
        assert!(!preview_off.publish);

        let builder_off =
            pages_editor_capabilities_for_rollout(full, &BuilderToggleProfile::BuilderOff.flags());
        assert_eq!(builder_off, CapabilityState::read_only());
    }

    #[test]
    fn validated_observed_snapshot_can_narrow_capabilities_without_changing_rollout_flags() {
        let health = ProviderHealthSnapshot::evaluate(ProviderSloObservations {
            preview_p95_ms: 1_600,
            publish_p95_ms: 2_000,
            sanitize_failure_rate: 0.0,
            runtime_error_rate: 0.0,
        });
        let snapshot = PagesBuilderRolloutSnapshot {
            flags: BuilderCapabilityFlags::default(),
            tenant_slug: "pages-tenant".to_string(),
            provider_health: Some(health),
        };
        let effective = pages_editor_capabilities_for_snapshot(CapabilityState::full(), &snapshot);
        assert!(effective.edit);
        assert!(effective.properties);
        assert!(!effective.publish);
        assert_eq!(snapshot.flags, BuilderCapabilityFlags::default());
        assert_eq!(
            snapshot.effective_runtime_flags(),
            BuilderToggleProfile::PublishOff.flags()
        );
    }

    #[test]
    fn missing_health_preserves_configured_runtime_flags() {
        let snapshot = PagesBuilderRolloutSnapshot {
            flags: BuilderToggleProfile::PreviewOff.flags(),
            tenant_slug: "pages-tenant".to_string(),
            provider_health: None,
        };
        assert_eq!(
            snapshot.effective_runtime_flags(),
            BuilderToggleProfile::PreviewOff.flags()
        );
    }
}
