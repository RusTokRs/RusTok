use async_trait::async_trait;
use rustok_events::{
    BLOG_COMMENTS_SCHEDULE_AUDIT_SCHEMA_VERSION, BLOG_COMMENTS_SCHEDULE_AUDIT_STATE_KEY,
    BlogCommentsDelegationScheduleAuditEvent,
};
use rustok_outbox::{ContractEventWriteOnceError, TransactionalEventBus};
use sea_orm::DatabaseTransaction;
use uuid::Uuid;

use super::keyring_schedule_audit_publication::{
    CommentsTcpDelegationScheduleAuditCanonicalPublication,
    CommentsTcpDelegationScheduleAuditCanonicalWriteError,
    CommentsTcpDelegationScheduleAuditCanonicalWriter,
};

/// Canonical writer for one already-durable Blog Comments schedule audit fact.
///
/// The Blog audit `request_id` is used as the exact canonical envelope UUID.
/// Persistence is delegated to the `rustok-outbox` write-once primitive inside
/// the caller-owned transaction; transport delivery remains owned by
/// `OutboxRelay` after commit.
#[derive(Clone, Copy, Debug, Default)]
pub struct RustokOutboxCommentsTcpDelegationScheduleAuditCanonicalWriter;

#[async_trait]
impl CommentsTcpDelegationScheduleAuditCanonicalWriter
    for RustokOutboxCommentsTcpDelegationScheduleAuditCanonicalWriter
{
    async fn write_once_in_transaction(
        &self,
        transaction: &DatabaseTransaction,
        publication: &CommentsTcpDelegationScheduleAuditCanonicalPublication,
    ) -> std::result::Result<Uuid, CommentsTcpDelegationScheduleAuditCanonicalWriteError> {
        let event = event_from_publication(publication)?;
        TransactionalEventBus::publish_contract_once_direct_in_tx_with_envelope_id(
            transaction,
            publication.idempotency_key(),
            publication.control_plane_tenant_id(),
            Some(publication.actor_id()),
            event,
        )
        .await
        .map_err(map_write_once_error)
    }
}

fn event_from_publication(
    publication: &CommentsTcpDelegationScheduleAuditCanonicalPublication,
) -> std::result::Result<
    BlogCommentsDelegationScheduleAuditEvent,
    CommentsTcpDelegationScheduleAuditCanonicalWriteError,
> {
    let occurred_at_unix_ms = i64::try_from(publication.occurred_at_unix_ms())
        .map_err(|_| CommentsTcpDelegationScheduleAuditCanonicalWriteError::Unavailable)?;
    let previous_generation = i64::try_from(publication.previous_generation())
        .map_err(|_| CommentsTcpDelegationScheduleAuditCanonicalWriteError::Unavailable)?;
    let candidate_generation = i64::try_from(publication.candidate_generation())
        .map_err(|_| CommentsTcpDelegationScheduleAuditCanonicalWriteError::Unavailable)?;

    Ok(
        BlogCommentsDelegationScheduleAuditEvent::ReplacementSucceeded {
            audit_schema_version: BLOG_COMMENTS_SCHEDULE_AUDIT_SCHEMA_VERSION,
            request_id: publication.request_id(),
            state_key: BLOG_COMMENTS_SCHEDULE_AUDIT_STATE_KEY.to_string(),
            occurred_at_unix_ms,
            principal_kind: publication.principal_kind_text().to_string(),
            operation: publication.operation_text().to_string(),
            source: publication.source_text().to_string(),
            previous_generation,
            candidate_generation,
        },
    )
}

const fn map_write_once_error(
    error: ContractEventWriteOnceError,
) -> CommentsTcpDelegationScheduleAuditCanonicalWriteError {
    match error {
        ContractEventWriteOnceError::Conflict => {
            CommentsTcpDelegationScheduleAuditCanonicalWriteError::Conflict
        }
        ContractEventWriteOnceError::Unavailable => {
            CommentsTcpDelegationScheduleAuditCanonicalWriteError::Unavailable
        }
    }
}

#[cfg(test)]
mod tests {
    use rustok_api::AuthPrincipalKind;
    use rustok_events::ValidateEvent;

    use super::*;
    use crate::services::comments_provider_runtime::{
        CommentsTcpDelegationKeyringSource, CommentsTcpDelegationScheduleTriggerOperation,
    };

    fn publication() -> CommentsTcpDelegationScheduleAuditCanonicalPublication {
        CommentsTcpDelegationScheduleAuditCanonicalPublication::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            Uuid::new_v4(),
            AuthPrincipalKind::Service,
            CommentsTcpDelegationScheduleTriggerOperation::ReplaceHostSchedule,
            CommentsTcpDelegationKeyringSource::HostProvided,
            1,
            1,
            2,
        )
        .expect("bounded publication")
    }

    #[test]
    fn maps_the_exact_bounded_audit_fact_into_the_registered_event() {
        let publication = publication();
        let event = event_from_publication(&publication).expect("registered event");
        event.validate().expect("event should validate");

        let BlogCommentsDelegationScheduleAuditEvent::ReplacementSucceeded {
            audit_schema_version,
            request_id,
            state_key,
            occurred_at_unix_ms,
            principal_kind,
            operation,
            source,
            previous_generation,
            candidate_generation,
        } = event;
        assert_eq!(audit_schema_version, 1);
        assert_eq!(request_id, publication.request_id());
        assert_eq!(state_key, publication.state_key());
        assert_eq!(occurred_at_unix_ms, 1);
        assert_eq!(principal_kind, "service");
        assert_eq!(operation, "replace_host_schedule");
        assert_eq!(source, "host_provided");
        assert_eq!(previous_generation, 1);
        assert_eq!(candidate_generation, 2);
    }

    #[test]
    fn maps_closed_outbox_errors_without_infrastructure_details() {
        assert_eq!(
            map_write_once_error(ContractEventWriteOnceError::Conflict),
            CommentsTcpDelegationScheduleAuditCanonicalWriteError::Conflict
        );
        assert_eq!(
            map_write_once_error(ContractEventWriteOnceError::Unavailable),
            CommentsTcpDelegationScheduleAuditCanonicalWriteError::Unavailable
        );
    }
}
