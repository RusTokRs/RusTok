use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::AuthPrincipalKind;
use sea_orm::DatabaseTransaction;
use uuid::Uuid;

use super::{keyring, keyring_schedule_trigger as trigger};

pub const COMMENTS_TCP_DELEGATION_SCHEDULE_CANONICAL_AUDIT_EVENT_TYPE: &str =
    "blog.comments_delegation_schedule.replacement_succeeded";
pub const COMMENTS_TCP_DELEGATION_SCHEDULE_CANONICAL_AUDIT_SCHEMA_VERSION: u16 = 1;
pub const COMMENTS_TCP_DELEGATION_SCHEDULE_CANONICAL_AUDIT_STATE_KEY: &str =
    "comments_tcp_delegation_schedule";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommentsTcpDelegationScheduleAuditCanonicalWriteError {
    Conflict,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CommentsTcpDelegationScheduleAuditCanonicalPublication {
    control_plane_tenant_id: Uuid,
    request_id: Uuid,
    actor_id: Uuid,
    principal_kind: AuthPrincipalKind,
    operation: trigger::CommentsTcpDelegationScheduleTriggerOperation,
    source: keyring::CommentsTcpDelegationKeyringSource,
    occurred_at_unix_ms: u64,
    previous_generation: u64,
    candidate_generation: u64,
}

impl CommentsTcpDelegationScheduleAuditCanonicalPublication {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        control_plane_tenant_id: Uuid,
        request_id: Uuid,
        actor_id: Uuid,
        principal_kind: AuthPrincipalKind,
        operation: trigger::CommentsTcpDelegationScheduleTriggerOperation,
        source: keyring::CommentsTcpDelegationKeyringSource,
        occurred_at_unix_ms: u64,
        previous_generation: u64,
        candidate_generation: u64,
    ) -> std::result::Result<Self, String> {
        if control_plane_tenant_id.is_nil() {
            return Err(
                "Comments TCP delegation schedule canonical audit publication requires a non-nil host control-plane tenant ID"
                    .to_string(),
            );
        }
        if request_id.is_nil() {
            return Err(
                "Comments TCP delegation schedule canonical audit publication request ID must be non-nil"
                    .to_string(),
            );
        }
        if actor_id.is_nil() {
            return Err(
                "Comments TCP delegation schedule canonical audit publication actor ID must be non-nil"
                    .to_string(),
            );
        }
        if matches!(principal_kind, AuthPrincipalKind::DelegatedUser) {
            return Err(
                "Delegated users cannot publish Comments TCP delegation schedule canonical audit events"
                    .to_string(),
            );
        }
        if occurred_at_unix_ms == 0 || occurred_at_unix_ms > i64::MAX as u64 {
            return Err(
                "Comments TCP delegation schedule canonical audit timestamp must fit the positive signed 64-bit wire range"
                    .to_string(),
            );
        }
        if previous_generation == 0
            || candidate_generation <= previous_generation
            || previous_generation > i64::MAX as u64
            || candidate_generation > i64::MAX as u64
        {
            return Err(
                "Comments TCP delegation schedule canonical audit generations must be positive, strictly increasing, and fit the signed 64-bit wire range"
                    .to_string(),
            );
        }
        Ok(Self {
            control_plane_tenant_id,
            request_id,
            actor_id,
            principal_kind,
            operation,
            source,
            occurred_at_unix_ms,
            previous_generation,
            candidate_generation,
        })
    }

    pub const fn event_type(&self) -> &'static str {
        COMMENTS_TCP_DELEGATION_SCHEDULE_CANONICAL_AUDIT_EVENT_TYPE
    }

    pub const fn schema_version(&self) -> u16 {
        COMMENTS_TCP_DELEGATION_SCHEDULE_CANONICAL_AUDIT_SCHEMA_VERSION
    }

    pub const fn state_key(&self) -> &'static str {
        COMMENTS_TCP_DELEGATION_SCHEDULE_CANONICAL_AUDIT_STATE_KEY
    }

    pub fn control_plane_tenant_id(&self) -> Uuid {
        self.control_plane_tenant_id
    }

    pub fn request_id(&self) -> Uuid {
        self.request_id
    }

    pub fn idempotency_key(&self) -> Uuid {
        self.request_id
    }

    pub fn actor_id(&self) -> Uuid {
        self.actor_id
    }

    pub fn principal_kind(&self) -> AuthPrincipalKind {
        self.principal_kind
    }

    pub const fn principal_kind_text(&self) -> &'static str {
        match self.principal_kind {
            AuthPrincipalKind::DirectUser => "direct_user",
            AuthPrincipalKind::Service => "service",
            AuthPrincipalKind::DelegatedUser => "delegated_user",
        }
    }

    pub fn operation(&self) -> trigger::CommentsTcpDelegationScheduleTriggerOperation {
        self.operation
    }

    pub const fn operation_text(&self) -> &'static str {
        match self.operation {
            trigger::CommentsTcpDelegationScheduleTriggerOperation::ReloadFile => "reload_file",
            trigger::CommentsTcpDelegationScheduleTriggerOperation::ReplaceHostSchedule => {
                "replace_host_schedule"
            }
        }
    }

    pub fn source(&self) -> keyring::CommentsTcpDelegationKeyringSource {
        self.source
    }

    pub const fn source_text(&self) -> &'static str {
        match self.source {
            keyring::CommentsTcpDelegationKeyringSource::HostProvided => "host_provided",
            keyring::CommentsTcpDelegationKeyringSource::File => "file",
        }
    }

    pub fn occurred_at_unix_ms(&self) -> u64 {
        self.occurred_at_unix_ms
    }

    pub fn previous_generation(&self) -> u64 {
        self.previous_generation
    }

    pub fn candidate_generation(&self) -> u64 {
        self.candidate_generation
    }
}

#[async_trait]
pub trait CommentsTcpDelegationScheduleAuditCanonicalWriter: Send + Sync {
    /// Write one canonical platform event inside the caller-owned transaction.
    ///
    /// Implementations must preserve `publication.request_id()` as the stable
    /// idempotency identity and return the exact canonical envelope UUID written
    /// by the same transaction. A duplicate exact publication may return the
    /// existing envelope UUID; any mismatched reuse must return `Conflict`.
    async fn write_once_in_transaction(
        &self,
        transaction: &DatabaseTransaction,
        publication: &CommentsTcpDelegationScheduleAuditCanonicalPublication,
    ) -> std::result::Result<Uuid, CommentsTcpDelegationScheduleAuditCanonicalWriteError>;
}

pub type SharedCommentsTcpDelegationScheduleAuditCanonicalWriter =
    Arc<dyn CommentsTcpDelegationScheduleAuditCanonicalWriter>;

#[cfg(test)]
mod tests {
    use super::*;

    fn publication(
        principal_kind: AuthPrincipalKind,
    ) -> std::result::Result<CommentsTcpDelegationScheduleAuditCanonicalPublication, String> {
        CommentsTcpDelegationScheduleAuditCanonicalPublication::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            Uuid::new_v4(),
            principal_kind,
            trigger::CommentsTcpDelegationScheduleTriggerOperation::ReplaceHostSchedule,
            keyring::CommentsTcpDelegationKeyringSource::HostProvided,
            1,
            1,
            2,
        )
    }

    #[test]
    fn admits_bounded_direct_and_service_publications() {
        for principal_kind in [AuthPrincipalKind::DirectUser, AuthPrincipalKind::Service] {
            let publication = publication(principal_kind)
                .expect("eligible principal publication should validate");
            assert_eq!(publication.idempotency_key(), publication.request_id());
            assert_eq!(
                publication.event_type(),
                COMMENTS_TCP_DELEGATION_SCHEDULE_CANONICAL_AUDIT_EVENT_TYPE
            );
            assert_eq!(publication.schema_version(), 1);
            assert_eq!(publication.state_key(), "comments_tcp_delegation_schedule");
            assert_eq!(publication.previous_generation(), 1);
            assert_eq!(publication.candidate_generation(), 2);
        }
    }

    #[test]
    fn rejects_ineligible_identity_and_generation_inputs() {
        assert!(publication(AuthPrincipalKind::DelegatedUser).is_err());
        assert!(
            CommentsTcpDelegationScheduleAuditCanonicalPublication::new(
                Uuid::nil(),
                Uuid::new_v4(),
                Uuid::new_v4(),
                AuthPrincipalKind::Service,
                trigger::CommentsTcpDelegationScheduleTriggerOperation::ReloadFile,
                keyring::CommentsTcpDelegationKeyringSource::File,
                1,
                1,
                2,
            )
            .is_err()
        );
        assert!(
            CommentsTcpDelegationScheduleAuditCanonicalPublication::new(
                Uuid::new_v4(),
                Uuid::new_v4(),
                Uuid::new_v4(),
                AuthPrincipalKind::Service,
                trigger::CommentsTcpDelegationScheduleTriggerOperation::ReloadFile,
                keyring::CommentsTcpDelegationKeyringSource::File,
                1,
                2,
                2,
            )
            .is_err()
        );
    }

    #[test]
    fn rejects_values_outside_the_signed_wire_range() {
        let too_large = i64::MAX as u64 + 1;
        assert!(
            CommentsTcpDelegationScheduleAuditCanonicalPublication::new(
                Uuid::new_v4(),
                Uuid::new_v4(),
                Uuid::new_v4(),
                AuthPrincipalKind::Service,
                trigger::CommentsTcpDelegationScheduleTriggerOperation::ReloadFile,
                keyring::CommentsTcpDelegationKeyringSource::File,
                too_large,
                1,
                2,
            )
            .is_err()
        );
        assert!(
            CommentsTcpDelegationScheduleAuditCanonicalPublication::new(
                Uuid::new_v4(),
                Uuid::new_v4(),
                Uuid::new_v4(),
                AuthPrincipalKind::Service,
                trigger::CommentsTcpDelegationScheduleTriggerOperation::ReloadFile,
                keyring::CommentsTcpDelegationKeyringSource::File,
                1,
                i64::MAX as u64,
                too_large,
            )
            .is_err()
        );
    }
}
