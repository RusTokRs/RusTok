use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{EntityKey, FieldName, IndexValue, LinkName, LocaleKey, SchemaRef};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct LinkedEntityKey {
    pub schema: SchemaRef,
    pub entity_id: Uuid,
    pub locale: Option<LocaleKey>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexLinkValue {
    pub name: LinkName,
    pub targets: Vec<LinkedEntityKey>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexRecord {
    pub key: EntityKey,
    pub source_version: u64,
    pub fields: BTreeMap<FieldName, IndexValue>,
    pub links: Vec<IndexLinkValue>,
}
