use crate::landing::LandingProjectError;
use crate::static_landing::StaticLandingCompiler;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

pub const PAGE_BUILDER_STATIC_SANITIZATION_FORMAT: &str =
    "page_builder_static_publish_sanitization_v1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PageBuilderSanitizedStaticLandingProject {
    pub format: String,
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
        if !is_sha256(&self.sanitized_hash) {
            return Err(PageBuilderStaticLandingSanitizationError::Integrity(
                "sanitized static landing hash must be SHA-256".to_string(),
            ));
        }
        let expected = stable_hash(&self.sanitized_project)?;
        if expected != self.sanitized_hash {
            return Err(PageBuilderStaticLandingSanitizationError::Integrity(
                "sanitized static landing hash mismatch".to_string(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PageBuilderStaticLandingSanitizationError {
    #[error(transparent)]
    Landing(#[from] LandingProjectError),
    #[error("static publish sanitization encoding failed: {0}")]
    Encode(String),
    #[error("static publish sanitization integrity failed: {0}")]
    Integrity(String),
}

/// Applies the authoritative public-artifact policy before runtime materialization.
///
/// The returned project is a compiler-owned clone. It contains deterministic stable component ids,
/// preserves current Fly extension fields and has already passed structural and secure-resource
/// validation. The original editor source and runtime context remain untouched.
pub fn sanitize_static_landing_project(
    project_data: &Value,
) -> Result<PageBuilderSanitizedStaticLandingProject, PageBuilderStaticLandingSanitizationError> {
    let document = StaticLandingCompiler::default().prepare_document(project_data)?;
    let sanitized_project = serde_json::to_value(document.project).map_err(|error| {
        PageBuilderStaticLandingSanitizationError::Encode(error.to_string())
    })?;
    let result = PageBuilderSanitizedStaticLandingProject {
        format: PAGE_BUILDER_STATIC_SANITIZATION_FORMAT.to_string(),
        sanitized_hash: stable_hash(&sanitized_project)?,
        sanitized_project,
    };
    result.verify_integrity()?;
    Ok(result)
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
    use serde_json::json;

    #[test]
    fn sanitization_assigns_stable_ids_and_hashes_the_exact_project() {
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
}
