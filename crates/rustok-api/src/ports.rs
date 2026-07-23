/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use std::time::Duration;

use serde::{Deserialize, Serialize};

const PUBLIC_UNAVAILABLE_MESSAGE: &str = "the requested capability is temporarily unavailable";
const PUBLIC_INVARIANT_MESSAGE: &str = "the requested operation could not be completed safely";

/// Transport-agnostic context that must cross module service ports.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortContext {
    pub tenant_id: String,
    pub actor: PortActor,
    pub claims: Vec<String>,
    pub roles: Vec<String>,
    pub channel: Option<String>,
    pub locale: String,
    pub correlation_id: String,
    pub causation_id: Option<String>,
    pub traceparent: Option<String>,
    pub idempotency_key: Option<String>,
    pub deadline_ms: Option<u64>,
}

impl PortContext {
    pub fn new(
        tenant_id: impl Into<String>,
        actor: PortActor,
        locale: impl Into<String>,
        correlation_id: impl Into<String>,
    ) -> Self {
        Self {
            tenant_id: tenant_id.into(),
            actor,
            claims: Vec::new(),
            roles: Vec::new(),
            channel: None,
            locale: locale.into(),
            correlation_id: correlation_id.into(),
            causation_id: None,
            traceparent: None,
            idempotency_key: None,
            deadline_ms: None,
        }
    }

    pub fn with_claim(mut self, claim: impl Into<String>) -> Self {
        self.claims.push(claim.into());
        self
    }

    pub fn with_role(mut self, role: impl Into<String>) -> Self {
        self.roles.push(role.into());
        self
    }

    pub fn with_channel(mut self, channel: impl Into<String>) -> Self {
        self.channel = Some(channel.into());
        self
    }

    pub fn with_causation_id(mut self, causation_id: impl Into<String>) -> Self {
        self.causation_id = Some(causation_id.into());
        self
    }

    pub fn with_traceparent(mut self, traceparent: impl Into<String>) -> Self {
        self.traceparent = Some(traceparent.into());
        self
    }

    pub fn with_idempotency_key(mut self, key: impl Into<String>) -> Self {
        self.idempotency_key = Some(key.into());
        self
    }

    pub fn with_deadline(mut self, deadline: Duration) -> Self {
        self.deadline_ms = Some(deadline.as_millis().min(u128::from(u64::MAX)) as u64);
        self
    }

    pub fn require_deadline_semantics(&self) -> Result<(), PortError> {
        if self.deadline_ms.unwrap_or_default() == 0 {
            return Err(PortError::timeout(
                "port.deadline_required",
                "port calls require deadline semantics",
            ));
        }
        Ok(())
    }

    pub fn require_read_semantics(&self) -> Result<(), PortError> {
        self.require_deadline_semantics()
    }

    pub fn require_write_semantics(&self) -> Result<(), PortError> {
        if self
            .idempotency_key
            .as_deref()
            .unwrap_or_default()
            .is_empty()
        {
            return Err(PortError::validation(
                "port.idempotency_key_required",
                "write port calls require a non-empty idempotency key",
            ));
        }
        self.require_deadline_semantics()
    }

    pub fn require_policy(&self, policy: PortCallPolicy) -> Result<(), PortError> {
        if policy.requires_idempotency_key {
            self.require_write_semantics()
        } else if policy.requires_deadline {
            self.require_read_semantics()
        } else {
            Ok(())
        }
    }
}

/// Shared enforcement policy for module-owned port operations.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortCallPolicy {
    pub operation: PortOperationKind,
    pub requires_deadline: bool,
    pub requires_idempotency_key: bool,
}

impl PortCallPolicy {
    pub const fn read() -> Self {
        Self {
            operation: PortOperationKind::Read,
            requires_deadline: true,
            requires_idempotency_key: false,
        }
    }

    pub const fn write() -> Self {
        Self {
            operation: PortOperationKind::Write,
            requires_deadline: true,
            requires_idempotency_key: true,
        }
    }

    pub const fn event_replay() -> Self {
        Self {
            operation: PortOperationKind::EventReplay,
            requires_deadline: true,
            requires_idempotency_key: true,
        }
    }

    pub const fn best_effort_read() -> Self {
        Self {
            operation: PortOperationKind::BestEffortRead,
            requires_deadline: false,
            requires_idempotency_key: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PortOperationKind {
    Read,
    Write,
    EventReplay,
    BestEffortRead,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortActor {
    pub kind: PortActorKind,
    pub id: String,
}

impl PortActor {
    pub fn user(id: impl Into<String>) -> Self {
        Self {
            kind: PortActorKind::User,
            id: id.into(),
        }
    }

    pub fn service(id: impl Into<String>) -> Self {
        Self {
            kind: PortActorKind::Service,
            id: id.into(),
        }
    }

    pub fn system() -> Self {
        Self {
            kind: PortActorKind::System,
            id: "system".to_string(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PortActorKind {
    User,
    Service,
    System,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortError {
    pub kind: PortErrorKind,
    pub code: String,
    pub message: String,
    pub retryable: bool,
}

impl PortError {
    pub fn validation(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(PortErrorKind::Validation, code, message, false)
    }

    pub fn timeout(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(PortErrorKind::Timeout, code, message, true)
    }

    pub fn unavailable(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(PortErrorKind::Unavailable, code, message, true)
    }

    pub fn not_found(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(PortErrorKind::NotFound, code, message, false)
    }

    pub fn conflict(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(PortErrorKind::Conflict, code, message, false)
    }

    pub fn forbidden(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(PortErrorKind::Forbidden, code, message, false)
    }

    pub fn invariant_violation(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(PortErrorKind::InvariantViolation, code, message, false)
    }

    pub fn new(
        kind: PortErrorKind,
        code: impl Into<String>,
        message: impl Into<String>,
        retryable: bool,
    ) -> Self {
        let code = code.into();
        let message = sanitize_public_message(&kind, message.into());
        Self {
            kind,
            code,
            message,
            retryable,
        }
    }
}

fn sanitize_public_message(kind: &PortErrorKind, message: String) -> String {
    match kind {
        PortErrorKind::Unavailable => PUBLIC_UNAVAILABLE_MESSAGE.to_string(),
        PortErrorKind::InvariantViolation => PUBLIC_INVARIANT_MESSAGE.to_string(),
        PortErrorKind::Validation
        | PortErrorKind::NotFound
        | PortErrorKind::Conflict
        | PortErrorKind::Forbidden
        | PortErrorKind::Timeout => message,
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PortErrorKind {
    Validation,
    NotFound,
    Conflict,
    Forbidden,
    Unavailable,
    Timeout,
    InvariantViolation,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deadline_semantics_require_non_empty_deadline() {
        let context = PortContext::new("tenant-a", PortActor::user("user-a"), "ru", "corr-a");

        assert_eq!(
            context.require_deadline_semantics().unwrap_err().kind,
            PortErrorKind::Timeout
        );

        let context = context.with_deadline(Duration::from_secs(3));
        assert!(context.require_deadline_semantics().is_ok());
    }

    #[test]
    fn write_semantics_require_idempotency_key_and_deadline() {
        let context = PortContext::new("tenant-a", PortActor::user("user-a"), "ru", "corr-a");

        assert_eq!(
            context.require_write_semantics().unwrap_err().kind,
            PortErrorKind::Validation
        );

        let context = context
            .with_idempotency_key("idem-a")
            .with_deadline(Duration::from_secs(3));
        assert!(context.require_write_semantics().is_ok());
    }

    #[test]
    fn policy_enforcement_distinguishes_read_write_and_best_effort() {
        let context = PortContext::new("tenant-a", PortActor::service("pricing"), "ru", "corr-a");

        assert_eq!(
            context
                .require_policy(PortCallPolicy::read())
                .unwrap_err()
                .kind,
            PortErrorKind::Timeout
        );
        assert!(
            context
                .clone()
                .with_deadline(Duration::from_secs(3))
                .require_policy(PortCallPolicy::read())
                .is_ok()
        );
        assert_eq!(
            context
                .clone()
                .with_deadline(Duration::from_secs(3))
                .require_policy(PortCallPolicy::write())
                .unwrap_err()
                .kind,
            PortErrorKind::Validation
        );
        assert!(
            context
                .require_policy(PortCallPolicy::best_effort_read())
                .is_ok()
        );
    }

    #[test]
    fn context_builders_preserve_cross_transport_metadata() {
        let context = PortContext::new("tenant-a", PortActor::system(), "ru", "corr-a")
            .with_claim("catalog:read")
            .with_role("operator")
            .with_channel("web")
            .with_causation_id("event-a")
            .with_traceparent("00-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-bbbbbbbbbbbbbbbb-01");

        assert_eq!(context.actor.kind, PortActorKind::System);
        assert_eq!(context.claims, vec!["catalog:read"]);
        assert_eq!(context.roles, vec!["operator"]);
        assert_eq!(context.channel.as_deref(), Some("web"));
        assert_eq!(context.causation_id.as_deref(), Some("event-a"));
        assert!(context.traceparent.is_some());
    }

    #[test]
    fn typed_error_constructors_preserve_retry_policy() {
        let not_found = PortError::not_found("catalog.not_found", "missing");
        let conflict = PortError::conflict("catalog.conflict", "duplicate");
        let forbidden = PortError::forbidden("catalog.forbidden", "denied");
        let invariant = PortError::invariant_violation("catalog.invariant", "broken");
        let unavailable = PortError::unavailable("inventory.remote_unavailable", "try later");

        assert_eq!(not_found.kind, PortErrorKind::NotFound);
        assert_eq!(conflict.kind, PortErrorKind::Conflict);
        assert_eq!(forbidden.kind, PortErrorKind::Forbidden);
        assert_eq!(invariant.kind, PortErrorKind::InvariantViolation);
        assert!(!not_found.retryable);
        assert!(!conflict.retryable);
        assert!(!forbidden.retryable);
        assert!(!invariant.retryable);
        assert_eq!(unavailable.kind, PortErrorKind::Unavailable);
        assert!(unavailable.retryable);
    }

    #[test]
    fn technical_error_messages_are_sanitized() {
        let unavailable = PortError::unavailable(
            "pricing.database_unavailable",
            "postgres://secret@host:5432 failed: relation pricing does not exist",
        );
        let invariant = PortError::invariant_violation(
            "pricing.core_error",
            "internal invariant payload with implementation details",
        );

        assert_eq!(unavailable.message, PUBLIC_UNAVAILABLE_MESSAGE);
        assert_eq!(invariant.message, PUBLIC_INVARIANT_MESSAGE);
        assert!(!unavailable.message.contains("postgres"));
        assert!(!invariant.message.contains("implementation details"));
    }

    #[test]
    fn domain_error_messages_remain_actionable() {
        let validation = PortError::validation("pricing.currency_invalid", "currency is invalid");
        let conflict = PortError::conflict("pricing.price_conflict", "price already exists");

        assert_eq!(validation.message, "currency is invalid");
        assert_eq!(conflict.message, "price already exists");
    }
}
