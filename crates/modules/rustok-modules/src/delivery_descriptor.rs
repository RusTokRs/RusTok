//! Canonical "is this installation admissible for delivery right now?"
//! descriptor loader shared by the schedule and event delivery queues.
//!
//! This loader used to be copy-pasted into both queues and had already
//! diverged: the event queue's copy skipped the retired-work-generation and
//! quarantine/revocation checks, so event deliveries could be enqueued (and
//! partially processed) for installations that must never execute. There is
//! now exactly one implementation, with the strictest semantics:
//!
//! - admission is `active` and the installation was not uninstalled;
//! - no retired work generation exists for the installation;
//! - the release (slug + version + payload digest) is not quarantined or
//!   revoked;
//! - the module is enabled for the tenant (platform installations default
//!   to enabled when no tenant lifecycle row exists);
//! - the installation scope is either the tenant itself or platform;
//! - the stored descriptor still parses and validates.

use sea_orm::{ConnectionTrait, DbBackend, Statement};
use thiserror::Error;
use uuid::Uuid;

use crate::{
    ModuleArtifactDescriptor,
    data::{placeholder, uuid_value},
};

#[derive(Debug, Error)]
pub(crate) enum LoadAdmittedDescriptorError {
    /// The installation is not currently admissible for delivery. Callers
    /// map this to their domain "installation unavailable" outcome
    /// (cancel/dead-letter the queued item, reject the enqueue).
    #[error("installation is not admissible for delivery")]
    Unavailable,
    #[error("admitted descriptor storage failed: {0}")]
    Storage(String),
}

fn storage_error(error: impl std::fmt::Display) -> LoadAdmittedDescriptorError {
    LoadAdmittedDescriptorError::Storage(error.to_string())
}

pub(crate) async fn load_admitted_descriptor<C: ConnectionTrait>(
    connection: &C,
    backend: DbBackend,
    tenant_id: Uuid,
    installation_id: Uuid,
) -> Result<ModuleArtifactDescriptor, LoadAdmittedDescriptorError> {
    let enabled = match backend {
        DbBackend::Postgres => "COALESCE(lifecycle.enabled, TRUE) = TRUE",
        _ => "COALESCE(lifecycle.enabled, 1) = 1",
    };
    let true_val = match backend {
        DbBackend::Postgres => "TRUE",
        _ => "1",
    };
    let row = connection.query_one_raw(Statement::from_sql_and_values(backend, format!(
        "SELECT CAST(installation.descriptor AS TEXT) AS descriptor \
         FROM module_artifact_installations installation \
         JOIN module_artifact_admissions admission ON admission.installation_id = installation.installation_id \
         LEFT JOIN module_artifact_tenant_lifecycle lifecycle ON lifecycle.installation_id = installation.installation_id AND lifecycle.tenant_id = {} \
         WHERE installation.installation_id = {} AND admission.status = 'active' \
           AND NOT EXISTS (SELECT 1 FROM module_artifact_uninstall_operations uninstall WHERE uninstall.installation_id = installation.installation_id) \
           AND NOT EXISTS (SELECT 1 FROM module_artifact_work_generations wg WHERE wg.installation_id = installation.installation_id AND wg.retired = {true_val}) \
           AND NOT EXISTS (SELECT 1 FROM module_artifact_security_states sec WHERE sec.module_slug = installation.slug AND sec.module_version = installation.version AND sec.payload_digest = installation.payload_digest AND sec.status IN ('quarantined', 'revoked')) \
           AND {enabled} \
           AND ((installation.scope_kind = 'tenant' AND installation.tenant_id = {}) \
                OR (installation.scope_kind = 'platform' AND installation.tenant_id IS NULL))",
        placeholder(backend, 1), placeholder(backend, 2), placeholder(backend, 3)),
        vec![uuid_value(tenant_id, backend), uuid_value(installation_id, backend), uuid_value(tenant_id, backend)])).await.map_err(storage_error)?
        .ok_or(LoadAdmittedDescriptorError::Unavailable)?;
    let descriptor: ModuleArtifactDescriptor = serde_json::from_str(
        &row.try_get::<String>("", "descriptor")
            .map_err(storage_error)?,
    )
    .map_err(|_| LoadAdmittedDescriptorError::Unavailable)?;
    descriptor
        .validate()
        .map_err(|_| LoadAdmittedDescriptorError::Unavailable)?;
    Ok(descriptor)
}
