use crate::dto::BuilderCapabilityKind;
use crate::health::{ProviderHealthSnapshot, ProviderHealthState};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuilderCapabilityFlags {
    pub builder_enabled: bool,
    pub preview_enabled: bool,
    pub properties_enabled: bool,
    pub publish_enabled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BuilderToggleProfile {
    AllOn,
    PublishOff,
    PreviewOff,
    BuilderOff,
}

impl BuilderToggleProfile {
    pub const ALL: [Self; 4] = [
        Self::AllOn,
        Self::PublishOff,
        Self::PreviewOff,
        Self::BuilderOff,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::AllOn => "all_on",
            Self::PublishOff => "publish_off",
            Self::PreviewOff => "preview_off",
            Self::BuilderOff => "builder_off",
        }
    }

    pub fn flags(self) -> BuilderCapabilityFlags {
        match self {
            Self::AllOn => BuilderCapabilityFlags {
                builder_enabled: true,
                preview_enabled: true,
                properties_enabled: true,
                publish_enabled: true,
            },
            Self::PublishOff => BuilderCapabilityFlags {
                builder_enabled: true,
                preview_enabled: true,
                properties_enabled: true,
                publish_enabled: false,
            },
            Self::PreviewOff => BuilderCapabilityFlags {
                builder_enabled: true,
                preview_enabled: false,
                properties_enabled: true,
                publish_enabled: false,
            },
            Self::BuilderOff => BuilderCapabilityFlags {
                builder_enabled: false,
                preview_enabled: false,
                properties_enabled: false,
                publish_enabled: false,
            },
        }
    }

    pub fn fallback_outcome(self) -> BuilderFallbackOutcome {
        match self {
            Self::AllOn => BuilderFallbackOutcome {
                profile: self,
                admin_visual_path: "editable_builder",
                preview: "available",
                properties: "available",
                publish: "available",
                read_paths: "stable",
                disabled_capabilities: &[],
            },
            Self::PublishOff => BuilderFallbackOutcome {
                profile: self,
                admin_visual_path: "editable_builder_publish_disabled",
                preview: "available",
                properties: "available",
                publish: "typed_feature_disabled_error",
                read_paths: "stable",
                disabled_capabilities: &["publish"],
            },
            Self::PreviewOff => BuilderFallbackOutcome {
                profile: self,
                admin_visual_path: "preview_hidden_properties_available",
                preview: "typed_feature_disabled_error",
                properties: "available",
                publish: "typed_feature_disabled_error",
                read_paths: "stable",
                disabled_capabilities: &["preview", "publish"],
            },
            Self::BuilderOff => BuilderFallbackOutcome {
                profile: self,
                admin_visual_path: "readonly_fallback",
                preview: "typed_feature_disabled_error",
                properties: "typed_feature_disabled_error",
                publish: "typed_feature_disabled_error",
                read_paths: "stable",
                disabled_capabilities: &["preview", "tree", "properties", "publish"],
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuilderFallbackOutcome {
    pub profile: BuilderToggleProfile,
    pub admin_visual_path: &'static str,
    pub preview: &'static str,
    pub properties: &'static str,
    pub publish: &'static str,
    pub read_paths: &'static str,
    pub disabled_capabilities: &'static [&'static str],
}

pub fn fallback_matrix() -> [BuilderFallbackOutcome; 4] {
    BuilderToggleProfile::ALL.map(BuilderToggleProfile::fallback_outcome)
}

impl Default for BuilderCapabilityFlags {
    fn default() -> Self {
        Self {
            builder_enabled: true,
            preview_enabled: true,
            properties_enabled: true,
            publish_enabled: true,
        }
    }
}

impl BuilderCapabilityFlags {
    /// Normalizes the shared nested module-settings shape used by Page Builder consumers.
    /// Missing keys preserve the backwards-compatible all-on default. Present values must be
    /// booleans and the resulting combination must satisfy the rollout invariants.
    pub fn from_module_settings(settings: &Value) -> Result<Self, BuilderRolloutError> {
        fn nested_bool(settings: &Value, path: &[&str]) -> Result<bool, BuilderRolloutError> {
            let mut current = settings;
            for segment in path {
                let Value::Object(values) = current else {
                    return Err(BuilderRolloutError::InvalidFlagCombination(format!(
                        "Page Builder rollout setting `{}` must be an object",
                        path.join(".")
                    )));
                };
                let Some(next) = values.get(*segment) else {
                    return Ok(true);
                };
                current = next;
            }
            current.as_bool().ok_or_else(|| {
                BuilderRolloutError::InvalidFlagCombination(format!(
                    "Page Builder rollout setting `{}` must be a boolean",
                    path.join(".")
                ))
            })
        }

        let flags = Self {
            builder_enabled: nested_bool(settings, &["builder", "enabled"])?,
            preview_enabled: nested_bool(settings, &["builder", "preview", "enabled"])?,
            properties_enabled: nested_bool(settings, &["builder", "properties", "enabled"])?,
            publish_enabled: nested_bool(settings, &["builder", "publish", "enabled"])?,
        };
        flags.validate()?;
        Ok(flags)
    }

    pub fn is_allowed(&self, capability: BuilderCapabilityKind) -> bool {
        if !self.builder_enabled {
            return false;
        }

        match capability {
            BuilderCapabilityKind::Preview => self.preview_enabled,
            BuilderCapabilityKind::Tree | BuilderCapabilityKind::Properties => {
                self.properties_enabled
            }
            BuilderCapabilityKind::Publish => self.publish_enabled,
        }
    }

    pub fn validate(&self) -> Result<(), BuilderRolloutError> {
        if self.publish_enabled && !self.preview_enabled {
            return Err(BuilderRolloutError::InvalidFlagCombination(
                "publish_enabled requires preview_enabled".to_string(),
            ));
        }

        if !self.builder_enabled
            && (self.preview_enabled || self.properties_enabled || self.publish_enabled)
        {
            return Err(BuilderRolloutError::InvalidFlagCombination(
                "builder_enabled=false requires preview/properties/publish=false".to_string(),
            ));
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuilderControlPlaneChangeSet {
    pub tenant_id: String,
    pub change_set_id: String,
    pub requested_by: String,
    pub approved_by: Vec<String>,
    pub trace_id: String,
    pub profile: BuilderToggleProfile,
    pub flags_before: BuilderCapabilityFlags,
    pub flags_after: BuilderCapabilityFlags,
    pub rollback_decision: BuilderRollbackDecision,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BuilderRollbackDecision {
    Keep,
    Rollback,
}

impl BuilderControlPlaneChangeSet {
    pub fn dry_run(
        tenant_id: impl Into<String>,
        change_set_id: impl Into<String>,
        requested_by: impl Into<String>,
        approved_by: Vec<String>,
        trace_id: impl Into<String>,
        profile: BuilderToggleProfile,
        flags_before: BuilderCapabilityFlags,
    ) -> Result<Self, BuilderRolloutError> {
        let flags_after = profile.flags();
        flags_before.validate()?;
        flags_after.validate()?;

        Ok(Self {
            tenant_id: tenant_id.into(),
            change_set_id: change_set_id.into(),
            requested_by: requested_by.into(),
            approved_by,
            trace_id: trace_id.into(),
            profile,
            flags_before,
            flags_after,
            rollback_decision: BuilderRollbackDecision::Keep,
        })
    }

    pub fn atomic_flag_keys() -> [&'static str; 4] {
        [
            "builder.enabled",
            "builder.preview.enabled",
            "builder.properties.enabled",
            "builder.publish.enabled",
        ]
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum BuilderRolloutError {
    #[error("capability disabled: {0}")]
    CapabilityDisabled(&'static str),
    #[error("invalid flag combination: {0}")]
    InvalidFlagCombination(String),
}

pub fn ensure_capability(
    flags: &BuilderCapabilityFlags,
    capability: BuilderCapabilityKind,
) -> Result<(), BuilderRolloutError> {
    flags.validate()?;
    if flags.is_allowed(capability) {
        Ok(())
    } else {
        Err(BuilderRolloutError::CapabilityDisabled(capability.as_str()))
    }
}

/// Derive the effective runtime guard flags from configured rollout plus optional provider health.
///
/// Configured rollout remains authoritative and provider state may only narrow it. Invalid or
/// disabled rollout and observed provider unavailability fail closed to builder-off. Any degraded
/// provider-control state disables publish while preserving already-disabled rollout capabilities.
/// Missing health and observed Ready health never grant capabilities beyond configured rollout.
pub fn effective_provider_runtime_flags(
    flags: &BuilderCapabilityFlags,
    health: Option<&ProviderHealthSnapshot>,
) -> BuilderCapabilityFlags {
    if flags.validate().is_err()
        || !flags.builder_enabled
        || health.is_some_and(|snapshot| snapshot.state == ProviderHealthState::Unavailable)
    {
        return BuilderToggleProfile::BuilderOff.flags();
    }

    let degraded = !flags.preview_enabled
        || !flags.properties_enabled
        || !flags.publish_enabled
        || health.is_some_and(|snapshot| snapshot.state == ProviderHealthState::Degraded);
    if degraded {
        let mut effective = flags.clone();
        effective.publish_enabled = false;
        effective
    } else {
        flags.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::health::{ProviderHealthSnapshot, ProviderSloObservations};
    use serde_json::json;

    #[test]
    fn module_settings_normalization_matches_all_declared_profiles() {
        for profile in BuilderToggleProfile::ALL {
            let flags = profile.flags();
            let settings = json!({
                "builder": {
                    "enabled": flags.builder_enabled,
                    "preview": { "enabled": flags.preview_enabled },
                    "properties": { "enabled": flags.properties_enabled },
                    "publish": { "enabled": flags.publish_enabled }
                }
            });
            assert_eq!(BuilderCapabilityFlags::from_module_settings(&settings), Ok(flags));
        }
    }

    #[test]
    fn module_settings_normalization_defaults_missing_keys_but_rejects_bad_types() {
        assert_eq!(
            BuilderCapabilityFlags::from_module_settings(&json!({})),
            Ok(BuilderCapabilityFlags::default())
        );
        for settings in [
            json!({ "builder": "off" }),
            json!({ "builder": { "enabled": "false" } }),
            json!({ "builder": { "preview": false } }),
            json!({ "builder": { "publish": { "enabled": 0 } } }),
        ] {
            assert!(BuilderCapabilityFlags::from_module_settings(&settings).is_err());
        }
    }

    #[test]
    fn provider_health_runtime_flags_only_narrow_configured_rollout() {
        let ready = ProviderHealthSnapshot::evaluate(ProviderSloObservations {
            preview_p95_ms: 1_000,
            publish_p95_ms: 2_000,
            sanitize_failure_rate: 0.0,
            runtime_error_rate: 0.0,
        });
        assert_eq!(
            effective_provider_runtime_flags(&BuilderCapabilityFlags::default(), Some(&ready)),
            BuilderCapabilityFlags::default()
        );
        assert_eq!(
            effective_provider_runtime_flags(&BuilderToggleProfile::PreviewOff.flags(), Some(&ready)),
            BuilderToggleProfile::PreviewOff.flags()
        );

        let degraded = ProviderHealthSnapshot::evaluate(ProviderSloObservations {
            preview_p95_ms: 1_600,
            publish_p95_ms: 2_000,
            sanitize_failure_rate: 0.0,
            runtime_error_rate: 0.0,
        });
        assert_eq!(
            effective_provider_runtime_flags(&BuilderCapabilityFlags::default(), Some(&degraded)),
            BuilderToggleProfile::PublishOff.flags()
        );

        let unavailable = ProviderHealthSnapshot::evaluate(ProviderSloObservations {
            preview_p95_ms: 1_000,
            publish_p95_ms: 2_000,
            sanitize_failure_rate: 0.0,
            runtime_error_rate: 0.03,
        });
        assert_eq!(
            effective_provider_runtime_flags(&BuilderCapabilityFlags::default(), Some(&unavailable)),
            BuilderToggleProfile::BuilderOff.flags()
        );

        let properties_off = BuilderCapabilityFlags {
            builder_enabled: true,
            preview_enabled: true,
            properties_enabled: false,
            publish_enabled: true,
        };
        let effective = effective_provider_runtime_flags(&properties_off, None);
        assert!(effective.builder_enabled);
        assert!(effective.preview_enabled);
        assert!(!effective.properties_enabled);
        assert!(!effective.publish_enabled);
    }
}
