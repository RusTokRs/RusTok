use crate::{FlyError, FlyResult, GrapesProject, ProjectDocument};
use serde_json::Value;

pub struct GrapesJsV1Codec;

impl GrapesJsV1Codec {
    pub fn decode_slice(input: &[u8]) -> FlyResult<ProjectDocument> {
        let value: Value = serde_json::from_slice(input)
            .map_err(|error| FlyError::Decode(error.to_string()))?;
        Self::decode_value(value)
    }

    pub fn decode_str(input: &str) -> FlyResult<ProjectDocument> {
        Self::decode_slice(input.as_bytes())
    }

    pub fn decode_value(value: Value) -> FlyResult<ProjectDocument> {
        if !value.is_object() {
            return Err(FlyError::InvalidProjectRoot);
        }
        let project: GrapesProject = serde_json::from_value(value)
            .map_err(|error| FlyError::Decode(error.to_string()))?;
        Ok(ProjectDocument::new(project))
    }

    pub fn encode_value(document: &ProjectDocument) -> FlyResult<Value> {
        serde_json::to_value(&document.project)
            .map_err(|error| FlyError::Encode(error.to_string()))
    }

    pub fn encode_vec(document: &ProjectDocument) -> FlyResult<Vec<u8>> {
        serde_json::to_vec(&document.project)
            .map_err(|error| FlyError::Encode(error.to_string()))
    }

    pub fn encode_pretty(document: &ProjectDocument) -> FlyResult<String> {
        serde_json::to_string_pretty(&document.project)
            .map_err(|error| FlyError::Encode(error.to_string()))
    }
}
