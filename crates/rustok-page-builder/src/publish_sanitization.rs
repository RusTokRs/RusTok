use crate::landing::LandingProjectError;
use crate::static_landing::StaticLandingCompiler;
use crate::static_publish_policy::{
    PageBuilderStaticPublishPolicyError, PageBuilderStaticPublishPolicyEvidence,
    validate_static_publish_document,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

pub const PAGE_BUILDER_STATIC_SANITIZATION_FORMAT: &str =
    "page_builder_static_publish_sanitization_v2";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PageBuilderSanitizedStaticLandingProject {
    pub format: String,
    pub policy_format: String,
    pub policy_hash: String,
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
        if self.format != PAGE_BUILDER_STATIC_SANITIZATION_FORMAT {
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
        let expected = sanitization_hash(
            &self.sanitized_project,
            &self.policy_format,
            &self.policy_hash,
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
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PageBuilderStaticLandingSanitizationError {
    #[error(transparent)]
    Landing(#[from] LandingProjectError),
    #[error(transparent)]
    Policy(#[from] PageBuilderStaticPublishPolicyError),
    #[error("static publish sanitization encoding failed: {0}")]
    Encode(String),
    #[error("static publish sanitization integrity failed: {0}")]
    Integrity(String),
}

/// Applies the authoritative public-artifact policy before runtime materialization.
///
/// The returned project is a compiler-owned clone. It contains deterministic stable component ids,
/// preserves current Fly extension fields and has passed structural validation, the complete
/// fail-closed static publish policy and secure-resource validation. The original editor source and
/// runtime context remain untouched.
pub fn sanitize_static_landing_project(
    project_data: &Value,
) -> Result<PageBuilderSanitizedStaticLandingProject, PageBuilderStaticLandingSanitizationError> {
    let document = StaticLandingCompiler::default().prepare_document(project_data)?;
    let PageBuilderStaticPublishPolicyEvidence {
        format: policy_format,
        policy_hash,
    } = validate_static_publish_document(&document)?;
    let sanitized_project = serde_json::to_value(document.project).map_err(|error| {
        PageBuilderStaticLandingSanitizationError::Encode(error.to_string())
    })?;
    let sanitized_hash =
        sanitization_hash(&sanitized_project, &policy_format, &policy_hash)?;
    let result = PageBuilderSanitizedStaticLandingProject {
        format: PAGE_BUILDER_STATIC_SANITIZATION_FORMAT.to_string(),
        policy_format,
        policy_hash,
        sanitized_project,
        sanitized_hash,
    };
    result.verify_integrity()?;
    Ok(result)
}

fn sanitization_hash(
    sanitized_project: &Value,
    policy_format: &str,
    policy_hash: &str,
) -> Result<String, PageBuilderStaticLandingSanitizationError> {
    stable_hash(&(
        PAGE_BUILDER_STATIC_SANITIZATION_FORMAT,
        policy_format,
        policy_hash,
        sanitized_project,
    ))
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

    #[test]
    fn sanitization_assigns_stable_ids_and_hashes_policy_bound_project() {
        let project = json!({
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
        });
        let first = sanitize_static_landing_project(&project).expect("sanitized project");
        let second = sanitize_static_landing_project(&project).expect("sanitized project");

        assert_eq!(first, second);
        assert_eq!(first.sanitized_hash.len(), 64);
        assert_eq!(
            first.policy_format,
            PAGE_BUILDER_STATIC_PUBLISH_POLICY_FORMAT
        );
        assert_eq!(first.policy_hash.len(), 64);
        assert_eq!(
            first.sanitized_hash,
            sanitization_hash(
                &first.sanitized_project,
                &first.policy_format,
                &first.policy_hash,
            )
            .expect("policy-bound sanitization hash")
        );
        assert!(first.sanitized_project["pages"][0]["component"]["components"][0]["id"]
            .as_str()
            .is_some_and(|id| id.starts_with("fly-static-")));
        first.verify_integrity().expect("sanitization integrity");
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
        let PageBuilderStaticLandingSanitizationError::Landing(
            LandingProjectError::Validation { diagnostics },
        ) = error
        else {
            panic!("expected compiler policy validation error");
        };
        assert!(diagnostics.len() >= 3);
    }
}
