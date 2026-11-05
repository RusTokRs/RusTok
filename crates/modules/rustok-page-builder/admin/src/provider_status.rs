use fly_ui::{CapabilityState, EditorProviderState};
use rustok_page_builder::health::{ProviderHealthSnapshot, ProviderHealthState};
use rustok_page_builder::rollout::{BuilderCapabilityFlags, effective_provider_runtime_flags};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageBuilderAdminProviderState {
    Ready,
    Degraded,
    Unavailable,
    Unobserved,
}

impl PageBuilderAdminProviderState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Degraded => "degraded",
            Self::Unavailable => "unavailable",
            Self::Unobserved => "unobserved",
        }
    }
}

/// Admin-facing provider control snapshot supplied by a concrete consumer facade.
///
/// Rollout flags are authoritative for the capability pipeline composed by that facade. Health is
/// optional because a consumer must not fabricate an observed SLO state when no live telemetry
/// snapshot exists. The status can only narrow an already evaluated tenant/RBAC editor profile.
#[derive(Debug, Clone, PartialEq)]
pub struct PageBuilderAdminProviderStatus {
    pub flags: BuilderCapabilityFlags,
    pub health: Option<ProviderHealthSnapshot>,
}

impl PageBuilderAdminProviderStatus {
    pub fn unobserved(flags: BuilderCapabilityFlags) -> Self {
        Self {
            flags,
            health: None,
        }
    }

    pub fn observed(flags: BuilderCapabilityFlags, health: ProviderHealthSnapshot) -> Self {
        Self {
            flags,
            health: Some(health),
        }
    }

    pub fn state(&self) -> PageBuilderAdminProviderState {
        if self.flags.validate().is_err() || !self.flags.builder_enabled {
            return PageBuilderAdminProviderState::Unavailable;
        }
        if self
            .health
            .as_ref()
            .is_some_and(|health| health.state == ProviderHealthState::Unavailable)
        {
            return PageBuilderAdminProviderState::Unavailable;
        }
        if !self.flags.preview_enabled
            || !self.flags.properties_enabled
            || !self.flags.publish_enabled
            || self
                .health
                .as_ref()
                .is_some_and(|health| health.state == ProviderHealthState::Degraded)
        {
            return PageBuilderAdminProviderState::Degraded;
        }
        if self
            .health
            .as_ref()
            .is_some_and(|health| health.state == ProviderHealthState::Ready)
        {
            PageBuilderAdminProviderState::Ready
        } else {
            PageBuilderAdminProviderState::Unobserved
        }
    }

    pub fn editor_provider_state(&self) -> Option<EditorProviderState> {
        match self.state() {
            PageBuilderAdminProviderState::Ready => Some(EditorProviderState::Healthy),
            PageBuilderAdminProviderState::Degraded => Some(EditorProviderState::Degraded),
            PageBuilderAdminProviderState::Unavailable => Some(EditorProviderState::Unavailable),
            PageBuilderAdminProviderState::Unobserved => None,
        }
    }

    pub fn preview_enabled(&self) -> bool {
        self.flags.validate().is_ok()
            && self.flags.builder_enabled
            && self.flags.preview_enabled
            && self.state() != PageBuilderAdminProviderState::Unavailable
    }

    /// Derive runtime guard flags through the shared Page Builder core policy.
    ///
    /// Keeping this method as the admin facade seam preserves the existing UI/consumer API while
    /// preventing Pages or another consumer from reimplementing provider-health narrowing.
    pub fn effective_runtime_flags(&self) -> BuilderCapabilityFlags {
        effective_provider_runtime_flags(&self.flags, self.health.as_ref())
    }

    pub fn limit_capabilities(&self, capabilities: CapabilityState) -> CapabilityState {
        if self.flags.validate().is_err()
            || !self.flags.builder_enabled
            || self.state() == PageBuilderAdminProviderState::Unavailable
        {
            return CapabilityState::read_only();
        }

        let mut effective = capabilities;
        if !self.flags.properties_enabled {
            effective.properties = false;
        }
        if !self.flags.publish_enabled || self.state() == PageBuilderAdminProviderState::Degraded {
            effective.publish = false;
        }
        effective.normalized()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustok_page_builder::health::ProviderSloObservations;
    use rustok_page_builder::rollout::BuilderToggleProfile;

    #[test]
    fn unobserved_all_on_status_does_not_claim_healthy_or_reduce_capabilities() {
        let status = PageBuilderAdminProviderStatus::unobserved(BuilderCapabilityFlags::default());
        assert_eq!(status.state(), PageBuilderAdminProviderState::Unobserved);
        assert_eq!(status.editor_provider_state(), None);
        assert_eq!(
            status.limit_capabilities(CapabilityState::full()),
            CapabilityState::full()
        );
        assert!(status.preview_enabled());
        assert_eq!(
            status.effective_runtime_flags(),
            BuilderCapabilityFlags::default()
        );
    }

    #[test]
    fn rollout_publish_off_is_degraded_and_removes_publish_only() {
        let status =
            PageBuilderAdminProviderStatus::unobserved(BuilderToggleProfile::PublishOff.flags());
        let effective = status.limit_capabilities(CapabilityState::full());
        assert_eq!(status.state(), PageBuilderAdminProviderState::Degraded);
        assert!(effective.edit);
        assert!(effective.properties);
        assert!(!effective.publish);
        assert!(status.preview_enabled());
        assert_eq!(
            status.effective_runtime_flags(),
            BuilderToggleProfile::PublishOff.flags()
        );
    }

    #[test]
    fn rollout_preview_off_is_degraded_and_keeps_properties_without_preview_or_publish() {
        let status =
            PageBuilderAdminProviderStatus::unobserved(BuilderToggleProfile::PreviewOff.flags());
        let effective = status.limit_capabilities(CapabilityState::full());
        assert_eq!(status.state(), PageBuilderAdminProviderState::Degraded);
        assert!(effective.edit);
        assert!(effective.properties);
        assert!(!effective.publish);
        assert!(!status.preview_enabled());
        assert_eq!(
            status.effective_runtime_flags(),
            BuilderToggleProfile::PreviewOff.flags()
        );
    }

    #[test]
    fn rollout_builder_off_is_unavailable_and_forces_read_only() {
        let status =
            PageBuilderAdminProviderStatus::unobserved(BuilderToggleProfile::BuilderOff.flags());
        assert_eq!(status.state(), PageBuilderAdminProviderState::Unavailable);
        assert_eq!(
            status.limit_capabilities(CapabilityState::full()),
            CapabilityState::read_only()
        );
        assert!(!status.preview_enabled());
        assert_eq!(
            status.effective_runtime_flags(),
            BuilderToggleProfile::BuilderOff.flags()
        );
    }

    #[test]
    fn observed_health_degrades_publish_and_unavailable_health_forces_read_only() {
        let degraded = ProviderHealthSnapshot::evaluate(ProviderSloObservations {
            preview_p95_ms: 1_600,
            publish_p95_ms: 1_000,
            sanitize_failure_rate: 0.0,
            runtime_error_rate: 0.0,
        });
        let status =
            PageBuilderAdminProviderStatus::observed(BuilderCapabilityFlags::default(), degraded);
        assert_eq!(status.state(), PageBuilderAdminProviderState::Degraded);
        assert!(!status.limit_capabilities(CapabilityState::full()).publish);
        assert_eq!(
            status.effective_runtime_flags(),
            BuilderToggleProfile::PublishOff.flags()
        );

        let unavailable = ProviderHealthSnapshot::evaluate(ProviderSloObservations {
            preview_p95_ms: 1_000,
            publish_p95_ms: 1_000,
            sanitize_failure_rate: 0.0,
            runtime_error_rate: 0.03,
        });
        let status = PageBuilderAdminProviderStatus::observed(
            BuilderCapabilityFlags::default(),
            unavailable,
        );
        assert_eq!(status.state(), PageBuilderAdminProviderState::Unavailable);
        assert_eq!(
            status.limit_capabilities(CapabilityState::full()),
            CapabilityState::read_only()
        );
        assert_eq!(
            status.effective_runtime_flags(),
            BuilderToggleProfile::BuilderOff.flags()
        );
    }
}
