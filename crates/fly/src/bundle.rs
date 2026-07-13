use crate::{
    audit_page, validate_project, FlyError, FlyResult, GrapesJsV1Codec, PageLocator,
    ProjectDocument, ProjectHash, RegistrySet, ValidationLimits, ValidationReport, GRAPESJS_V1,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

pub const FLY_PROJECT_BUNDLE_V1: &str = "fly_project_bundle_v1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct BundleMetadata {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_module: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_document_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_revision_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exported_at: Option<String>,
    #[serde(flatten)]
    pub extensions: Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProjectBundle {
    pub bundle_format: String,
    pub project_format: String,
    pub project_hash: String,
    pub project_data: Value,
    #[serde(default)]
    pub metadata: BundleMetadata,
    #[serde(default, flatten)]
    pub extensions: Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BundleDecodePolicy {
    pub allow_raw_project: bool,
    pub allow_hash_mismatch: bool,
    pub require_supported_project_format: bool,
}

impl Default for BundleDecodePolicy {
    fn default() -> Self {
        Self {
            allow_raw_project: true,
            allow_hash_mismatch: false,
            require_supported_project_format: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DecodedProjectBundle {
    pub bundle: ProjectBundle,
    pub document: ProjectDocument,
    pub hash_matches: bool,
    pub imported_from_raw_project: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BundleInspection {
    pub bundle_format: String,
    pub project_format: String,
    pub declared_hash: String,
    pub actual_hash: String,
    pub hash_matches: bool,
    pub page_count: usize,
    pub node_count: usize,
    pub asset_count: usize,
    pub style_rule_count: usize,
    pub validation: ValidationReport,
    pub audit_error_count: usize,
    pub audit_warning_count: usize,
}

pub fn export_project_bundle(
    document: &ProjectDocument,
    metadata: BundleMetadata,
) -> FlyResult<ProjectBundle> {
    Ok(ProjectBundle {
        bundle_format: FLY_PROJECT_BUNDLE_V1.to_string(),
        project_format: GRAPESJS_V1.to_string(),
        project_hash: document.hash().hex(),
        project_data: GrapesJsV1Codec::encode_value(document)?,
        metadata,
        extensions: Map::new(),
    })
}

pub fn encode_project_bundle(bundle: &ProjectBundle, pretty: bool) -> FlyResult<Vec<u8>> {
    if pretty {
        serde_json::to_vec_pretty(bundle).map_err(|error| FlyError::Encode(error.to_string()))
    } else {
        serde_json::to_vec(bundle).map_err(|error| FlyError::Encode(error.to_string()))
    }
}

pub fn decode_project_bundle(
    bytes: &[u8],
    policy: &BundleDecodePolicy,
) -> FlyResult<DecodedProjectBundle> {
    let value: Value =
        serde_json::from_slice(bytes).map_err(|error| FlyError::Decode(error.to_string()))?;
    decode_project_bundle_value(value, policy)
}

pub fn decode_project_bundle_value(
    value: Value,
    policy: &BundleDecodePolicy,
) -> FlyResult<DecodedProjectBundle> {
    let imported_from_raw_project = value
        .as_object()
        .is_some_and(|object| !object.contains_key("bundle_format"));
    let bundle = if imported_from_raw_project {
        if !policy.allow_raw_project {
            return Err(FlyError::InvalidProjectBundle(
                "raw project import is disabled".to_string(),
            ));
        }
        let document = GrapesJsV1Codec::decode_value(value.clone())?;
        ProjectBundle {
            bundle_format: FLY_PROJECT_BUNDLE_V1.to_string(),
            project_format: GRAPESJS_V1.to_string(),
            project_hash: document.hash().hex(),
            project_data: value,
            metadata: BundleMetadata::default(),
            extensions: Map::new(),
        }
    } else {
        serde_json::from_value::<ProjectBundle>(value)
            .map_err(|error| FlyError::InvalidProjectBundle(error.to_string()))?
    };

    if bundle.bundle_format != FLY_PROJECT_BUNDLE_V1 {
        return Err(FlyError::UnsupportedBundleFormat(bundle.bundle_format));
    }
    if policy.require_supported_project_format && bundle.project_format != GRAPESJS_V1 {
        return Err(FlyError::UnsupportedProjectFormat(bundle.project_format));
    }

    let document = GrapesJsV1Codec::decode_value(bundle.project_data.clone())?;
    let actual_hash = document.hash().hex();
    let hash_matches = constant_time_eq(bundle.project_hash.as_bytes(), actual_hash.as_bytes());
    if !hash_matches && !policy.allow_hash_mismatch {
        return Err(FlyError::ProjectBundleHashMismatch {
            declared: bundle.project_hash,
            actual: actual_hash,
        });
    }

    Ok(DecodedProjectBundle {
        bundle,
        document,
        hash_matches,
        imported_from_raw_project,
    })
}

pub fn inspect_project_bundle(
    decoded: &DecodedProjectBundle,
    registries: &RegistrySet,
    limits: ValidationLimits,
) -> BundleInspection {
    let validation = validate_project(&decoded.document, registries, limits);
    let mut audit_error_count = 0usize;
    let mut audit_warning_count = 0usize;
    for index in 0..decoded.document.project.pages.len() {
        let audit = audit_page(&decoded.document, &PageLocator::by_index(index));
        audit_error_count = audit_error_count.saturating_add(audit.error_count);
        audit_warning_count = audit_warning_count.saturating_add(audit.warning_count);
    }
    BundleInspection {
        bundle_format: decoded.bundle.bundle_format.clone(),
        project_format: decoded.bundle.project_format.clone(),
        declared_hash: decoded.bundle.project_hash.clone(),
        actual_hash: decoded.document.hash().hex(),
        hash_matches: decoded.hash_matches,
        page_count: validation.page_count,
        node_count: validation.node_count,
        asset_count: validation.asset_count,
        style_rule_count: validation.style_rule_count,
        validation,
        audit_error_count,
        audit_warning_count,
    }
}

pub fn bundle_hash(bundle: &ProjectBundle) -> FlyResult<ProjectHash> {
    let document = GrapesJsV1Codec::decode_value(bundle.project_data.clone())?;
    Ok(document.hash())
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    let mut difference = 0u8;
    for (left, right) in left.iter().zip(right) {
        difference |= left ^ right;
    }
    difference == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn document() -> ProjectDocument {
        GrapesJsV1Codec::decode_value(json!({
            "futureProjectField": { "keep": true },
            "pages": [{
                "id": "home",
                "flyPageMeta": { "title": "Home", "slug": "home" },
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "components": [{
                        "id": "heading",
                        "type": "heading",
                        "tagName": "h1",
                        "content": "Hello"
                    }]
                }
            }]
        }))
        .expect("document")
    }

    #[test]
    fn bundle_round_trip_preserves_project_and_metadata() {
        let bundle = export_project_bundle(
            &document(),
            BundleMetadata {
                name: Some("Landing".to_string()),
                source_module: Some("pages".to_string()),
                extensions: Map::from_iter([("futureMeta".to_string(), json!(true))]),
                ..BundleMetadata::default()
            },
        )
        .expect("export");
        let bytes = encode_project_bundle(&bundle, true).expect("encode");
        let decoded = decode_project_bundle(&bytes, &BundleDecodePolicy::default())
            .expect("decode");
        assert!(decoded.hash_matches);
        assert_eq!(decoded.bundle.metadata.name.as_deref(), Some("Landing"));
        assert_eq!(decoded.bundle.metadata.extensions["futureMeta"], true);
        assert_eq!(
            GrapesJsV1Codec::encode_value(&decoded.document)
                .expect("encode project")["futureProjectField"]["keep"],
            true
        );
    }

    #[test]
    fn raw_project_fallback_wraps_grapesjs_data() {
        let raw = GrapesJsV1Codec::encode_value(&document()).expect("raw project");
        let decoded = decode_project_bundle_value(raw, &BundleDecodePolicy::default())
            .expect("raw import");
        assert!(decoded.imported_from_raw_project);
        assert!(decoded.hash_matches);
        assert_eq!(decoded.bundle.bundle_format, FLY_PROJECT_BUNDLE_V1);
    }

    #[test]
    fn tampered_bundle_is_rejected_unless_policy_allows_it() {
        let mut bundle = export_project_bundle(&document(), BundleMetadata::default())
            .expect("export");
        bundle.project_data["pages"][0]["id"] = json!("tampered");
        let value = serde_json::to_value(bundle).expect("bundle value");
        assert!(matches!(
            decode_project_bundle_value(value.clone(), &BundleDecodePolicy::default()),
            Err(FlyError::ProjectBundleHashMismatch { .. })
        ));
        let decoded = decode_project_bundle_value(
            value,
            &BundleDecodePolicy {
                allow_hash_mismatch: true,
                ..BundleDecodePolicy::default()
            },
        )
        .expect("allow mismatch");
        assert!(!decoded.hash_matches);
    }

    #[test]
    fn inspection_aggregates_validation_and_audit() {
        let bundle = export_project_bundle(&document(), BundleMetadata::default())
            .expect("export");
        let decoded = decode_project_bundle_value(
            serde_json::to_value(bundle).expect("bundle value"),
            &BundleDecodePolicy::default(),
        )
        .expect("decode");
        let inspection = inspect_project_bundle(
            &decoded,
            &RegistrySet::with_builtins(),
            ValidationLimits::default(),
        );
        assert_eq!(inspection.page_count, 1);
        assert_eq!(inspection.node_count, 2);
        assert!(inspection.hash_matches);
    }
}
