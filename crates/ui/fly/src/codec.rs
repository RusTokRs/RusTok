use crate::{FlyError, FlyResult, GrapesProject, ProjectDocument};
use serde_json::Value;

/// Hard ceiling on JSON nesting accepted by the codec.
///
/// Far above anything a real page produces (`ValidationLimits::maximum_depth` defaults to 64) and
/// far below what would exhaust the stack.
pub const MAXIMUM_DECODE_DEPTH: usize = 512;

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
        if !value.is_object() {
            return Err(FlyError::InvalidProjectRoot);
        }
        // Checked iteratively, before anything recursive touches the payload. Model construction,
        // traversal, rendering and `Value`'s own `Drop` are all recursive, so a deeply nested
        // document would abort the process on stack exhaustion rather than return an error.
        ensure_depth_within_limit(&value, MAXIMUM_DECODE_DEPTH)?;
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

/// Reject a JSON value nested deeper than `maximum`, without recursing.
fn ensure_depth_within_limit(value: &Value, maximum: usize) -> FlyResult<()> {
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
    &document.project
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

        assert!(document.project.pages[0].component.is_none());
        assert_eq!(
            document.project.pages[0].frames.as_ref().unwrap()[0]["component"]["id"],
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
}
