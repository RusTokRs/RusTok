use crate::dto::BuilderCapabilityKind;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuilderCapabilityFlags {
    pub builder_enabled: bool,
    pub preview_enabled: bool,
    pub properties_enabled: bool,
    pub publish_enabled: bool,
    pub legacy_bridge_readonly: bool,
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
                legacy_bridge_readonly: true,
            },
            Self::PublishOff => BuilderCapabilityFlags {
                builder_enabled: true,
                preview_enabled: true,
                properties_enabled: true,
                publish_enabled: false,
                legacy_bridge_readonly: true,
            },
            Self::PreviewOff => BuilderCapabilityFlags {
                builder_enabled: true,
                preview_enabled: false,
                properties_enabled: true,
                publish_enabled: false,
                legacy_bridge_readonly: true,
            },
            Self::BuilderOff => BuilderCapabilityFlags {
                builder_enabled: false,
                preview_enabled: false,
                properties_enabled: false,
                publish_enabled: false,
                legacy_bridge_readonly: true,
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
                expected_error: None,
            },
            Self::PublishOff => BuilderFallbackOutcome {
                profile: self,
                admin_visual_path: "editable_builder_publish_disabled",
                preview: "available",
                properties: "available",
                publish: "typed_feature_disabled_error",
                read_paths: "stable",
                expected_error: Some("publish"),
            },
            Self::PreviewOff => BuilderFallbackOutcome {
                profile: self,
                admin_visual_path: "preview_hidden_properties_available",
                preview: "typed_feature_disabled_error",
                properties: "available",
                publish: "typed_feature_disabled_error",
                read_paths: "stable",
                expected_error: Some("preview"),
            },
            Self::BuilderOff => BuilderFallbackOutcome {
                profile: self,
                admin_visual_path: "readonly_fallback",
                preview: "typed_feature_disabled_error",
                properties: "typed_feature_disabled_error",
                publish: "typed_feature_disabled_error",
                read_paths: "stable",
                expected_error: Some("builder"),
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
    pub expected_error: Option<&'static str>,
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
            legacy_bridge_readonly: false,
        }
    }
}

impl BuilderCapabilityFlags {
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
