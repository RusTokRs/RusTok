use crate::{FlyError, FlyResult, GrapesProject, ProjectDocument};
use serde_json::Value;

/// Hard ceiling on JSON nesting accepted by the codec.
///
/// This matches serde_json's default parser recursion ceiling, so byte/string decoding and
/// already-parsed Value decoding enforce the same bound.
pub const MAXIMUM_DECODE_DEPTH: usize = 128;

pub struct GrapesJsCodec;

impl GrapesJsCodec {
    pub fn decode_slice(input: &[u8]) -> FlyResult<ProjectDocument> {
        let value: Value =
            serde_json::from_slice(input).map_err(|error| FlyError::Decode(error.to_string()))?;
        Self::decode_value(value)
    }

    pub fn decode_str(input: &str) -> FlyResult<ProjectDocument> {
        Self::decode_slice(input.as_bytes())
    }

    pub fn decode_value(value: Value) -> FlyResult<ProjectDocument> {
        // Check before model construction. On rejection, dismantle the Value iteratively too:
        // its ordinary destructor follows nested arrays/objects recursively.
        if let Err(error) = Self::validate_input(&value) {
            drop_value_iteratively(value);
            return Err(error);
        }
        Self::decode_checked_value(value)
    }

    /// Decode borrowed JSON without cloning an unchecked, potentially deeply nested value.
    /// The caller retains ownership, including responsibility for dropping rejected input.
    pub fn decode_ref(value: &Value) -> FlyResult<ProjectDocument> {
        Self::validate_input(value)?;
        Self::decode_checked_value(value.clone())
    }

    fn validate_input(value: &Value) -> FlyResult<()> {
        if !value.is_object() {
            return Err(FlyError::InvalidProjectRoot);
        }
        ensure_depth_within_limit(value, MAXIMUM_DECODE_DEPTH)
    }

    fn decode_checked_value(value: Value) -> FlyResult<ProjectDocument> {
        let project: GrapesProject =
            serde_json::from_value(value).map_err(|error| FlyError::Decode(error.to_string()))?;
        Ok(ProjectDocument::new(project))
    }

    pub fn encode_value(document: &ProjectDocument) -> FlyResult<Value> {
        serde_json::to_value(canonical_project(document))
            .map_err(|error| FlyError::Encode(error.to_string()))
    }

    pub fn encode_vec(document: &ProjectDocument) -> FlyResult<Vec<u8>> {
        serde_json::to_vec(canonical_project(document))
            .map_err(|error| FlyError::Encode(error.to_string()))
    }

    pub fn encode_pretty(document: &ProjectDocument) -> FlyResult<String> {
        serde_json::to_string_pretty(canonical_project(document))
            .map_err(|error| FlyError::Encode(error.to_string()))
    }
}

/**
 * Drop an untrusted JSON value without recursively dropping nested arrays or objects.
 *
 * serde_json::Value owns nested containers, and its normal destructor follows that
 * nesting recursively. A value rejected by the depth guard (or root-shape check) must
 * therefore be dismantled iteratively before returning the error.
 */
pub fn drop_value_iteratively(value: Value) {
    let mut pending = vec![value];
    while let Some(value) = pending.pop() {
        match value {
            Value::Array(values) => pending.extend(values),
            Value::Object(entries) => {
                pending.extend(entries.into_iter().map(|(_, value)| value));
            }
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
        }
    }
}

/// Reject a JSON value nested deeper than `maximum`, without recursing.
/// The root is depth 1; scalar leaves count towards the limit. Borrowed callers
/// retain ownership and may use `drop_value_iteratively` on rejected input.
pub fn ensure_depth_within_limit(value: &Value, maximum: usize) -> FlyResult<()> {
    let mut pending = vec![(value, 1usize)];
    while let Some((value, depth)) = pending.pop() {
        if depth > maximum {
            return Err(FlyError::MaximumDepthExceeded { maximum });
        }
        match value {
            Value::Array(values) => pending.extend(values.iter().map(|value| (value, depth + 1))),
            Value::Object(entries) => {
                pending.extend(entries.values().map(|value| (value, depth + 1)));
            }
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
        }
    }
    Ok(())
}

/// Borrow the canonical, encodable view of a document.
///
/// Fly's canonical editable tree is `pages[].component`. GrapesJS frame payloads are opaque
/// compatibility data: decode preserves them, but never imports from them; encode preserves them,
/// but never rewrites them as a second component-tree authority.
///
/// This used to deep-clone the entire project on every encode — and encode runs on every hash,
/// every snapshot and every save.
fn canonical_project(document: &ProjectDocument) -> &GrapesProject {
    &document.project()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ComponentPatch, EditorCommand, FlyEditor, RegistrySet};
    use serde_json::{Map, json};

    #[test]
    fn decode_preserves_frame_components_without_importing_them() {
        let document = GrapesJsCodec::decode_value(json!({
            "pages": [{
                "id": "home",
                "frames": [{
                    "id": "frame-home",
                    "component": {
                        "id": "root",
                        "type": "wrapper",
                        "components": []
                    }
                }]
            }]
        }))
        .expect("decode");

        assert!(document.project().pages[0].component.is_none());
        assert_eq!(
            document.project().pages[0].frames.as_ref().unwrap()[0]["component"]["id"],
            "root"
        );
    }

    #[test]
    fn encode_preserves_stale_frame_component_without_mirroring() {
        let document = GrapesJsCodec::decode_value(json!({
            "pages": [{
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "components": [{ "id": "current", "type": "section" }]
                },
                "frames": [{
                    "id": "frame-home",
                    "component": {
                        "id": "root",
                        "type": "wrapper",
                        "components": [{ "id": "stale", "type": "section" }]
                    }
                }]
            }]
        }))
        .expect("decode");
        let encoded = GrapesJsCodec::encode_value(&document).expect("encode");

        assert_eq!(
            encoded["pages"][0]["component"]["components"][0]["id"],
            "current"
        );
        assert_eq!(
            encoded["pages"][0]["frames"][0]["component"]["components"][0]["id"],
            "stale"
        );
        assert_eq!(encoded["pages"][0]["frames"][0]["id"], "frame-home");
    }

    #[test]
    fn encode_preserves_grapesjs_runtime_frame_scaffold() {
        let document = GrapesJsCodec::decode_value(json!({
            "pages": [{
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "components": [{ "id": "current", "type": "section" }]
                },
                "frames": [{
                    "id": "frame-home",
                    "component": {
                        "type": "wrapper",
                        "stylable": ["background", "background-color"],
                        "head": { "type": "head" },
                        "docEl": { "tagName": "html" }
                    }
                }]
            }]
        }))
        .expect("decode");
        let mut editor = FlyEditor::new(document, RegistrySet::with_builtins());
        editor
            .apply(EditorCommand::Patch {
                component_id: "current".to_string(),
                patch: ComponentPatch {
                    attributes: Map::from_iter([("data-state".to_string(), json!("edited"))]),
                    ..ComponentPatch::default()
                },
            })
            .expect("patch");

        let encoded = GrapesJsCodec::encode_value(editor.document()).expect("encode");
        assert_eq!(
            encoded["pages"][0]["component"]["components"][0]["attributes"]["data-state"],
            "edited"
        );
        assert_eq!(
            encoded["pages"][0]["frames"][0]["component"],
            json!({
                "type": "wrapper",
                "stylable": ["background", "background-color"],
                "head": { "type": "head" },
                "docEl": { "tagName": "html" }
            })
        );
    }

    #[test]
    fn project_hash_matches_encoded_bytes_after_component_mutation() {
        let document = GrapesJsCodec::decode_value(json!({
            "pages": [{
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "components": [{ "id": "hero", "type": "section" }]
                },
                "frames": [{
                    "component": {
                        "id": "root",
                        "type": "wrapper",
                        "components": [{ "id": "hero", "type": "section" }]
                    }
                }]
            }]
        }))
        .expect("decode");
        let mut editor = FlyEditor::new(document, RegistrySet::with_builtins());
        editor
            .apply(EditorCommand::Patch {
                component_id: "hero".to_string(),
                patch: ComponentPatch {
                    attributes: Map::from_iter([("data-state".to_string(), json!("edited"))]),
                    ..ComponentPatch::default()
                },
            })
            .expect("patch");

        let bytes = GrapesJsCodec::encode_vec(editor.document()).expect("encode");
        assert_eq!(
            editor.revision().project_hash,
            crate::ProjectHash::from_bytes(&bytes)
        );
    }

    #[test]
    fn decode_rejects_pathologically_nested_payloads_without_recursing() {
        let mut value = json!({ "type": "text" });
        for _ in 0..(MAXIMUM_DECODE_DEPTH + 50) {
            value = json!({ "type": "wrapper", "components": [value] });
        }
        let project = json!({ "pages": [{ "component": value }] });

        assert!(matches!(
            GrapesJsCodec::decode_value(project),
            Err(FlyError::MaximumDepthExceeded { .. })
        ));
    }

    #[test]
    fn decode_accepts_documents_within_the_depth_limit() {
        let mut value = json!({ "id": "leaf", "type": "text" });
        for index in 0..32 {
            value = json!({
                "id": format!("node-{index}"),
                "type": "wrapper",
                "components": [value]
            });
        }
        let project = json!({ "pages": [{ "id": "home", "component": value }] });

        assert!(GrapesJsCodec::decode_value(project).is_ok());
    }

    #[test]
    fn deeply_nested_non_object_roots_are_rejected_without_recursive_drop() {
        let mut value = serde_json::Value::Null;
        for _ in 0..50_000 {
            value = serde_json::Value::Array(vec![value]);
        }
        assert!(matches!(
            GrapesJsCodec::decode_value(value),
            Err(FlyError::InvalidProjectRoot)
        ));
    }

    #[test]
    fn deeply_nested_object_values_are_rejected_without_recursive_drop() {
        let mut nested = serde_json::Value::Null;
        for _ in 0..50_000 {
            nested = serde_json::Value::Array(vec![nested]);
        }
        // json! would serialize the deeply nested Value before reaching the guard.
        let project = Value::Object(serde_json::Map::from_iter([(
            "payload".to_string(),
            nested,
        )]));
        assert!(matches!(
            GrapesJsCodec::decode_value(project),
            Err(FlyError::MaximumDepthExceeded {
                maximum: MAXIMUM_DECODE_DEPTH
            })
        ));
    }

    #[test]
    fn borrowed_decoder_checks_depth_before_cloning() {
        let mut nested = Value::Null;
        for _ in 0..50_000 {
            nested = Value::Array(vec![nested]);
        }
        let project = Value::Object(Map::from_iter([("payload".to_string(), nested)]));
        let result = GrapesJsCodec::decode_ref(&project);
        drop_value_iteratively(project);
        assert!(matches!(
            result,
            Err(FlyError::MaximumDepthExceeded {
                maximum: MAXIMUM_DECODE_DEPTH
            })
        ));
    }

    #[test]
    fn borrowed_decoder_matches_owned_decoder() {
        let project = json!({"pages": [], "custom": {"opaque": true}});
        let borrowed = GrapesJsCodec::decode_ref(&project).expect("borrowed decode");
        let owned = GrapesJsCodec::decode_value(project).expect("owned decode");
        assert_eq!(
            GrapesJsCodec::encode_value(&borrowed).unwrap(),
            GrapesJsCodec::encode_value(&owned).unwrap()
        );
        assert!(matches!(
            GrapesJsCodec::decode_ref(&Value::Null),
            Err(FlyError::InvalidProjectRoot)
        ));
    }

    #[test]
    fn byte_decoder_obeys_the_same_depth_ceiling() {
        let depth = MAXIMUM_DECODE_DEPTH + 1;
        let input = format!("{}0{}", "[".repeat(depth), "]".repeat(depth));
        assert!(matches!(
            GrapesJsCodec::decode_slice(input.as_bytes()),
            Err(FlyError::Decode(_))
        ));
    }
}
