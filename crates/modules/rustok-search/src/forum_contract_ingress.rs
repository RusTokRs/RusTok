use std::fmt;

use chrono::Utc;
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement};
use uuid::Uuid;

use rustok_events::{
    ContractEventEnvelope, ContractEventPayload, DomainEvent, EventEnvelope,
    ForumSearchProjectionEvent,
};

use crate::forum_inbox::{ForumProjectionInbox, ForumProjectionScope};

pub const FORUM_SEARCH_CONTRACT_EVENT_TYPE: &str = "forum.search_projection.invalidation_issued";
pub const FORUM_SEARCH_CONTRACT_CONSUMER_GROUP: &str = "rustok-search-forum-projection-v1";
pub const FORUM_SEARCH_CONTRACT_TOPIC: &str = "domain";

const FORUM_SOURCE_MODULE: &str = "forum";
const LEGACY_ROOT_EVENT_TYPE: &str = "index.reindex_requested";
const LEGACY_ROOT_SCHEMA_VERSION: u16 = 1;
const FULL_SCOPE_KEY: &str = "forum";
const CATEGORY_SCOPE_PREFIX: &str = "forum_category:";

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ForumSearchContractIngressOutcome {
    DurablyAccepted {
        root_event_id: Uuid,
        owner_revision: i64,
    },
    IgnoredUnrelated {
        event_type: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ForumSearchContractIngressError {
    BackendUnsupported,
    MissingCausation,
    EnvelopeInvalid,
    InboxIdentityConflict,
    Storage(String),
}

impl ForumSearchContractIngressError {
    pub const fn stable_code(&self) -> &'static str {
        match self {
            Self::BackendUnsupported => "forum.search_projection.contract_backend_unsupported",
            Self::MissingCausation => "forum.search_projection.contract_causation_required",
            Self::EnvelopeInvalid => "forum.search_projection.contract_envelope_invalid",
            Self::InboxIdentityConflict => {
                "forum.search_projection.contract_inbox_identity_conflict"
            }
            Self::Storage(_) => "forum.search_projection.contract_storage_failed",
        }
    }

    pub const fn is_retryable(&self) -> bool {
        matches!(self, Self::Storage(_))
    }
}

impl fmt::Display for ForumSearchContractIngressError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.stable_code())
    }
}

impl std::error::Error for ForumSearchContractIngressError {}

#[derive(Clone)]
pub struct ForumSearchContractIngress {
    db: DatabaseConnection,
    inbox: ForumProjectionInbox,
}

impl ForumSearchContractIngress {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            inbox: ForumProjectionInbox::new(db.clone()),
            db,
        }
    }

    pub fn supports_persistent_ingress(&self) -> bool {
        self.db.get_database_backend() == DbBackend::Postgres
    }

    /// Adapts one validated Forum typed invalidation into the existing legacy-root
    /// Search inbox identity. The broker offset may be acknowledged only after this
    /// method returns `DurablyAccepted` or `IgnoredUnrelated`.
    pub async fn ingest(
        &self,
        envelope: &ContractEventEnvelope,
    ) -> Result<ForumSearchContractIngressOutcome, ForumSearchContractIngressError> {
        if !self.supports_persistent_ingress() {
            return Err(ForumSearchContractIngressError::BackendUnsupported);
        }

        let Some(adapted) = adapt_forum_invalidation(envelope)? else {
            return Ok(ForumSearchContractIngressOutcome::IgnoredUnrelated {
                event_type: envelope.event_type().to_string(),
            });
        };

        self.inbox
            .enqueue(&adapted.root_envelope, &adapted.scope)
            .await
            .map_err(|error| ForumSearchContractIngressError::Storage(error.to_string()))?;
        self.verify_durable_root(&adapted).await?;

        Ok(ForumSearchContractIngressOutcome::DurablyAccepted {
            root_event_id: adapted.root_event_id,
            owner_revision: adapted.owner_revision,
        })
    }

    async fn verify_durable_root(
        &self,
        adapted: &AdaptedForumInvalidation,
    ) -> Result<(), ForumSearchContractIngressError> {
        let row = self
            .db
            .query_one_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"
                SELECT tenant_id, source_module, scope_key, event_type, envelope_json
                FROM search_projection_inbox
                WHERE event_id = $1
                "#,
                vec![adapted.root_event_id.into()],
            ))
            .await
            .map_err(|error| ForumSearchContractIngressError::Storage(error.to_string()))?
            .ok_or_else(|| {
                ForumSearchContractIngressError::Storage(
                    "durable Forum projection inbox row disappeared after insert".to_string(),
                )
            })?;

        let tenant_id: Uuid = row
            .try_get("", "tenant_id")
            .map_err(|error| ForumSearchContractIngressError::Storage(error.to_string()))?;
        let source_module: String = row
            .try_get("", "source_module")
            .map_err(|error| ForumSearchContractIngressError::Storage(error.to_string()))?;
        let scope_key: String = row
            .try_get("", "scope_key")
            .map_err(|error| ForumSearchContractIngressError::Storage(error.to_string()))?;
        let event_type: String = row
            .try_get("", "event_type")
            .map_err(|error| ForumSearchContractIngressError::Storage(error.to_string()))?;
        let envelope_json: serde_json::Value = row
            .try_get("", "envelope_json")
            .map_err(|error| ForumSearchContractIngressError::Storage(error.to_string()))?;
        let stored_envelope: EventEnvelope = serde_json::from_value(envelope_json)
            .map_err(|_| ForumSearchContractIngressError::InboxIdentityConflict)?;

        let expected_event = &adapted.root_envelope.event;
        let identity_matches = tenant_id == adapted.root_envelope.tenant_id
            && source_module == FORUM_SOURCE_MODULE
            && scope_key == adapted.scope_key
            && event_type == LEGACY_ROOT_EVENT_TYPE
            && stored_envelope.id == adapted.root_event_id
            && stored_envelope.tenant_id == adapted.root_envelope.tenant_id
            && stored_envelope.event_type == LEGACY_ROOT_EVENT_TYPE
            && stored_envelope.schema_version == LEGACY_ROOT_SCHEMA_VERSION
            && stored_envelope.correlation_id == adapted.root_event_id
            && stored_envelope.event == *expected_event
            && stored_envelope.validate_registered_schema().is_ok();
        if !identity_matches {
            return Err(ForumSearchContractIngressError::InboxIdentityConflict);
        }
        Ok(())
    }
}

struct AdaptedForumInvalidation {
    root_event_id: Uuid,
    owner_revision: i64,
    root_envelope: EventEnvelope,
    scope: ForumProjectionScope,
    scope_key: String,
}

fn adapt_forum_invalidation(
    envelope: &ContractEventEnvelope,
) -> Result<Option<AdaptedForumInvalidation>, ForumSearchContractIngressError> {
    let payload = envelope
        .payload()
        .map_err(|_| ForumSearchContractIngressError::EnvelopeInvalid)?;
    let ContractEventPayload::ForumSearchProjection(
        ForumSearchProjectionEvent::InvalidationIssued {
            owner_revision,
            target_type,
            target_id,
        },
    ) = payload
    else {
        return Ok(None);
    };
    if envelope.event_type() != FORUM_SEARCH_CONTRACT_EVENT_TYPE {
        return Err(ForumSearchContractIngressError::EnvelopeInvalid);
    }

    let root_event_id = envelope
        .causation_id()
        .ok_or(ForumSearchContractIngressError::MissingCausation)?;
    let (scope, scope_key) = match (target_type.as_str(), target_id) {
        ("forum", None) | ("forum_topic", Some(_)) => {
            (ForumProjectionScope::Full, FULL_SCOPE_KEY.to_string())
        }
        ("forum_category", Some(category_id)) => (
            ForumProjectionScope::Category(*category_id),
            format!("{CATEGORY_SCOPE_PREFIX}{category_id}"),
        ),
        _ => return Err(ForumSearchContractIngressError::EnvelopeInvalid),
    };
    let event = DomainEvent::ReindexRequested {
        target_type: target_type.clone(),
        target_id: *target_id,
    };
    let root_envelope = EventEnvelope {
        id: root_event_id,
        event_type: LEGACY_ROOT_EVENT_TYPE.to_string(),
        schema_version: LEGACY_ROOT_SCHEMA_VERSION,
        correlation_id: root_event_id,
        causation_id: None,
        tenant_id: envelope.tenant_id(),
        trace_id: None,
        timestamp: Utc::now(),
        actor_id: None,
        event,
        retry_count: 0,
    };
    root_envelope
        .validate_registered_schema()
        .map_err(|_| ForumSearchContractIngressError::EnvelopeInvalid)?;

    Ok(Some(AdaptedForumInvalidation {
        root_event_id,
        owner_revision: *owner_revision,
        root_envelope,
        scope,
        scope_key,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caused_invalidation_reuses_exact_legacy_root_identity() {
        let tenant_id = Uuid::new_v4();
        let root_event_id = Uuid::new_v4();
        let category_id = Uuid::new_v4();
        let envelope = ContractEventEnvelope::new_caused_by(
            tenant_id,
            None,
            root_event_id,
            ForumSearchProjectionEvent::InvalidationIssued {
                owner_revision: 7,
                target_type: "forum_category".to_string(),
                target_id: Some(category_id),
            },
        )
        .expect("typed invalidation should validate");

        let adapted = adapt_forum_invalidation(&envelope)
            .expect("adapter should accept valid contract")
            .expect("Forum contract should be relevant");
        assert_eq!(adapted.root_event_id, root_event_id);
        assert_eq!(adapted.root_envelope.id, root_event_id);
        assert_eq!(adapted.root_envelope.tenant_id, tenant_id);
        assert_eq!(adapted.scope_key, format!("forum_category:{category_id}"));
        assert_eq!(
            adapted.root_envelope.event,
            DomainEvent::ReindexRequested {
                target_type: "forum_category".to_string(),
                target_id: Some(category_id),
            }
        );
    }

    #[test]
    fn missing_root_causation_fails_closed() {
        let envelope = ContractEventEnvelope::new(
            Uuid::new_v4(),
            None,
            ForumSearchProjectionEvent::InvalidationIssued {
                owner_revision: 1,
                target_type: "forum".to_string(),
                target_id: None,
            },
        )
        .expect("uncausated envelope remains schema-valid");

        assert!(matches!(
            adapt_forum_invalidation(&envelope),
            Err(ForumSearchContractIngressError::MissingCausation)
        ));
    }
}
