use crate::{FlyError, FlyResult, GrapesProject, ProjectDocument};
use serde_json::{Map, Value};

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

    pub fn decode_value(mut value: Value) -> FlyResult<ProjectDocument> {
        if !value.is_object() {
            return Err(FlyError::InvalidProjectRoot);
        }
        hydrate_page_components_from_frames(&mut value);
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

fn hydrate_page_components_from_frames(project: &mut Value) {
    let Some(pages) = project
        .as_object_mut()
        .and_then(|project| project.get_mut("pages"))
        .and_then(Value::as_array_mut)
    else {
        return;
    };

    for page in pages {
        let Some(page) = page.as_object_mut() else {
            continue;
        };
        if page
            .get("component")
            .is_some_and(|component| !component.is_null())
        {
            continue;
        }
        let component = page
            .get("frames")
            .and_then(Value::as_array)
            .and_then(|frames| frames.first())
            .and_then(Value::as_object)
            .and_then(|frame| frame.get("component"))
            .cloned();
        if let Some(component) = component {
            page.insert("component".to_string(), component);
        }
    }
}

fn canonical_project(document: &ProjectDocument) -> FlyResult<GrapesProject> {
    let mut project = document.project.clone();
    for page in &mut project.pages {
        let Some(component) = page.component.as_ref() else {
            continue;
        };
        if first_frame_has_runtime_scaffold(page.frames.as_ref()) {
            continue;
        }
        let component =
            serde_json::to_value(component).map_err(|error| FlyError::Encode(error.to_string()))?;
        synchronize_first_frame(&mut page.frames, component);
    }
    Ok(project)
}

fn first_frame_has_runtime_scaffold(frames: Option<&Value>) -> bool {
    let Some(component) = frames
        .and_then(Value::as_array)
        .and_then(|frames| frames.first())
        .and_then(Value::as_object)
        .and_then(|frame| frame.get("component"))
        .and_then(Value::as_object)
    else {
        return false;
    };

    component.contains_key("docEl")
        || component
            .get("head")
            .and_then(Value::as_object)
            .and_then(|head| head.get("type"))
            .and_then(Value::as_str)
            == Some("head")
}

fn synchronize_first_frame(frames: &mut Option<Value>, component: Value) {
    match frames {
        Some(Value::Array(frames)) => {
            if frames.is_empty() {
                frames.push(Value::Object(Map::from_iter([(
                    "component".to_string(),
                    component,
                )])));
            } else if let Some(frame) = frames.first_mut().and_then(Value::as_object_mut) {
                frame.insert("component".to_string(), component);
            }
        }
        Some(Value::Null) | None => {
            *frames = Some(Value::Array(vec![Value::Object(Map::from_iter([(
                "component".to_string(),
                component,
            )]))]));
        }
        Some(_) => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ComponentPatch, EditorCommand, FlyEditor, RegistrySet};
    use serde_json::json;

    #[test]
    fn decode_hydrates_canonical_component_from_grapesjs_frame() {
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

        assert_eq!(
            document.project.pages[0]
                .component
                .as_ref()
                .and_then(|node| node.id()),
            Some("root")
        );
    }

    #[test]
    fn encode_refreshes_frame_component_from_canonical_tree() {
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
            encoded["pages"][0]["frames"][0]["component"]["components"][0]["id"],
            "current"
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
