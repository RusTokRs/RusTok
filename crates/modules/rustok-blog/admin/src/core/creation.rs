use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::model::BlogPostDraft;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CreationScope {
    pub actor_id: Uuid,
    pub tenant_id: Uuid,
}

impl CreationScope {
    pub fn matches_actor(self, actor: Option<&str>) -> bool {
        !self.tenant_id.is_nil()
            && !self.actor_id.is_nil()
            && actor.and_then(|value| Uuid::parse_str(value).ok()) == Some(self.actor_id)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum JournalError {
    Unavailable,
    Invalid,
    Changed,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct CreationRecord {
    version: u8,
    scope: CreationScope,
    draft: BlogPostDraft,
}

pub(crate) fn encode(scope: CreationScope, draft: &BlogPostDraft) -> Result<String, JournalError> {
    if draft.command_id.is_nil() || draft.version.is_some() {
        return Err(JournalError::Invalid);
    }
    serde_json::to_string(&CreationRecord {
        version: 1,
        scope,
        draft: draft.clone(),
    })
    .map_err(|_| JournalError::Invalid)
}

pub(crate) fn decode(scope: CreationScope, raw: &str) -> Result<BlogPostDraft, JournalError> {
    let record: CreationRecord = serde_json::from_str(raw).map_err(|_| JournalError::Invalid)?;
    if record.scope != scope {
        return Err(JournalError::Changed);
    }
    if record.version != 1 || record.draft.command_id.is_nil() || record.draft.version.is_some() {
        return Err(JournalError::Invalid);
    }
    Ok(record.draft)
}
