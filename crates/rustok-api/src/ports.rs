use std::time::Duration;

use serde::{Deserialize, Serialize};

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

    pub fn with_idempotency_key(mut self, key: impl Into<String>) -> Self {
        self.idempotency_key = Some(key.into());
        self
    }

    pub fn with_deadline(mut self, deadline: Duration) -> Self {
        self.deadline_ms = Some(deadline.as_millis().min(u128::from(u64::MAX)) as u64);
        self
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
        if self.deadline_ms.unwrap_or_default() == 0 {
            return Err(PortError::timeout(
                "port.deadline_required",
                "write port calls require deadline semantics",
            ));
        }
        Ok(())
    }
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

    pub fn new(
        kind: PortErrorKind,
        code: impl Into<String>,
        message: impl Into<String>,
        retryable: bool,
    ) -> Self {
        Self {
            kind,
            code: code.into(),
            message: message.into(),
            retryable,
        }
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
    fn unavailable_errors_are_retryable() {
        let error = PortError::unavailable("inventory.remote_unavailable", "try later");

        assert_eq!(error.kind, PortErrorKind::Unavailable);
        assert!(error.retryable);
    }
}
