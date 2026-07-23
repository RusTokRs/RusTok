use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::contract::{ContractEventPayload, EventContract, sealed};
use crate::validation::{EventValidationError, ValidateEvent, validators};
use crate::{EventSchema, FieldSchema};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "data")]
pub enum ForumMentionEvent {
    UserMentionAdded {
        source_kind: String,
        source_id: Uuid,
        source_revision_id: i64,
        source_locale: String,
        mentioned_user_id: Uuid,
    },
    AudienceMentionAdded {
        source_kind: String,
        source_id: Uuid,
        source_revision_id: i64,
        source_locale: String,
        audience: String,
    },
}

impl ForumMentionEvent {
    pub fn event_type(&self) -> &'static str {
        match self {
            Self::UserMentionAdded { .. } => "forum.mention.user_added",
            Self::AudienceMentionAdded { .. } => "forum.mention.audience_added",
        }
    }

    pub const fn schema_version(&self) -> u16 {
        1
    }

    fn source(&self) -> (&str, &Uuid, i64, &str) {
        match self {
            Self::UserMentionAdded {
                source_kind,
                source_id,
                source_revision_id,
                source_locale,
                ..
            }
            | Self::AudienceMentionAdded {
                source_kind,
                source_id,
                source_revision_id,
                source_locale,
                ..
            } => (source_kind, source_id, *source_revision_id, source_locale),
        }
    }
}

const USER_MENTION_FIELDS: &[FieldSchema] = &[
    FieldSchema {
        name: "source_kind",
        data_type: "string",
        optional: false,
    },
    FieldSchema {
        name: "source_id",
        data_type: "uuid",
        optional: false,
    },
    FieldSchema {
        name: "source_revision_id",
        data_type: "int64",
        optional: false,
    },
    FieldSchema {
        name: "source_locale",
        data_type: "string",
        optional: false,
    },
    FieldSchema {
        name: "mentioned_user_id",
        data_type: "uuid",
        optional: false,
    },
];

const AUDIENCE_MENTION_FIELDS: &[FieldSchema] = &[
    FieldSchema {
        name: "source_kind",
        data_type: "string",
        optional: false,
    },
    FieldSchema {
        name: "source_id",
        data_type: "uuid",
        optional: false,
    },
    FieldSchema {
        name: "source_revision_id",
        data_type: "int64",
        optional: false,
    },
    FieldSchema {
        name: "source_locale",
        data_type: "string",
        optional: false,
    },
    FieldSchema {
        name: "audience",
        data_type: "string",
        optional: false,
    },
];

pub const FORUM_MENTION_EVENT_SCHEMAS: &[EventSchema] = &[
    EventSchema {
        event_type: "forum.mention.user_added",
        version: 1,
        description: "A resolved user was newly mentioned by a Forum source revision.",
        fields: USER_MENTION_FIELDS,
    },
    EventSchema {
        event_type: "forum.mention.audience_added",
        version: 1,
        description: "A typed audience was newly mentioned by a Forum source revision.",
        fields: AUDIENCE_MENTION_FIELDS,
    },
];

impl sealed::Sealed for ForumMentionEvent {}

impl EventContract for ForumMentionEvent {
    fn event_type(&self) -> &'static str {
        ForumMentionEvent::event_type(self)
    }

    fn schema_version(&self) -> u16 {
        ForumMentionEvent::schema_version(self)
    }

    fn into_contract_payload(self) -> ContractEventPayload {
        ContractEventPayload::ForumMention(self)
    }
}

impl ValidateEvent for ForumMentionEvent {
    fn validate(&self) -> Result<(), EventValidationError> {
        let (source_kind, source_id, source_revision_id, source_locale) = self.source();
        if !matches!(source_kind, "topic" | "reply") {
            return Err(EventValidationError::InvalidValue(
                "source_kind",
                "must be topic or reply".to_string(),
            ));
        }
        validators::validate_not_nil_uuid("source_id", source_id)?;
        validators::validate_range("source_revision_id", source_revision_id, 1, i64::MAX)?;
        validate_locale(source_locale)?;

        match self {
            Self::UserMentionAdded {
                mentioned_user_id, ..
            } => validators::validate_not_nil_uuid("mentioned_user_id", mentioned_user_id)?,
            Self::AudienceMentionAdded { audience, .. } => {
                if audience != "moderators" {
                    return Err(EventValidationError::InvalidValue(
                        "audience",
                        "unsupported Forum mention audience".to_string(),
                    ));
                }
            }
        }
        Ok(())
    }
}

fn validate_locale(locale: &str) -> Result<(), EventValidationError> {
    validators::validate_not_empty("source_locale", locale)?;
    validators::validate_max_length("source_locale", locale, 35)?;
    if !locale
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    {
        return Err(EventValidationError::InvalidCharacters("source_locale"));
    }
    Ok(())
}

pub fn forum_mention_event_schema(event_type: &str) -> Option<&'static EventSchema> {
    FORUM_MENTION_EVENT_SCHEMAS
        .iter()
        .find(|schema| schema.event_type == event_type)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_user_and_audience_contracts() {
        let source_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        assert!(
            ForumMentionEvent::UserMentionAdded {
                source_kind: "topic".to_string(),
                source_id,
                source_revision_id: 1,
                source_locale: "en".to_string(),
                mentioned_user_id: user_id,
            }
            .validate()
            .is_ok()
        );
        assert!(
            ForumMentionEvent::AudienceMentionAdded {
                source_kind: "reply".to_string(),
                source_id,
                source_revision_id: 2,
                source_locale: "pt-br".to_string(),
                audience: "moderators".to_string(),
            }
            .validate()
            .is_ok()
        );
    }

    #[test]
    fn rejects_unknown_audience_and_source_kind() {
        let source_id = Uuid::new_v4();
        assert!(
            ForumMentionEvent::AudienceMentionAdded {
                source_kind: "post".to_string(),
                source_id,
                source_revision_id: 1,
                source_locale: "en".to_string(),
                audience: "everyone".to_string(),
            }
            .validate()
            .is_err()
        );
    }
}
