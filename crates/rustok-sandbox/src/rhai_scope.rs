//! Serializable request-scoped Rhai bindings shared by every executor placement.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

pub const MAX_RHAI_SCOPE_CONSTANTS: usize = 32;
pub const MAX_RHAI_SCOPE_RECORDS: usize = 16;
pub const MAX_RHAI_SCOPE_NAME_BYTES: usize = 64;
pub const MAX_RHAI_SCOPE_BYTES: usize = 256 * 1024;

const RESERVED_BINDING_NAMES: &[&str] = &[
    "input",
    "EXECUTION_ID",
    "PHASE",
    "TIMESTAMP",
    "TENANT_ID",
    "ACTOR_ID",
];

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RhaiScopeInput {
    #[serde(default)]
    pub constants: BTreeMap<String, Value>,
    #[serde(default)]
    pub records: BTreeMap<String, RhaiRecordInput>,
}

impl RhaiScopeInput {
    pub fn validate(&self) -> Result<(), RhaiScopeError> {
        if self.constants.len() > MAX_RHAI_SCOPE_CONSTANTS {
            return Err(RhaiScopeError::TooManyConstants {
                limit: MAX_RHAI_SCOPE_CONSTANTS,
            });
        }
        if self.records.len() > MAX_RHAI_SCOPE_RECORDS {
            return Err(RhaiScopeError::TooManyRecords {
                limit: MAX_RHAI_SCOPE_RECORDS,
            });
        }

        let mut names = BTreeSet::new();
        for name in self.constants.keys().chain(self.records.keys()) {
            validate_binding_name(name)?;
            if !names.insert(name) {
                return Err(RhaiScopeError::DuplicateBinding(name.clone()));
            }
        }
        for record in self.records.values() {
            record.validate()?;
        }

        let encoded = serde_json::to_vec(self)
            .map_err(|error| RhaiScopeError::Serialize(error.to_string()))?;
        if encoded.len() > MAX_RHAI_SCOPE_BYTES {
            return Err(RhaiScopeError::TooLarge {
                limit: MAX_RHAI_SCOPE_BYTES,
            });
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RhaiRecordInput {
    pub id: String,
    pub record_type: String,
    #[serde(default = "empty_object")]
    pub fields: Value,
    #[serde(default)]
    pub mutable: bool,
}

impl RhaiRecordInput {
    fn validate(&self) -> Result<(), RhaiScopeError> {
        if self.id.trim().is_empty() {
            return Err(RhaiScopeError::EmptyRecordId);
        }
        if self.record_type.trim().is_empty() {
            return Err(RhaiScopeError::EmptyRecordType);
        }
        if !self.fields.is_object() {
            return Err(RhaiScopeError::RecordFieldsMustBeObject);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RhaiScopeOutput {
    #[serde(default)]
    pub record_changes: BTreeMap<String, Value>,
}

fn validate_binding_name(name: &str) -> Result<(), RhaiScopeError> {
    if RESERVED_BINDING_NAMES.contains(&name) {
        return Err(RhaiScopeError::ReservedBindingName(name.to_string()));
    }
    let mut bytes = name.bytes();
    let Some(first) = bytes.next() else {
        return Err(RhaiScopeError::InvalidBindingName(name.to_string()));
    };
    if name.len() > MAX_RHAI_SCOPE_NAME_BYTES
        || !(first.is_ascii_alphabetic() || first == b'_')
        || !bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    {
        return Err(RhaiScopeError::InvalidBindingName(name.to_string()));
    }
    Ok(())
}

fn empty_object() -> Value {
    Value::Object(serde_json::Map::new())
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum RhaiScopeError {
    #[error("Rhai scope contains more than {limit} constants")]
    TooManyConstants { limit: usize },
    #[error("Rhai scope contains more than {limit} records")]
    TooManyRecords { limit: usize },
    #[error("Rhai scope binding name `{0}` is invalid")]
    InvalidBindingName(String),
    #[error("Rhai scope binding name `{0}` is reserved by the sandbox")]
    ReservedBindingName(String),
    #[error("Rhai scope binding `{0}` is declared more than once")]
    DuplicateBinding(String),
    #[error("Rhai scope record id must not be empty")]
    EmptyRecordId,
    #[error("Rhai scope record type must not be empty")]
    EmptyRecordType,
    #[error("Rhai scope record fields must be a JSON object")]
    RecordFieldsMustBeObject,
    #[error("Rhai scope exceeds {limit} serialized bytes")]
    TooLarge { limit: usize },
    #[error("Rhai scope serialization failed: {0}")]
    Serialize(String),
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn rejects_duplicate_constant_and_record_names() {
        let scope = RhaiScopeInput {
            constants: BTreeMap::from([("entity".to_string(), Value::Null)]),
            records: BTreeMap::from([(
                "entity".to_string(),
                RhaiRecordInput {
                    id: "1".to_string(),
                    record_type: "order".to_string(),
                    fields: json!({}),
                    mutable: true,
                },
            )]),
        };

        assert_eq!(
            scope.validate(),
            Err(RhaiScopeError::DuplicateBinding("entity".to_string()))
        );
    }

    #[test]
    fn accepts_bounded_data_only_bindings() {
        let scope = RhaiScopeInput {
            constants: BTreeMap::from([("params".to_string(), json!({ "amount": 42 }))]),
            records: BTreeMap::from([(
                "entity".to_string(),
                RhaiRecordInput {
                    id: "order-1".to_string(),
                    record_type: "order".to_string(),
                    fields: json!({ "status": "pending" }),
                    mutable: true,
                },
            )]),
        };

        scope.validate().expect("valid scope");
    }

    #[test]
    fn rejects_host_owned_binding_names() {
        let scope = RhaiScopeInput {
            constants: BTreeMap::from([("input".to_string(), Value::Null)]),
            records: BTreeMap::new(),
        };

        assert_eq!(
            scope.validate(),
            Err(RhaiScopeError::ReservedBindingName("input".to_string()))
        );
    }
}
