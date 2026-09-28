use crate::{FlyError, FlyResult, GrapesProject, ProjectDocument};
use serde_json::Value;

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
        let project: GrapesProject =
            serde_json::from_value(value).map_err(|error| FlyError::Decode(error.to_string()))?;
        Ok(ProjectDocument::new(project))
    }

    pub fn encode_value(document: &ProjectDocument) -> FlyResult<Value> {
        serde_json::to_value(canonical_project(document)?)
            .map_err(|error| FlyError::Encode(error.to_string()))
    }

    pub fn encode_vec(document: &ProjectDocument) -> FlyResult<Vec<u8>> {
        serde_json::to_vec(&canonical_project(document)?)
            .map_err(|error| FlyError::Encode(error.to_string()))
    }

    pub fn encode_pretty(document: &ProjectDocument) -> FlyResult<String> {
        serde_json::to_string_pretty(&canonical_project(document)?)
            .map_err(|error| FlyError::Encode(error.to_string()))
    }
}

fn canonical_project(document: &ProjectDocument) -> FlyResult<GrapesProject> {
    // Fly's canonical editable tree is `pages[].component`. GrapesJS frame payloads are opaque
    // compatibility data: decode preserves them, but never imports from them; encode preserves
    // them, but never rewrites them as a second component-tree authority.
    Ok(document.project.clone())
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
}
