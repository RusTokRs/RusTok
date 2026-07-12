//! Stable, transport-neutral contracts for module control-plane commands.
//!
//! These types deliberately contain no SeaORM, Axum, GraphQL, or compile-time
//! module-registry types. Owner services accept them before performing a write
//! and transports map their serialized form without recreating error taxonomy.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Optimistic-concurrency revision of one durable control-plane aggregate.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ControlPlaneRevision(pub u64);

impl ControlPlaneRevision {
    pub const INITIAL: Self = Self(0);

    pub const fn next(self) -> Self {
        Self(self.0 + 1)
    }

    pub fn require(self, expected: Self) -> Result<Self, ModuleControlPlaneError> {
        if self == expected {
            Ok(self.next())
        } else {
            Err(ModuleControlPlaneError::conflict(
                ModuleErrorCode::RevisionConflict,
                "The command was based on a stale aggregate revision.",
                serde_json::json!({ "expected_revision": expected.0, "actual_revision": self.0 }),
            ))
        }
    }
}

/// Mandatory request-scoped evidence carried by every owner command.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleCommandContext {
    pub actor_id: String,
    pub tenant_id: Option<Uuid>,
    pub trace_id: String,
    pub correlation_id: String,
    pub idempotency_key: String,
}

impl ModuleCommandContext {
    pub fn validate(&self) -> Result<(), ModuleControlPlaneError> {
        for (name, value) in [
            ("actor_id", &self.actor_id),
            ("trace_id", &self.trace_id),
            ("correlation_id", &self.correlation_id),
            ("idempotency_key", &self.idempotency_key),
        ] {
            if value.trim().is_empty() {
                return Err(ModuleControlPlaneError::validation(
                    ModuleErrorCode::InvalidCommandContext,
                    format!("`{name}` must not be empty."),
                    serde_json::json!({ "field": name }),
                ));
            }
        }
        Ok(())
    }
}

/// Stable error code families exposed by owner transports.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ModuleErrorCode {
    Validation,
    NotFound,
    Conflict,
    RevisionConflict,
    PermissionDenied,
    PolicyDenied,
    DependencyConflict,
    InvalidCommandContext,
    Unavailable,
    Internal,
    Unknown(String),
}

impl ModuleErrorCode {
    pub const fn as_str(&self) -> &str {
        match self {
            Self::Validation => "validation",
            Self::NotFound => "not_found",
            Self::Conflict => "conflict",
            Self::RevisionConflict => "revision_conflict",
            Self::PermissionDenied => "permission_denied",
            Self::PolicyDenied => "policy_denied",
            Self::DependencyConflict => "dependency_conflict",
            Self::InvalidCommandContext => "invalid_command_context",
            Self::Unavailable => "unavailable",
            Self::Internal => "internal",
            Self::Unknown(value) => value,
        }
    }
}

impl Serialize for ModuleErrorCode {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for ModuleErrorCode {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Ok(match value.as_str() {
            "validation" => Self::Validation,
            "not_found" => Self::NotFound,
            "conflict" => Self::Conflict,
            "revision_conflict" => Self::RevisionConflict,
            "permission_denied" => Self::PermissionDenied,
            "policy_denied" => Self::PolicyDenied,
            "dependency_conflict" => Self::DependencyConflict,
            "invalid_command_context" => Self::InvalidCommandContext,
            "unavailable" => Self::Unavailable,
            "internal" => Self::Internal,
            _ => Self::Unknown(value),
        })
    }
}

/// One serialized owner error envelope for GraphQL, native, and future worker adapters.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModuleControlPlaneError {
    pub code: ModuleErrorCode,
    pub message: String,
    pub details: serde_json::Value,
    pub retryable: bool,
}

impl ModuleControlPlaneError {
    pub fn validation(code: ModuleErrorCode, message: impl Into<String>, details: serde_json::Value) -> Self {
        Self { code, message: message.into(), details, retryable: false }
    }

    pub fn conflict(code: ModuleErrorCode, message: impl Into<String>, details: serde_json::Value) -> Self {
        Self { code, message: message.into(), details, retryable: false }
    }
}

/// Kinds of durable control-plane state that have a serializable snapshot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModuleSnapshotKind {
    Catalog,
    Release,
    Artifact,
    Installation,
    EffectivePolicy,
    Composition,
    Governance,
    Lifecycle,
    Recovery,
    Build,
}

/// Transport-neutral snapshot envelope. `state` is a versioned owner DTO for
/// the selected kind, keeping transport evolution independent from persistence.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModuleControlPlaneSnapshot {
    pub kind: ModuleSnapshotKind,
    pub aggregate_id: String,
    pub revision: ControlPlaneRevision,
    pub captured_at: DateTime<Utc>,
    pub state: serde_json::Value,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context() -> ModuleCommandContext {
        ModuleCommandContext {
            actor_id: "user:42".into(),
            tenant_id: Some(Uuid::nil()),
            trace_id: "trace-1".into(),
            correlation_id: "correlation-1".into(),
            idempotency_key: "install:1".into(),
        }
    }

    #[test]
    fn contracts_round_trip_through_json() {
        let snapshot = ModuleControlPlaneSnapshot {
            kind: ModuleSnapshotKind::Installation,
            aggregate_id: "tenant:00000000-0000-0000-0000-000000000000/sample".into(),
            revision: ControlPlaneRevision(4),
            captured_at: Utc::now(),
            state: serde_json::json!({ "status": "installed" }),
        };
        let decoded: ModuleControlPlaneSnapshot =
            serde_json::from_str(&serde_json::to_string(&snapshot).expect("serialize"))
                .expect("deserialize");
        assert_eq!(decoded, snapshot);
    }

    #[test]
    fn unknown_error_codes_remain_explicit_and_round_trip() {
        let error: ModuleControlPlaneError = serde_json::from_value(serde_json::json!({
            "code": "future_code", "message": "future", "details": {}, "retryable": false
        }))
        .expect("deserialize");
        assert_eq!(error.code, ModuleErrorCode::Unknown("future_code".into()));
        assert_eq!(serde_json::to_value(error).expect("serialize")["code"], "future_code");
    }

    #[test]
    fn stale_revision_cannot_advance() {
        assert!(matches!(
            ControlPlaneRevision(3).require(ControlPlaneRevision(2)),
            Err(ModuleControlPlaneError { code: ModuleErrorCode::RevisionConflict, .. })
        ));
    }

    #[test]
    fn command_context_requires_audit_and_idempotency_evidence() {
        context().validate().expect("valid context");
        let mut invalid = context();
        invalid.idempotency_key.clear();
        assert!(matches!(
            invalid.validate(),
            Err(ModuleControlPlaneError { code: ModuleErrorCode::InvalidCommandContext, .. })
        ));
    }
}
