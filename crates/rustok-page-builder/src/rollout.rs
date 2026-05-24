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

        if !self.builder_enabled && (self.preview_enabled || self.properties_enabled || self.publish_enabled)
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
    CapabilityDisabled(BuilderCapabilityKind),
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
        Err(BuilderRolloutError::CapabilityDisabled(capability))
    }
}
