//! Transport-neutral contracts for RBAC-owned artifact permission registration.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::PortError;

/// Scope under which an admitted artifact permission becomes available to RBAC.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum ArtifactPermissionScope {
    Platform,
    Tenant { tenant_id: Uuid },
}

/// Localized, immutable operator-facing metadata for one artifact permission.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactPermissionLocalization {
    pub locale: String,
    pub label: String,
    pub description: String,
}

/// One module-owned permission registered from an admitted immutable release.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactPermissionRegistration {
    pub key: String,
    pub localizations: Vec<ArtifactPermissionLocalization>,
}

/// Idempotent request emitted only for an admitted artifact installation.
///
/// The installation identity is the idempotency key. Registration adds
/// vocabulary only; it must never assign the permission to a role or actor.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactPermissionRegistrationRequest {
    pub installation_id: Uuid,
    pub scope: ArtifactPermissionScope,
    pub module_slug: String,
    pub release_digest: String,
    pub permissions: Vec<ArtifactPermissionRegistration>,
}

/// RBAC-owned registration boundary for admitted artifact permissions.
#[async_trait]
pub trait ArtifactPermissionRegistrationPort: Send + Sync {
    async fn register_admitted_permissions(
        &self,
        request: ArtifactPermissionRegistrationRequest,
    ) -> Result<(), PortError>;
}
