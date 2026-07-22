use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Deserializer, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::keys::{
    NotificationAudienceCursor, NotificationKeyError, NotificationSourceSlug,
    NotificationTargetKind, NotificationTargetRoute, NotificationTemplateKey, NotificationTypeKey,
};

pub const MAX_NOTIFICATION_TEMPLATE_FIELDS: usize = 32;
pub const MAX_NOTIFICATION_TEMPLATE_KEY_BYTES: usize = 64;
pub const MAX_NOTIFICATION_TEMPLATE_VALUE_BYTES: usize = 512;
pub const MAX_NOTIFICATION_TEMPLATE_DATA_BYTES: usize = 4 * 1024;
pub const MAX_NOTIFICATION_AUDIENCE_PAGE_SIZE: usize = 256;

#[derive(Debug, Error, Clone, Eq, PartialEq)]
pub enum NotificationContractError {
    #[error(transparent)]
    InvalidKey(#[from] NotificationKeyError),
    #[error("notification template data exceeds {MAX_NOTIFICATION_TEMPLATE_FIELDS} fields")]
    TooManyTemplateFields,
    #[error("notification template data key exceeds {MAX_NOTIFICATION_TEMPLATE_KEY_BYTES} bytes")]
    TemplateKeyTooLong,
    #[error(
        "notification template data value exceeds {MAX_NOTIFICATION_TEMPLATE_VALUE_BYTES} bytes"
    )]
    TemplateValueTooLong,
    #[error(
        "notification template data exceeds {MAX_NOTIFICATION_TEMPLATE_DATA_BYTES} total bytes"
    )]
    TemplateDataTooLarge,
    #[error("notification template data contains an invalid key")]
    InvalidTemplateDataKey,
    #[error("notification audience page exceeds {MAX_NOTIFICATION_AUDIENCE_PAGE_SIZE} recipients")]
    AudiencePageTooLarge,
    #[error("notification audience page contains a duplicate recipient")]
    DuplicateAudienceRecipient,
    #[error("notification source event revision must be greater than zero")]
    InvalidSourceRevision,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct NotificationTemplateData(BTreeMap<String, String>);

impl NotificationTemplateData {
    pub fn try_new(values: BTreeMap<String, String>) -> Result<Self, NotificationContractError> {
        if values.len() > MAX_NOTIFICATION_TEMPLATE_FIELDS {
            return Err(NotificationContractError::TooManyTemplateFields);
        }

        let mut total_bytes = 0usize;
        for (key, value) in &values {
            if key.len() > MAX_NOTIFICATION_TEMPLATE_KEY_BYTES {
                return Err(NotificationContractError::TemplateKeyTooLong);
            }
            if value.len() > MAX_NOTIFICATION_TEMPLATE_VALUE_BYTES {
                return Err(NotificationContractError::TemplateValueTooLong);
            }
            NotificationTemplateKey::new(key.as_str())
                .map_err(|_| NotificationContractError::InvalidTemplateDataKey)?;
            total_bytes = total_bytes
                .saturating_add(key.len())
                .saturating_add(value.len());
        }
        if total_bytes > MAX_NOTIFICATION_TEMPLATE_DATA_BYTES {
            return Err(NotificationContractError::TemplateDataTooLarge);
        }

        Ok(Self(values))
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).map(String::as_str)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.0
            .iter()
            .map(|(key, value)| (key.as_str(), value.as_str()))
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn into_inner(self) -> BTreeMap<String, String> {
        self.0
    }
}

impl TryFrom<BTreeMap<String, String>> for NotificationTemplateData {
    type Error = NotificationContractError;

    fn try_from(value: BTreeMap<String, String>) -> Result<Self, Self::Error> {
        Self::try_new(value)
    }
}

impl<'de> Deserialize<'de> for NotificationTemplateData {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let values = BTreeMap::<String, String>::deserialize(deserializer)?;
        Self::try_new(values).map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct NotificationSourceEventRef {
    tenant_id: Uuid,
    event_id: Uuid,
    source: NotificationSourceSlug,
    event_type: NotificationTypeKey,
    source_revision: u64,
}

impl NotificationSourceEventRef {
    pub fn new(
        tenant_id: Uuid,
        event_id: Uuid,
        source: NotificationSourceSlug,
        event_type: NotificationTypeKey,
        source_revision: u64,
    ) -> Result<Self, NotificationContractError> {
        if source_revision == 0 {
            return Err(NotificationContractError::InvalidSourceRevision);
        }
        Ok(Self {
            tenant_id,
            event_id,
            source,
            event_type,
            source_revision,
        })
    }

    pub fn tenant_id(&self) -> Uuid {
        self.tenant_id
    }

    pub fn event_id(&self) -> Uuid {
        self.event_id
    }

    pub fn source(&self) -> &NotificationSourceSlug {
        &self.source
    }

    pub fn event_type(&self) -> &NotificationTypeKey {
        &self.event_type
    }

    pub fn source_revision(&self) -> u64 {
        self.source_revision
    }
}

impl<'de> Deserialize<'de> for NotificationSourceEventRef {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct RawNotificationSourceEventRef {
            tenant_id: Uuid,
            event_id: Uuid,
            source: NotificationSourceSlug,
            event_type: NotificationTypeKey,
            source_revision: u64,
        }

        let raw = RawNotificationSourceEventRef::deserialize(deserializer)?;
        Self::new(
            raw.tenant_id,
            raw.event_id,
            raw.source,
            raw.event_type,
            raw.source_revision,
        )
        .map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NotificationTargetRef {
    pub owner: NotificationSourceSlug,
    pub kind: NotificationTargetKind,
    pub id: Uuid,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationPriority {
    Low,
    #[default]
    Normal,
    High,
    Urgent,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NotificationSemanticDescriptor {
    pub notification_type: NotificationTypeKey,
    pub template_key: NotificationTemplateKey,
    pub target: NotificationTargetRef,
    pub actor_id: Option<Uuid>,
    pub priority: NotificationPriority,
    #[serde(default)]
    pub template_data: NotificationTemplateData,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NotificationAudienceCandidate {
    pub recipient_id: Uuid,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct NotificationAudiencePage {
    recipients: Vec<NotificationAudienceCandidate>,
    next_cursor: Option<NotificationAudienceCursor>,
}

impl NotificationAudiencePage {
    pub fn try_new(
        recipients: Vec<NotificationAudienceCandidate>,
        next_cursor: Option<NotificationAudienceCursor>,
    ) -> Result<Self, NotificationContractError> {
        if recipients.len() > MAX_NOTIFICATION_AUDIENCE_PAGE_SIZE {
            return Err(NotificationContractError::AudiencePageTooLarge);
        }
        let mut unique = BTreeSet::new();
        if recipients
            .iter()
            .any(|candidate| !unique.insert(candidate.recipient_id))
        {
            return Err(NotificationContractError::DuplicateAudienceRecipient);
        }
        Ok(Self {
            recipients,
            next_cursor,
        })
    }

    pub fn empty() -> Self {
        Self {
            recipients: Vec::new(),
            next_cursor: None,
        }
    }

    pub fn recipients(&self) -> &[NotificationAudienceCandidate] {
        self.recipients.as_slice()
    }

    pub fn next_cursor(&self) -> Option<&NotificationAudienceCursor> {
        self.next_cursor.as_ref()
    }

    pub fn into_parts(
        self,
    ) -> (
        Vec<NotificationAudienceCandidate>,
        Option<NotificationAudienceCursor>,
    ) {
        (self.recipients, self.next_cursor)
    }

    pub fn is_complete(&self) -> bool {
        self.next_cursor.is_none()
    }
}

impl<'de> Deserialize<'de> for NotificationAudiencePage {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct RawNotificationAudiencePage {
            recipients: Vec<NotificationAudienceCandidate>,
            next_cursor: Option<NotificationAudienceCursor>,
        }

        let raw = RawNotificationAudiencePage::deserialize(deserializer)?;
        Self::try_new(raw.recipients, raw.next_cursor).map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum NotificationOpenAuthorization {
    Allowed { route: NotificationTargetRoute },
    Unavailable,
}
