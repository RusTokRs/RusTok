use crate::landing::LandingProjectError;
use crate::static_landing::StaticLandingCompiler;
use crate::static_publish_policy::{
    PageBuilderStaticPublishPolicyError, PageBuilderStaticPublishPolicyEvidence,
    validate_static_publish_document,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

#[path = "static_publish_resource_limits.rs"]
pub mod static_publish_resource_limits;

use static_publish_resource_limits::{
    PageBuilderStaticPublishResourceEvidence, PageBuilderStaticPublishResourceLimitError,
    validate_static_publish_resource_limits,
};

pub const PAGE_BUILDER_STATIC_SANITIZATION_FORMAT: &str =
    "page_builder_static_publish_sanitization_v3";
const PAGE_BUILDER_STATIC_SANITIZATION_LEGACY_FORMAT: &str =
    "page_builder_static_publish_sanitization_v2";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PageBuilderSanitizedStaticLandingProject {
    pub format: String,
    pub policy_format: String,
    pub policy_hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resource_limits: Option<PageBuilderStaticPublishResourceEvidence>,
    pub sanitized_project: Value,
    pub sanitized_hash: String,
}

impl PageBuilderSanitizedStaticLandingProject {
    pub fn project_data(&self) -> &Value {
        &self.sanitized_project
    }

    pub fn sanitized_hash(&self) -> &str {
        &self.sanitized_hash
    }

    pub fn policy_evidence(&self) -> PageBuilderStaticPublishPolicyEvidence {
        PageBuilderStaticPublishPolicyEvidence {
            format: self.policy_format.clone(),
            policy_hash: self.policy_hash.clone(),
        }
    }

    pub fn verify_integrity(&self) -> Result<(), PageBuilderStaticLandingSanitizationError> {
        if !matches!(
            self.format.as_str(),
            PAGE_BUILDER_STATIC_SANITIZATION_FORMAT
                | PAGE_BUILDER_STATIC_SANITIZATION_LEGACY_FORMAT
        ) {
            return Err(PageBuilderStaticLandingSanitizationError::Integrity(
                "unsupported static publish sanitization format".to_string(),
            ));
        }
        if !self.sanitized_project.is_object() {
            return Err(PageBuilderStaticLandingSanitizationError::Integrity(
                "sanitized static landing project must be a JSON object".to_string(),
            ));
        }
        let policy_evidence = self.policy_evidence();
        policy_evidence.verify_integrity()?;
        if !is_sha256(&self.sanitized_hash) {
            return Err(PageBuilderStaticLandingSanitizationError::Integrity(
                "sanitized static landing hash must be SHA-256".to_string(),
            ));
        }

        match self.format.as_str() {
            PAGE_BUILDER_STATIC_SANITIZATION_FORMAT => self
                .resource_limits
                .as_ref()
                .ok_or_else(|| {
                    PageBuilderStaticLandingSanitizationError::Integrity(
                        "current sanitization evidence is missing resource limits".to_string(),
                    )
                })?
                .verify_integrity()?,
            PAGE_BUILDER_STATIC_SANITIZATION_LEGACY_FORMAT => {
                if self.resource_limits.is_some() {
                    return Err(PageBuilderStaticLandingSanitizationError::Integrity(
                        "legacy sanitization evidence must not contain resource limits".to_string(),
                    ));
                }
            }
            _ => unreachable!("sanitization format checked above"),
        }

        let expected = sanitization_hash(
            &self.format,
            &self.sanitized_project,
            &self.policy_format,
            &self.policy_hash,
            self.resource_limits.as_ref(),
        )?;
        if expected != self.sanitized_hash {
            return Err(PageBuilderStaticLandingSanitizationError::Integrity(
                "sanitized static landing hash mismatch".to_string(),
            ));
        }

        let document =
            StaticLandingCompiler::default().prepare_document(&self.sanitized_project)?;
        let verified_policy = validate_static_publish_document(&document)?;
        if verified_policy != policy_evidence {
            return Err(PageBuilderStaticLandingSanitizationError::Integrity(
                "sanitized static landing policy evidence mismatch".to_string(),
            ));
        }
        if let Some(expected_resources) = self.resource_limits.as_ref() {
            let verified_resources = validate_static_publish_resource_limits(&document)?;
            if &verified_resources != expected_resources {
                return Err(PageBuilderStaticLandingSanitizationError::Integrity(
                    "sanitized static landing resource evidence mismatch".to_string(),
                ));
            }
        }
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PageBuilderStaticLandingSanitizationError {
    #[error(transparent)]
    Landing(#[from] LandingProjectError),
    #[error(transparent)]
    Policy(#[from] PageBuilderStaticPublishPolicyError),
    #[error(transparent)]
    Resource(#[from] PageBuilderStaticPublishResourceLimitError),
    #[error("static publish sanitization encoding failed: {0}")]
    Encode(String),
    #[error("static publish sanitization integrity failed: {0}")]
    Integrity(String),
}

/// Applies the authoritative public-artifact policy before runtime materialization.
///
/// The returned project is a compiler-owned clone. It contains deterministic stable component ids,
/// preserves current Fly extension fields and has passed structural validation, the complete
/// fail-closed static publish policy, secure-resource validation and bounded global resource limits.
/// The original editor source and runtime context remain untouched.
pub fn sanitize_static_landing_project(
    project_data: &Value,
) -> Result<PageBuilderSanitizedStaticLandingProject, PageBuilderStaticLandingSanitizationError> {
    let document = StaticLandingCompiler::default().prepare_document(project_data)?;
    let PageBuilderStaticPublishPolicyEvidence {
        format: policy_format,
        policy_hash,
    } = validate_static_publish_document(&document)?;
    let resource_limits = validate_static_publish_resource_limits(&document)?;
    let sanitized_project = serde_json::to_value(document.project)
        .map_err(|error| PageBuilderStaticLandingSanitizationError::Encode(error.to_string()))?;
    let sanitized_hash = sanitization_hash(
        PAGE_BUILDER_STATIC_SANITIZATION_FORMAT,
        &sanitized_project,
        &policy_format,
        &policy_hash,
        Some(&resource_limits),
    )?;
    let result = PageBuilderSanitizedStaticLandingProject {
        format: PAGE_BUILDER_STATIC_SANITIZATION_FORMAT.to_string(),
        policy_format,
        policy_hash,
        resource_limits: Some(resource_limits),
        sanitized_project,
        sanitized_hash,
    };
    result.verify_integrity()?;
    Ok(result)
}

fn sanitization_hash(
    format: &str,
    sanitized_project: &Value,
    policy_format: &str,
    policy_hash: &str,
    resource_limits: Option<&PageBuilderStaticPublishResourceEvidence>,
) -> Result<String, PageBuilderStaticLandingSanitizationError> {
    match format {
        PAGE_BUILDER_STATIC_SANITIZATION_FORMAT => stable_hash(&(
            PAGE_BUILDER_STATIC_SANITIZATION_FORMAT,
            policy_format,
            policy_hash,
            resource_limits.ok_or_else(|| {
                PageBuilderStaticLandingSanitizationError::Integrity(
                    "current sanitization hash is missing resource limits".to_string(),
                )
            })?,
            sanitized_project,
        )),
        PAGE_BUILDER_STATIC_SANITIZATION_LEGACY_FORMAT => {
            if resource_limits.is_some() {
                return Err(PageBuilderStaticLandingSanitizationError::Integrity(
                    "legacy sanitization hash must not include resource limits".to_string(),
                ));
            }
            stable_hash(&(
                PAGE_BUILDER_STATIC_SANITIZATION_LEGACY_FORMAT,
                policy_format,
                policy_hash,
                sanitized_project,
            ))
        }
        _ => Err(PageBuilderStaticLandingSanitizationError::Integrity(
            "unsupported sanitization hash format".to_string(),
        )),
    }
}

fn stable_hash(
    value: &impl Serialize,
) -> Result<String, PageBuilderStaticLandingSanitizationError> {
    let bytes = serde_json::to_vec(value)
        .map_err(|error| PageBuilderStaticLandingSanitizationError::Encode(error.to_string()))?;
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
    use crate::static_publish_policy::PAGE_BUILDER_STATIC_PUBLISH_POLICY_FORMAT;
    use serde_json::json;

    fn project() -> Value {
        json!({
            "pages": [{
                "id": "home",
                "flyPageMeta": {
                    "title": "Home",
                    "description": "Sanitized landing",
                    "slug": "home",
                    "canonical_url": "/home"
                },
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "components": [{
                        "type": "heading",
                        "tagName": "h1",
                        "content": "Welcome"
                    }]
                }
            }]
        })
    }

    #[test]
    fn sanitization_assigns_stable_ids_and_hashes_policy_bound_project() {
        let project = project();
        let first = sanitize_static_landing_project(&project).expect("sanitized project");
        let second = sanitize_static_landing_project(&project).expect("sanitized project");

        assert_eq!(first, second);
        assert_eq!(first.format, PAGE_BUILDER_STATIC_SANITIZATION_FORMAT);
        assert_eq!(first.sanitized_hash.len(), 64);
        assert_eq!(
            first.policy_format,
            PAGE_BUILDER_STATIC_PUBLISH_POLICY_FORMAT
        );
        assert_eq!(first.policy_hash.len(), 64);
        let resource_limits = first
            .resource_limits
            .as_ref()
            .expect("current resource evidence");
        assert_eq!(resource_limits.observed.page_count, 1);
        assert_eq!(resource_limits.observed.component_count, 2);
        assert_eq!(
            first.sanitized_hash,
            sanitization_hash(
                &first.format,
                &first.sanitized_project,
                &first.policy_format,
                &first.policy_hash,
                first.resource_limits.as_ref(),
            )
            .expect("policy-and-resource-bound sanitization hash")
        );
        assert!(
            first.sanitized_project["pages"][0]["component"]["components"][0]["id"]
                .as_str()
                .is_some_and(|id| id.starts_with("fly-static-"))
        );
        first.verify_integrity().expect("sanitization integrity");
    }

    #[test]
    fn legacy_v2_sanitization_remains_verifiable() {
        let mut legacy = sanitize_static_landing_project(&project()).expect("current sanitization");
        legacy.format = PAGE_BUILDER_STATIC_SANITIZATION_LEGACY_FORMAT.to_string();
        legacy.resource_limits = None;
        legacy.sanitized_hash = sanitization_hash(
            &legacy.format,
            &legacy.sanitized_project,
            &legacy.policy_format,
            &legacy.policy_hash,
            None,
        )
        .expect("legacy sanitization hash");

        legacy.verify_integrity().expect("legacy v2 integrity");
    }

    #[test]
    fn sanitization_rejects_insecure_public_resources() {
        let project = json!({
            "pages": [{
                "id": "home",
                "flyPageMeta": {
                    "title": "Home",
                    "description": "Sanitized landing",
                    "slug": "home"
                },
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "components": [{
                        "id": "hero",
                        "type": "image",
                        "attributes": { "src": "http://cdn.example.com/hero.webp" }
                    }]
                }
            }]
        });

        assert!(sanitize_static_landing_project(&project).is_err());
    }

    #[test]
    fn sanitization_rejects_renderer_dropped_attributes_and_css() {
        let project = json!({
            "pages": [{
                "id": "home",
                "flyPageMeta": { "title": "Home", "slug": "home" },
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "components": [{
                        "id": "hero",
                        "type": "link",
                        "tagName": "a",
                        "attributes": {
                            "onclick": "alert(1)",
                            "href": "javascript:alert(1)"
                        },
                        "style": { "background-image": "url(https://evil.example/x.png)" },
                        "content": "Unsafe"
                    }]
                }
            }]
        });

        let error = sanitize_static_landing_project(&project).expect_err("policy rejection");
        let PageBuilderStaticLandingSanitizationError::Landing(LandingProjectError::Validation {
            diagnostics,
        }) = error
        else {
            panic!("expected compiler policy validation error");
        };
        assert!(diagnostics.len() >= 3);
    }
}
