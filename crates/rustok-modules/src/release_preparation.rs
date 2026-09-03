//! Release Preparation domain modeling explicit preparation identity,
//! platform-public vs tenant-private authorization, RLS domain isolation,
//! and sanitized evidence projections.
//!
//! Enforces:
//! - Explicit `preparation_id`.
//! - Preparation metadata is shared across tenants ONLY for platform-authorized public catalog releases.
//! - Private tenant preparations never share metadata, authority, or raw logs with other tenants.
//! - Globally deduplicates only immutable CAS bytes.
//! - Each production transition receives an isolated `operation_id` referencing the authorized preparation.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

use crate::{ModuleInstallationScope, OciArtifactReference};

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ReleasePreparationError {
    #[error("Invalid release preparation state transition from `{from}` to `{to}`")]
    InvalidStateTransition { from: String, to: String },
    #[error("Release preparation `{0}` is already in a terminal state")]
    AlreadyTerminal(Uuid),
    #[error(
        "Tenant `{target_tenant}` is unauthorized to access private preparation `{preparation_id}` of another tenant"
    )]
    UnauthorizedTenant {
        preparation_id: Uuid,
        target_tenant: Uuid,
    },
    #[error(
        "Transition operation derivation requires an active admitted preparation, but preparation `{0}` is in state `{1}`"
    )]
    NotAdmitted(Uuid, String),
}

/// Lifecycle state machine for release preparation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "phase")]
pub enum ReleasePreparationState {
    Received {
        received_at: DateTime<Utc>,
    },
    Verifying {
        started_at: DateTime<Utc>,
    },
    Validating {
        started_at: DateTime<Utc>,
    },
    Publishing {
        started_at: DateTime<Utc>,
    },
    Admitted {
        admitted_at: DateTime<Utc>,
        release_digest: String,
    },
    Rejected {
        reason: String,
        rejected_at: DateTime<Utc>,
    },
}

impl ReleasePreparationState {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Received { .. } => "received",
            Self::Verifying { .. } => "verifying",
            Self::Validating { .. } => "validating",
            Self::Publishing { .. } => "publishing",
            Self::Admitted { .. } => "admitted",
            Self::Rejected { .. } => "rejected",
        }
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Admitted { .. } | Self::Rejected { .. })
    }

    pub fn is_admitted(&self) -> bool {
        matches!(self, Self::Admitted { .. })
    }
}

/// Sanitized evidence projection for downstream observers, dashboards, and audit summaries.
///
/// Intentionally excludes raw host logs, internal traces, tokens, and credentials.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SanitizedPreparationEvidence {
    pub preparation_id: Uuid,
    pub scope: ModuleInstallationScope,
    pub state_name: String,
    pub artifact_digest: String,
    pub is_public_catalog: bool,
    pub timestamp: DateTime<Utc>,
}

/// Explicit release preparation model with tenancy and RLS isolation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleasePreparation {
    pub preparation_id: Uuid,
    pub scope: ModuleInstallationScope,
    pub reference: OciArtifactReference,
    pub state: ReleasePreparationState,
    pub is_public_catalog: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl ReleasePreparation {
    pub fn new(
        preparation_id: Uuid,
        scope: ModuleInstallationScope,
        reference: OciArtifactReference,
        is_public_catalog: bool,
    ) -> Self {
        let now = Utc::now();
        Self {
            preparation_id,
            scope,
            reference,
            state: ReleasePreparationState::Received { received_at: now },
            is_public_catalog,
            created_at: now,
            updated_at: now,
        }
    }

    /// Checks whether metadata of this preparation can be projected to or shared with `target_tenant`.
    ///
    /// Rules:
    /// - Platform-scope preparation can be shared with any tenant ONLY IF `is_public_catalog` is true.
    /// - Tenant-private preparation can ONLY be accessed by its owning tenant.
    pub fn can_share_metadata_with(&self, target_tenant: Uuid) -> bool {
        match &self.scope {
            ModuleInstallationScope::Platform => self.is_public_catalog,
            ModuleInstallationScope::Tenant { tenant_id } => *tenant_id == target_tenant,
        }
    }

    /// Generates a sanitized projection safe for external observers without leaking raw logs.
    pub fn sanitized_evidence(&self) -> SanitizedPreparationEvidence {
        SanitizedPreparationEvidence {
            preparation_id: self.preparation_id,
            scope: self.scope.clone(),
            state_name: self.state.name().to_string(),
            artifact_digest: self.reference.digest.clone(),
            is_public_catalog: self.is_public_catalog,
            timestamp: self.updated_at,
        }
    }

    /// Advances preparation from Received to Verifying.
    pub fn advance_to_verifying(&mut self) -> Result<(), ReleasePreparationError> {
        self.ensure_active()?;
        match self.state {
            ReleasePreparationState::Received { .. } => {
                self.state = ReleasePreparationState::Verifying {
                    started_at: Utc::now(),
                };
                self.updated_at = Utc::now();
                Ok(())
            }
            _ => Err(ReleasePreparationError::InvalidStateTransition {
                from: self.state.name().to_string(),
                to: "verifying".to_string(),
            }),
        }
    }

    /// Advances preparation from Verifying to Validating.
    pub fn advance_to_validating(&mut self) -> Result<(), ReleasePreparationError> {
        self.ensure_active()?;
        match self.state {
            ReleasePreparationState::Verifying { .. } => {
                self.state = ReleasePreparationState::Validating {
                    started_at: Utc::now(),
                };
                self.updated_at = Utc::now();
                Ok(())
            }
            _ => Err(ReleasePreparationError::InvalidStateTransition {
                from: self.state.name().to_string(),
                to: "validating".to_string(),
            }),
        }
    }

    /// Advances preparation from Validating to Publishing.
    pub fn advance_to_publishing(&mut self) -> Result<(), ReleasePreparationError> {
        self.ensure_active()?;
        match self.state {
            ReleasePreparationState::Validating { .. } => {
                self.state = ReleasePreparationState::Publishing {
                    started_at: Utc::now(),
                };
                self.updated_at = Utc::now();
                Ok(())
            }
            _ => Err(ReleasePreparationError::InvalidStateTransition {
                from: self.state.name().to_string(),
                to: "publishing".to_string(),
            }),
        }
    }

    /// Finalizes preparation into the Admitted terminal state.
    pub fn finalize_admitted(
        &mut self,
        release_digest: String,
    ) -> Result<(), ReleasePreparationError> {
        self.ensure_active()?;
        match self.state {
            ReleasePreparationState::Publishing { .. }
            | ReleasePreparationState::Validating { .. } => {
                self.state = ReleasePreparationState::Admitted {
                    admitted_at: Utc::now(),
                    release_digest,
                };
                self.updated_at = Utc::now();
                Ok(())
            }
            _ => Err(ReleasePreparationError::InvalidStateTransition {
                from: self.state.name().to_string(),
                to: "admitted".to_string(),
            }),
        }
    }

    /// Rejects preparation into the Rejected terminal state.
    pub fn reject(&mut self, reason: String) -> Result<(), ReleasePreparationError> {
        self.ensure_active()?;
        self.state = ReleasePreparationState::Rejected {
            reason,
            rejected_at: Utc::now(),
        };
        self.updated_at = Utc::now();
        Ok(())
    }

    /// Derives an isolated, scope-bound `operation_id` for a production transition.
    ///
    /// Validates tenant authorization and ensures that concurrent tenant installs
    /// for the same candidate release derive distinct `operation_id` domains and
    /// never share authority, correlation, or raw logs.
    pub fn derive_transition_operation_id(
        &self,
        target_tenant: Option<Uuid>,
        idempotency_key: &str,
    ) -> Result<Uuid, ReleasePreparationError> {
        if !self.state.is_admitted() {
            return Err(ReleasePreparationError::NotAdmitted(
                self.preparation_id,
                self.state.name().to_string(),
            ));
        }

        if let Some(tenant) = target_tenant {
            if !self.can_share_metadata_with(tenant) {
                return Err(ReleasePreparationError::UnauthorizedTenant {
                    preparation_id: self.preparation_id,
                    target_tenant: tenant,
                });
            }
        } else if let ModuleInstallationScope::Tenant { tenant_id } = self.scope {
            // Target is platform-wide, but preparation is tenant-private
            return Err(ReleasePreparationError::UnauthorizedTenant {
                preparation_id: self.preparation_id,
                target_tenant: tenant_id,
            });
        }

        // Derive deterministic UUID v5 from (preparation_id, target_tenant, idempotency_key)
        let tenant_str = target_tenant
            .map(|id| id.to_string())
            .unwrap_or_else(|| "platform".to_string());
        let seed = format!(
            "preparation:{}:{}:{}:{}",
            self.preparation_id, self.reference.digest, tenant_str, idempotency_key
        );
        let hash = Sha256::digest(seed.as_bytes());
        let mut uuid_bytes = [0u8; 16];
        uuid_bytes.copy_from_slice(&hash[0..16]);
        // Set UUID v4/v5 RFC variant bits
        uuid_bytes[6] = (uuid_bytes[6] & 0x0f) | 0x50;
        uuid_bytes[8] = (uuid_bytes[8] & 0x3f) | 0x80;

        Ok(Uuid::from_bytes(uuid_bytes))
    }

    fn ensure_active(&self) -> Result<(), ReleasePreparationError> {
        if self.state.is_terminal() {
            Err(ReleasePreparationError::AlreadyTerminal(
                self.preparation_id,
            ))
        } else {
            Ok(())
        }
    }
}
