use fly_ui::CapabilityState;
use rustok_page_builder::rollout::BuilderCapabilityFlags;
use rustok_page_builder_admin::PageBuilderAdminProviderStatus;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PagesBuilderRolloutSnapshot {
    pub flags: BuilderCapabilityFlags,
    pub tenant_slug: String,
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
    let (routed_tenant, flags) = crate::transport::fetch_page_builder_rollout_snapshot(
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
    })
}

pub fn pages_editor_capabilities_for_rollout(
    capabilities: CapabilityState,
    flags: &BuilderCapabilityFlags,
) -> CapabilityState {
    PageBuilderAdminProviderStatus::unobserved(flags.clone()).limit_capabilities(capabilities)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustok_page_builder::rollout::BuilderToggleProfile;

    #[test]
    fn rollout_profiles_only_narrow_browser_capabilities() {
        let full = CapabilityState::full();
        let publish_off = pages_editor_capabilities_for_rollout(
            full.clone(),
            &BuilderToggleProfile::PublishOff.flags(),
        );
        assert!(publish_off.edit);
        assert!(publish_off.properties);
        assert!(!publish_off.publish);

        let preview_off = pages_editor_capabilities_for_rollout(
            full.clone(),
            &BuilderToggleProfile::PreviewOff.flags(),
        );
        assert!(preview_off.edit);
        assert!(preview_off.properties);
        assert!(!preview_off.publish);

        let builder_off =
            pages_editor_capabilities_for_rollout(full, &BuilderToggleProfile::BuilderOff.flags());
        assert_eq!(builder_off, CapabilityState::read_only());
    }
}
