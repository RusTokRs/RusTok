use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Mutex},
};

use jsonschema::{Draft, PatternOptions, Validator};
use serde_json::Value;

const MAX_ARTIFACT_SCHEMA_VALIDATORS: usize = 128;
const MAX_ARTIFACT_SCHEMA_REGEX_BYTES: usize = 64 * 1024;

/// Content-free failure taxonomy for validation against an admitted artifact
/// schema. Callers add their own binding/settings context without exposing the
/// rejected value or validator internals.
#[derive(Debug)]
pub(crate) enum ArtifactSchemaValidationError {
    Compilation,
    Violation,
    CachePoisoned,
}

/// Bounded node-local cache compiled only from descriptor-bundled Draft
/// 2020-12 schemas. The workspace disables filesystem and network resolvers,
/// so validation cannot escape the admitted descriptor bundle.
pub(crate) struct ArtifactSchemaValidatorCache {
    state: Mutex<ArtifactSchemaValidatorCacheState>,
}

#[derive(Default)]
struct ArtifactSchemaValidatorCacheState {
    validators: HashMap<String, Arc<Validator>>,
    lru: VecDeque<String>,
}

impl Default for ArtifactSchemaValidatorCache {
    fn default() -> Self {
        Self {
            state: Mutex::new(ArtifactSchemaValidatorCacheState::default()),
        }
    }
}

impl ArtifactSchemaValidatorCache {
    pub(crate) fn validate(
        &self,
        schema_digest: &str,
        schema: &Value,
        value: &Value,
    ) -> Result<(), ArtifactSchemaValidationError> {
        let validator = self.get_or_compile(schema_digest, schema)?;
        validator
            .validate(value)
            .map_err(|_| ArtifactSchemaValidationError::Violation)
    }

    fn get_or_compile(
        &self,
        schema_digest: &str,
        schema: &Value,
    ) -> Result<Arc<Validator>, ArtifactSchemaValidationError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| ArtifactSchemaValidationError::CachePoisoned)?;
        if let Some(validator) = state.validators.get(schema_digest).cloned() {
            state.lru.retain(|digest| digest != schema_digest);
            state.lru.push_back(schema_digest.to_string());
            return Ok(validator);
        }
        drop(state);

        let validator = Arc::new(
            jsonschema::options()
                .with_draft(Draft::Draft202012)
                .should_validate_formats(true)
                .should_ignore_unknown_formats(false)
                .with_pattern_options(
                    PatternOptions::regex()
                        .size_limit(MAX_ARTIFACT_SCHEMA_REGEX_BYTES)
                        .dfa_size_limit(MAX_ARTIFACT_SCHEMA_REGEX_BYTES),
                )
                .build(schema)
                .map_err(|_| ArtifactSchemaValidationError::Compilation)?,
        );

        let mut state = self
            .state
            .lock()
            .map_err(|_| ArtifactSchemaValidationError::CachePoisoned)?;
        if let Some(existing) = state.validators.get(schema_digest).cloned() {
            state.lru.retain(|digest| digest != schema_digest);
            state.lru.push_back(schema_digest.to_string());
            return Ok(existing);
        }
        while state.validators.len() >= MAX_ARTIFACT_SCHEMA_VALIDATORS {
            let Some(oldest) = state.lru.pop_front() else {
                break;
            };
            state.validators.remove(&oldest);
        }
        state
            .validators
            .insert(schema_digest.to_string(), Arc::clone(&validator));
        state.lru.push_back(schema_digest.to_string());
        Ok(validator)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn admitted_schema_validator_rejects_an_invalid_value() {
        let schema = json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "type": "object",
            "required": ["enabled"],
            "additionalProperties": false,
            "properties": {
                "enabled": { "type": "boolean" }
            }
        });
        let cache = ArtifactSchemaValidatorCache::default();

        cache
            .validate("sha256:settings", &schema, &json!({ "enabled": true }))
            .expect("valid settings");
        assert!(matches!(
            cache.validate("sha256:settings", &schema, &json!({ "enabled": "yes" })),
            Err(ArtifactSchemaValidationError::Violation)
        ));
    }
}
