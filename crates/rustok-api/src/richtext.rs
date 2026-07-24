/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;

use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value as JsonValue;
use thiserror::Error;
use utoipa::ToSchema;

/// Canonical ProseMirror/Tiptap root document transported across RusToK
/// boundaries.
///
/// The executable node, mark, attribute, and size policy intentionally lives
/// in `rustok-content::richtext`; this type only defines the neutral wire
/// shape. Locale and schema versions are owner context, not document fields.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct RichTextDocument {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub content: Vec<RichTextNode>,
}

impl RichTextDocument {
    pub fn empty() -> Self {
        Self {
            kind: "doc".to_string(),
            content: Vec::new(),
        }
    }

    pub fn single_paragraph(text: impl Into<String>) -> Self {
        Self {
            kind: "doc".to_string(),
            content: vec![RichTextNode {
                kind: "paragraph".to_string(),
                attrs: BTreeMap::new(),
                content: vec![RichTextNode {
                    kind: "text".to_string(),
                    attrs: BTreeMap::new(),
                    content: Vec::new(),
                    marks: Vec::new(),
                    text: Some(text.into()),
                }],
                marks: Vec::new(),
                text: None,
            }],
        }
    }
}

impl Default for RichTextDocument {
    fn default() -> Self {
        Self::empty()
    }
}

/// A structural ProseMirror/Tiptap node.
///
/// Attributes remain structurally neutral here because their allowed keys and
/// values differ by server-selected profile. The policy layer rejects every
/// attribute that is not explicitly registered for a node.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct RichTextNode {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub attrs: BTreeMap<String, JsonValue>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub content: Vec<RichTextNode>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub marks: Vec<RichTextMark>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

/// A structural ProseMirror/Tiptap mark.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct RichTextMark {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub attrs: BTreeMap<String, JsonValue>,
}

/// Stable identifier for a server-owned richtext profile.
///
/// Profile definitions are deliberately not represented by this type. They
/// remain executable policy in `rustok-content`.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, JsonSchema, ToSchema)]
#[serde(transparent)]
#[schemars(transparent)]
pub struct RichTextProfileId(String);

impl RichTextProfileId {
    pub const MAX_LENGTH: usize = 64;

    pub fn new(value: impl Into<String>) -> Result<Self, RichTextProfileIdError> {
        let value = value.into();
        validate_profile_id(&value)?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for RichTextProfileId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

impl fmt::Display for RichTextProfileId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl AsRef<str> for RichTextProfileId {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl FromStr for RichTextProfileId {
    type Err = RichTextProfileIdError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}

impl TryFrom<String> for RichTextProfileId {
    type Error = RichTextProfileIdError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl TryFrom<&str> for RichTextProfileId {
    type Error = RichTextProfileIdError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum RichTextProfileIdError {
    #[error("richtext profile identifier must not be empty")]
    Empty,
    #[error(
        "richtext profile identifier exceeds the maximum length of {} characters",
        RichTextProfileId::MAX_LENGTH
    )]
    TooLong,
    #[error("richtext profile identifier must start with an ASCII lowercase letter")]
    InvalidStart,
    #[error(
        "richtext profile identifier contains invalid character `{0}`; use ASCII lowercase letters, digits, or `_`"
    )]
    InvalidCharacter(char),
}

/// Read-only richtext projection. `html` is derived by the server and must
/// never be accepted as write input.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[cfg_attr(feature = "server", derive(async_graphql::SimpleObject))]
#[serde(deny_unknown_fields)]
pub struct RichTextView {
    pub document: RichTextDocument,
    pub html: String,
}

/// Return the generated structural JSON schema used by browser contract
/// generation. Executable profile constraints are exported separately by
/// `rustok-content::richtext`.
pub fn document_json_schema() -> JsonValue {
    serde_json::to_value(schemars::schema_for!(RichTextDocument))
        .expect("the RichTextDocument schema must serialize")
}

fn validate_profile_id(value: &str) -> Result<(), RichTextProfileIdError> {
    if value.is_empty() {
        return Err(RichTextProfileIdError::Empty);
    }
    if value.len() > RichTextProfileId::MAX_LENGTH {
        return Err(RichTextProfileIdError::TooLong);
    }

    let mut chars = value.chars();
    let first = chars.next().expect("non-empty value has a first character");
    if !first.is_ascii_lowercase() {
        return Err(RichTextProfileIdError::InvalidStart);
    }

    for character in chars {
        if !(character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_') {
            return Err(RichTextProfileIdError::InvalidCharacter(character));
        }
    }

    Ok(())
}

#[cfg(feature = "server")]
mod graphql {
    use async_graphql::{
        InputValueError, InputValueResult, Scalar, ScalarType, Value as GraphqlValue,
    };

    use super::{RichTextDocument, RichTextProfileId};

    #[Scalar(name = "RichText")]
    impl ScalarType for RichTextDocument {
        fn parse(value: GraphqlValue) -> InputValueResult<Self> {
            let json = value.into_json().map_err(InputValueError::custom)?;
            serde_json::from_value(json).map_err(InputValueError::custom)
        }

        fn to_value(&self) -> GraphqlValue {
            let json = serde_json::to_value(self)
                .expect("RichTextDocument contains only serializable values");
            GraphqlValue::from_json(json)
                .expect("serialized RichTextDocument is a valid GraphQL value")
        }
    }

    #[Scalar(name = "RichTextProfileId")]
    impl ScalarType for RichTextProfileId {
        fn parse(value: GraphqlValue) -> InputValueResult<Self> {
            let GraphqlValue::String(value) = value else {
                return Err(InputValueError::expected_type(value));
            };
            Self::new(value).map_err(InputValueError::custom)
        }

        fn to_value(&self) -> GraphqlValue {
            GraphqlValue::String(self.as_str().to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{RichTextDocument, RichTextProfileId, document_json_schema};

    #[test]
    fn empty_document_uses_the_canonical_root_shape() {
        assert_eq!(
            serde_json::to_value(RichTextDocument::empty()).expect("serialize"),
            json!({"type": "doc", "content": []})
        );
    }

    #[test]
    fn rejects_the_removed_envelope() {
        let error = serde_json::from_value::<RichTextDocument>(json!({
            "version": "rt_json_v1",
            "locale": "en",
            "doc": {"type": "doc", "content": []}
        }))
        .expect_err("the old envelope must not be a RichTextDocument");

        assert!(error.to_string().contains("unknown field"));
    }

    #[test]
    fn rejects_unknown_structural_fields() {
        let error = serde_json::from_value::<RichTextDocument>(json!({
            "type": "doc",
            "content": [{
                "type": "paragraph",
                "html": "<script>alert(1)</script>"
            }]
        }))
        .expect_err("unknown structural fields must fail closed");

        assert!(error.to_string().contains("unknown field"));
    }

    #[test]
    fn profile_identifier_uses_one_unversioned_stem() {
        assert_eq!(
            RichTextProfileId::new("discussion")
                .expect("valid profile")
                .as_str(),
            "discussion"
        );
        assert!(RichTextProfileId::new("comment").is_ok());
        assert!(RichTextProfileId::new("article").is_ok());
        assert!(RichTextProfileId::new("RichText").is_err());
        assert!(RichTextProfileId::new("rich-text").is_err());
    }

    #[test]
    fn generated_schema_is_an_object_contract() {
        let schema = document_json_schema();
        assert_eq!(schema["type"], "object");
        assert_eq!(schema["properties"]["type"]["type"], "string");
        assert!(schema["properties"]["content"].is_object());
    }
}
