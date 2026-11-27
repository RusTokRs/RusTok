use crate::codec::{MAXIMUM_DECODE_DEPTH, drop_value_iteratively, ensure_depth_within_limit};
use crate::{
    ContentDigest, FlyError, FlyResult, GrapesJsCodec, PageLocator, ProjectDocument, ProjectHash,
    RegistrySet, ValidationLimits, ValidationReport, audit_page, constant_time_eq,
    validate_project,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

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
    /// Cheap change-detection fingerprint. Not an integrity guarantee — see `content_digest`.
    pub project_hash: String,
    /// Collision-resistant digest of `project_data`.
    ///
    /// Optional only so that bundles exported before digests existed stay importable. Everything
    /// Fly exports today populates it, and import refuses a bundle whose digest is present and
    /// wrong unless the caller explicitly opts out.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_digest: Option<ContentDigest>,
    pub project_data: Value,
    #[serde(default)]
    pub metadata: BundleMetadata,
    #[serde(default, flatten)]
    pub extensions: Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BundleDecodePolicy {
    pub allow_raw_project: bool,
    /// Tolerate a stale cheap fingerprint. Says nothing about integrity.
    pub allow_hash_mismatch: bool,
    /// Tolerate a well-formed but nonmatching collision-resistant digest.
    /// Only for forensic/recovery tooling; malformed digest encodings always fail decoding.
    pub allow_digest_mismatch: bool,
    /// Reject inputs that carry no declared digest, including raw projects even when
    /// `allow_raw_project` is enabled. A locally computed digest is not verification.
    pub require_content_digest: bool,
}

impl Default for BundleDecodePolicy {
    fn default() -> Self {
        Self {
            allow_raw_project: true,
            allow_hash_mismatch: false,
            allow_digest_mismatch: false,
            require_content_digest: false,
        }
    }
}

impl BundleDecodePolicy {
    /// Strictest policy: a trusted, digest-carrying bundle or nothing.
    pub fn verified() -> Self {
        Self {
            allow_raw_project: false,
            allow_hash_mismatch: false,
            allow_digest_mismatch: false,
            require_content_digest: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DecodedProjectBundle {
    pub bundle: ProjectBundle,
    pub document: ProjectDocument,
    pub hash_matches: bool,
    /// `Some(true)` when an input digest was present and verified, `Some(false)` when it was
    /// present and wrong (only reachable with `allow_digest_mismatch`), `None` when the input
    /// carried none, including raw projects wrapped locally.
    pub digest_matches: Option<bool>,
    pub imported_from_raw_project: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BundleInspection {
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
    let project_data = GrapesJsCodec::encode_value(document)?;
    Ok(ProjectBundle {
        project_hash: document.hash().hex(),
        content_digest: Some(ContentDigest::from_json(&project_data)?),
        project_data,
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
    // Value callers can bypass serde_json's parser recursion ceiling. Reject before
    // cloning, deserializing or digesting any part of the envelope, including metadata.
    if let Err(error) = ensure_depth_within_limit(&value, MAXIMUM_DECODE_DEPTH) {
        drop_value_iteratively(value);
        return Err(error);
    }
    let imported_from_raw_project = value
        .as_object()
        .is_some_and(|object| !object.contains_key("project_hash"));
    if imported_from_raw_project {
        if !policy.allow_raw_project {
            return Err(FlyError::InvalidProjectBundle(
                "raw project import is disabled".to_string(),
            ));
        }
        if policy.require_content_digest {
            return Err(FlyError::ProjectBundleDigestMissing);
        }
        let document = GrapesJsCodec::decode_ref(&value)?;
        let bundle = ProjectBundle {
            project_hash: document.hash().hex(),
            content_digest: Some(ContentDigest::from_json(&value)?),
            project_data: value,
            metadata: BundleMetadata::default(),
            extensions: Map::new(),
        };
        // The wrapper is useful for subsequent export, but its computed digest
        // cannot verify the integrity of the original raw input.
        return Ok(DecodedProjectBundle {
            bundle,
            document,
            hash_matches: true,
            digest_matches: None,
            imported_from_raw_project: true,
        });
    }
    let bundle = serde_json::from_value::<ProjectBundle>(value)
        .map_err(|error| FlyError::InvalidProjectBundle(error.to_string()))?;

    // Integrity first: the collision-resistant digest is the only check a crafted payload cannot
    // walk around. The cheap fingerprint is verified afterwards, for drift reporting.
    let digest_matches = match bundle.content_digest.as_ref() {
        Some(declared) => {
            let actual = ContentDigest::from_json(&bundle.project_data)?;
            let matches = declared.matches(&actual);
            if !matches && !policy.allow_digest_mismatch {
                return Err(FlyError::ProjectBundleDigestMismatch {
                    declared: declared.to_string(),
                    actual: actual.to_string(),
                });
            }
            Some(matches)
        }
        None => {
            if policy.require_content_digest {
                return Err(FlyError::ProjectBundleDigestMissing);
            }
            None
        }
    };

    let document = GrapesJsCodec::decode_ref(&bundle.project_data)?;
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
        digest_matches,
        imported_from_raw_project: false,
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
    for index in 0..decoded.document.project().pages.len() {
        let audit = audit_page(&decoded.document, &PageLocator::by_index(index));
        audit_error_count = audit_error_count.saturating_add(audit.error_count);
        audit_warning_count = audit_warning_count.saturating_add(audit.warning_count);
    }
    BundleInspection {
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
    let document = GrapesJsCodec::decode_ref(&bundle.project_data)?;
    Ok(document.hash())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn document() -> ProjectDocument {
        GrapesJsCodec::decode_value(json!({
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

    fn deeply_nested_value() -> Value {
        let mut value = Value::Null;
        for _ in 0..50_000 {
            value = Value::Array(vec![value]);
        }
        value
    }

    #[test]
    fn value_import_rejects_deep_raw_and_wrapped_projects_before_recursive_work() {
        for wrapped in [false, true] {
            let mut fields = Map::new();
            if wrapped {
                fields.insert(
                    "project_hash".to_string(),
                    Value::String("unused".to_string()),
                );
                fields.insert("project_data".to_string(), deeply_nested_value());
            } else {
                fields.insert("payload".to_string(), deeply_nested_value());
            }
            assert!(matches!(
                decode_project_bundle_value(Value::Object(fields), &BundleDecodePolicy::default()),
                Err(FlyError::MaximumDepthExceeded {
                    maximum: MAXIMUM_DECODE_DEPTH
                })
            ));
        }
    }

    #[test]
    fn bundle_hash_rejects_deep_values_before_cloning() {
        let mut bundle = ProjectBundle {
            project_hash: String::new(),
            content_digest: None,
            project_data: deeply_nested_value(),
            metadata: BundleMetadata::default(),
            extensions: Map::new(),
        };
        let result = bundle_hash(&bundle);
        drop_value_iteratively(std::mem::take(&mut bundle.project_data));
        assert!(matches!(
            result,
            Err(FlyError::MaximumDepthExceeded {
                maximum: MAXIMUM_DECODE_DEPTH
            })
        ));
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
        let decoded =
            decode_project_bundle(&bytes, &BundleDecodePolicy::default()).expect("decode");
        assert!(decoded.hash_matches);
        assert_eq!(decoded.bundle.metadata.name.as_deref(), Some("Landing"));
        assert_eq!(decoded.bundle.metadata.extensions["futureMeta"], true);
        assert_eq!(
            GrapesJsCodec::encode_value(&decoded.document).expect("encode project")["futureProjectField"]
                ["keep"],
            true
        );
    }

    #[test]
    fn raw_project_fallback_wraps_grapesjs_data() {
        let raw = GrapesJsCodec::encode_value(&document()).expect("raw project");
        let decoded =
            decode_project_bundle_value(raw, &BundleDecodePolicy::default()).expect("raw import");
        assert!(decoded.imported_from_raw_project);
        assert!(decoded.hash_matches);
        assert_eq!(decoded.digest_matches, None);
        assert!(decoded.bundle.content_digest.is_some());
        let wrapped = serde_json::to_value(decoded.bundle).expect("bundle");
        let verified = decode_project_bundle_value(wrapped, &BundleDecodePolicy::verified())
            .expect("verify wrapped project");
        assert_eq!(verified.digest_matches, Some(true));
        assert!(!verified.imported_from_raw_project);
    }

    #[test]
    fn required_digest_cannot_be_synthesized_from_raw_input() {
        let raw = GrapesJsCodec::encode_value(&document()).expect("raw project");
        for allow_digest_mismatch in [false, true] {
            let policy = BundleDecodePolicy {
                allow_raw_project: true,
                require_content_digest: true,
                allow_digest_mismatch,
                ..BundleDecodePolicy::default()
            };
            assert!(matches!(
                decode_project_bundle_value(raw.clone(), &policy),
                Err(FlyError::ProjectBundleDigestMissing)
            ));
        }
    }

    #[test]
    fn tampered_bundle_is_rejected_unless_policy_allows_it() {
        let mut bundle =
            export_project_bundle(&document(), BundleMetadata::default()).expect("export");
        bundle.project_data["pages"][0]["id"] = json!("tampered");
        let value = serde_json::to_value(bundle).expect("bundle value");

        // A tampered payload is caught by the collision-resistant digest before the cheap
        // fingerprint is even consulted.
        assert!(matches!(
            decode_project_bundle_value(value.clone(), &BundleDecodePolicy::default()),
            Err(FlyError::ProjectBundleDigestMismatch { .. })
        ));

        // Tolerating the digest alone still trips the fingerprint check.
        assert!(matches!(
            decode_project_bundle_value(
                value.clone(),
                &BundleDecodePolicy {
                    allow_digest_mismatch: true,
                    ..BundleDecodePolicy::default()
                },
            ),
            Err(FlyError::ProjectBundleHashMismatch { .. })
        ));

        let decoded = decode_project_bundle_value(
            value,
            &BundleDecodePolicy {
                allow_hash_mismatch: true,
                allow_digest_mismatch: true,
                ..BundleDecodePolicy::default()
            },
        )
        .expect("allow mismatch");
        assert!(!decoded.hash_matches);
        assert_eq!(decoded.digest_matches, Some(false));
    }

    #[test]
    fn inspection_aggregates_validation_and_audit() {
        let bundle = export_project_bundle(&document(), BundleMetadata::default()).expect("export");
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

    #[test]
    fn exported_bundles_carry_a_verifiable_content_digest() {
        let bundle = export_project_bundle(&document(), BundleMetadata::default()).expect("export");
        let digest = bundle.content_digest.clone().expect("digest");
        assert_eq!(digest.algorithm(), "sha256");

        let decoded = decode_project_bundle_value(
            serde_json::to_value(&bundle).expect("value"),
            &BundleDecodePolicy::verified(),
        )
        .expect("verified import");
        assert_eq!(decoded.digest_matches, Some(true));
        assert!(decoded.hash_matches);
    }

    #[test]
    fn verified_policy_rejects_a_bundle_without_a_digest() {
        let mut bundle =
            export_project_bundle(&document(), BundleMetadata::default()).expect("export");
        bundle.content_digest = None;
        assert!(matches!(
            decode_project_bundle_value(
                serde_json::to_value(&bundle).expect("value"),
                &BundleDecodePolicy::verified(),
            ),
            Err(FlyError::ProjectBundleDigestMissing)
        ));
    }
}
