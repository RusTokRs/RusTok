use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{EntityKey, IndexRecord};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum IndexMutation {
    Upsert {
        event_id: Uuid,
        record: IndexRecord,
    },
    Delete {
        event_id: Uuid,
        key: EntityKey,
        source_version: u64,
    },
}

impl IndexMutation {
    pub fn event_id(&self) -> Uuid {
        match self {
            Self::Upsert { event_id, .. } | Self::Delete { event_id, .. } => *event_id,
        }
    }

    pub fn key(&self) -> &EntityKey {
        match self {
            Self::Upsert { record, .. } => &record.key,
            Self::Delete { key, .. } => key,
        }
    }

    pub fn source_version(&self) -> u64 {
        match self {
            Self::Upsert { record, .. } => record.source_version,
            Self::Delete { source_version, .. } => *source_version,
        }
    }
}
