//! Versioned JSON binding shared by every Rhai sandbox subject.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

/// The only Rhai request/result binding version supported by the runtime.
pub const RHAI_BINDING_VERSION: u32 = 1;

/// The versioned data envelope made available to a Rhai program as `input`.
///
/// The enclosed value belongs to the subject owner (for example Alloy's draft
/// snapshot or an admitted artifact binding input). The envelope itself is
/// neutral so draft and published Rhai executions use the same ABI boundary.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RhaiBindingInput {
    pub binding_version: u32,
    pub input: Value,
}

impl RhaiBindingInput {
    pub fn new(input: Value) -> Self {
        Self {
            binding_version: RHAI_BINDING_VERSION,
            input,
        }
    }

    pub fn decode(value: Value) -> Result<Self, RhaiBindingError> {
        let binding: Self = serde_json::from_value(value)
            .map_err(|error| RhaiBindingError::InvalidInput(error.to_string()))?;
        binding.validate()?;
        Ok(binding)
    }

    pub fn validate(&self) -> Result<(), RhaiBindingError> {
        if self.binding_version != RHAI_BINDING_VERSION {
            return Err(RhaiBindingError::UnsupportedVersion(self.binding_version));
        }
        Ok(())
    }
}

/// The versioned JSON result returned by every Rhai sandbox execution.
///
/// Consumers must decode this envelope before interpreting the subject-owned
/// output value. There is intentionally no raw-result compatibility path.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RhaiBindingOutput {
    pub binding_version: u32,
    pub output: Value,
}

impl RhaiBindingOutput {
    pub fn new(output: Value) -> Self {
        Self {
            binding_version: RHAI_BINDING_VERSION,
            output,
        }
    }

    pub fn decode(value: Value) -> Result<Self, RhaiBindingError> {
        let binding: Self = serde_json::from_value(value)
            .map_err(|error| RhaiBindingError::InvalidOutput(error.to_string()))?;
        binding.validate()?;
        Ok(binding)
    }

    pub fn validate(&self) -> Result<(), RhaiBindingError> {
        if self.binding_version != RHAI_BINDING_VERSION {
            return Err(RhaiBindingError::UnsupportedVersion(self.binding_version));
        }
        Ok(())
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RhaiBindingError {
    #[error("Rhai binding version `{0}` is unsupported")]
    UnsupportedVersion(u32),
    #[error("invalid Rhai binding input: {0}")]
    InvalidInput(String),
    #[error("invalid Rhai binding output: {0}")]
    InvalidOutput(String),
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn input_rejects_raw_values_and_unknown_fields() {
        assert!(matches!(
            RhaiBindingInput::decode(json!({ "value": 1 })),
            Err(RhaiBindingError::InvalidInput(_))
        ));
        assert!(matches!(
            RhaiBindingInput::decode(json!({
                "binding_version": RHAI_BINDING_VERSION,
                "input": null,
                "unexpected": true,
            })),
            Err(RhaiBindingError::InvalidInput(_))
        ));
    }

    #[test]
    fn output_rejects_another_version() {
        assert_eq!(
            RhaiBindingOutput::decode(json!({
                "binding_version": RHAI_BINDING_VERSION + 1,
                "output": null,
            })),
            Err(RhaiBindingError::UnsupportedVersion(
                RHAI_BINDING_VERSION + 1
            ))
        );
    }
}
