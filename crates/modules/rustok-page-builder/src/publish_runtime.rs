use crate::dto::{
    MAX_PREVIEW_RUNTIME_CONTEXT_BYTES, MAX_PREVIEW_SCENARIO_ID_BYTES, PageBuilderPreviewRuntime,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

pub const PAGE_BUILDER_PUBLISH_RUNTIME_REVIEW_FORMAT: &str =
    "page_builder_publish_runtime_review_v1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PageBuilderReviewedPublishRuntime {
    pub format: String,
    pub scenario_id: String,
    pub context: Value,
    pub review_hash: String,
}

impl PageBuilderReviewedPublishRuntime {
    pub fn new(
        scenario_id: impl Into<String>,
        context: Value,
    ) -> Result<Self, PageBuilderPublishRuntimeReviewError> {
        let scenario_id = scenario_id.into();
        let review_hash = review_hash(&scenario_id, &context)?;
        let review = Self {
            format: PAGE_BUILDER_PUBLISH_RUNTIME_REVIEW_FORMAT.to_string(),
            scenario_id,
            context,
            review_hash,
        };
        review.validate()?;
        Ok(review)
    }

    pub fn validate(&self) -> Result<(), PageBuilderPublishRuntimeReviewError> {
        if self.format != PAGE_BUILDER_PUBLISH_RUNTIME_REVIEW_FORMAT {
            return Err(PageBuilderPublishRuntimeReviewError::InvalidFormat);
        }
        if self.scenario_id.is_empty() || self.scenario_id.trim() != self.scenario_id {
            return Err(PageBuilderPublishRuntimeReviewError::InvalidScenarioId);
        }
        if self.scenario_id.len() > MAX_PREVIEW_SCENARIO_ID_BYTES {
            return Err(PageBuilderPublishRuntimeReviewError::ScenarioIdTooLarge {
                maximum: MAX_PREVIEW_SCENARIO_ID_BYTES,
            });
        }
        if !self.context.is_object() {
            return Err(PageBuilderPublishRuntimeReviewError::ContextMustBeObject);
        }
        let context_bytes = serde_json::to_vec(&self.context)
            .map_err(|error| PageBuilderPublishRuntimeReviewError::Encode(error.to_string()))?;
        if context_bytes.len() > MAX_PREVIEW_RUNTIME_CONTEXT_BYTES {
            return Err(PageBuilderPublishRuntimeReviewError::ContextTooLarge {
                maximum: MAX_PREVIEW_RUNTIME_CONTEXT_BYTES,
            });
        }
        if !is_sha256(&self.review_hash) {
            return Err(PageBuilderPublishRuntimeReviewError::InvalidReviewHash);
        }
        let expected = review_hash(&self.scenario_id, &self.context)?;
        if expected != self.review_hash {
            return Err(PageBuilderPublishRuntimeReviewError::ReviewHashMismatch);
        }
        Ok(())
    }

    pub fn preview_runtime(
        &self,
    ) -> Result<PageBuilderPreviewRuntime, PageBuilderPublishRuntimeReviewError> {
        self.validate()?;
        let runtime =
            PageBuilderPreviewRuntime::new(self.context.clone(), Some(self.scenario_id.clone()));
        runtime
            .validate()
            .map_err(|error| PageBuilderPublishRuntimeReviewError::Runtime(error.to_string()))?;
        Ok(runtime)
    }

    pub fn runtime_context_hash(&self) -> Result<String, PageBuilderPublishRuntimeReviewError> {
        self.validate()?;
        stable_hash(&self.context)
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum PageBuilderPublishRuntimeReviewError {
    #[error("unsupported Page Builder publish-runtime review format")]
    InvalidFormat,
    #[error("publish-runtime scenario_id must be a non-empty normalized string")]
    InvalidScenarioId,
    #[error("publish-runtime scenario_id exceeds {maximum} bytes")]
    ScenarioIdTooLarge { maximum: usize },
    #[error("publish-runtime context must be a JSON object")]
    ContextMustBeObject,
    #[error("publish-runtime context exceeds {maximum} bytes")]
    ContextTooLarge { maximum: usize },
    #[error("publish-runtime review_hash must be a SHA-256 value")]
    InvalidReviewHash,
    #[error("publish-runtime review_hash does not match scenario/context")]
    ReviewHashMismatch,
    #[error("publish-runtime encoding failed: {0}")]
    Encode(String),
    #[error("publish-runtime contract failed: {0}")]
    Runtime(String),
}

fn review_hash(
    scenario_id: &str,
    context: &Value,
) -> Result<String, PageBuilderPublishRuntimeReviewError> {
    stable_hash(&(
        PAGE_BUILDER_PUBLISH_RUNTIME_REVIEW_FORMAT,
        scenario_id,
        context,
    ))
}

fn stable_hash(value: &impl Serialize) -> Result<String, PageBuilderPublishRuntimeReviewError> {
    let bytes = serde_json::to_vec(value)
        .map_err(|error| PageBuilderPublishRuntimeReviewError::Encode(error.to_string()))?;
    Ok(Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn reviewed_runtime_roundtrip_is_stable() {
        let reviewed = PageBuilderReviewedPublishRuntime::new(
            "landing-primary",
            json!({ "page": { "title": "Reviewed" } }),
        )
        .expect("reviewed runtime");
        let encoded = serde_json::to_value(&reviewed).expect("serialize reviewed runtime");
        let decoded: PageBuilderReviewedPublishRuntime =
            serde_json::from_value(encoded).expect("deserialize reviewed runtime");

        decoded.validate().expect("review integrity");
        assert_eq!(
            decoded.preview_runtime().unwrap().scenario_id.as_deref(),
            Some("landing-primary")
        );
        assert_eq!(decoded.review_hash.len(), 64);
    }

    #[test]
    fn reviewed_runtime_rejects_mutated_context() {
        let mut reviewed = PageBuilderReviewedPublishRuntime::new(
            "landing-primary",
            json!({ "page": { "title": "Reviewed" } }),
        )
        .expect("reviewed runtime");
        reviewed.context = json!({ "page": { "title": "Changed" } });

        assert_eq!(
            reviewed.validate(),
            Err(PageBuilderPublishRuntimeReviewError::ReviewHashMismatch)
        );
    }

    #[test]
    fn reviewed_runtime_requires_explicit_scenario() {
        assert_eq!(
            PageBuilderReviewedPublishRuntime::new("", json!({})),
            Err(PageBuilderPublishRuntimeReviewError::InvalidScenarioId)
        );
    }
}
