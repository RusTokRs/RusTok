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
//! - the stored descriptor validates and matches the installation and
//!   admission release identity. Missing or out-of-scope installation rows,
//!   missing admission rows, and corruption are identity errors, not an
//!   installation-unavailable outcome that could cancel scheduled work.

use sea_orm::{ConnectionTrait, Statement};
use thiserror::Error;
use uuid::Uuid;

use crate::ModuleArtifactDescriptor;
use crate::backend::SupportedBackend;

#[derive(Debug, Error)]
pub(crate) enum LoadAdmittedDescriptorError {
    /// The installation is not currently admissible for delivery. Callers
    /// map this to their domain "installation unavailable" outcome
    /// (cancel/dead-letter the queued item, reject the enqueue).
    #[error("installation is not admissible for delivery")]
    Unavailable,
    #[error("artifact descriptor or immutable release identity is inconsistent")]
    InvalidIdentity,
    #[error("admitted descriptor storage failed: {0}")]
    Storage(String),
}

fn storage_error(error: impl std::fmt::Display) -> LoadAdmittedDescriptorError {
    LoadAdmittedDescriptorError::Storage(error.to_string())
}

pub(crate) async fn load_admitted_descriptor<C: ConnectionTrait>(
    connection: &C,
    backend: SupportedBackend,
    tenant_id: Uuid,
    installation_id: Uuid,
) -> Result<ModuleArtifactDescriptor, LoadAdmittedDescriptorError> {
    let enabled = match backend {
        SupportedBackend::Postgres => "COALESCE(lifecycle.enabled, TRUE)",
        SupportedBackend::Sqlite => "COALESCE(lifecycle.enabled, 1)",
    };
    let true_val = match backend {
        SupportedBackend::Postgres => "TRUE",
        SupportedBackend::Sqlite => "1",
    };
    let row = connection.query_one_raw(Statement::from_sql_and_values(backend.db_backend(), format!(
        "SELECT installation.slug, installation.version, installation.payload_digest, \
                admission.payload_digest AS admission_payload_digest, admission.status AS admission_status, \
                CAST(installation.descriptor AS TEXT) AS descriptor, {enabled} AS tenant_enabled, \
                EXISTS (SELECT 1 FROM module_artifact_uninstall_operations uninstall WHERE uninstall.installation_id = installation.installation_id) AS uninstalled, \
                EXISTS (SELECT 1 FROM module_artifact_work_generations wg WHERE wg.installation_id = installation.installation_id AND wg.retired = {true_val}) AS retired, \
                EXISTS (SELECT 1 FROM module_artifact_security_states sec WHERE sec.module_slug = installation.slug AND sec.module_version = installation.version AND sec.payload_digest = installation.payload_digest AND sec.status IN ('quarantined', 'revoked')) AS blocked \
         FROM module_artifact_installations installation \
         LEFT JOIN module_artifact_admissions admission ON admission.installation_id = installation.installation_id \
         LEFT JOIN module_artifact_tenant_lifecycle lifecycle ON lifecycle.installation_id = installation.installation_id AND lifecycle.tenant_id = {} \
         WHERE installation.installation_id = {} \
           AND ((installation.scope_kind = 'tenant' AND installation.tenant_id = {}) \
                OR (installation.scope_kind = 'platform' AND installation.tenant_id IS NULL))",
        backend.placeholder(1), backend.placeholder(2), backend.placeholder(3)),
        vec![backend.uuid_value(tenant_id), backend.uuid_value(installation_id), backend.uuid_value(tenant_id)])).await.map_err(storage_error)?
        .ok_or(LoadAdmittedDescriptorError::InvalidIdentity)?;
    // A missing admission or its identity is corruption, not a normal
    // inactive status, and must not become a terminal queue decision.
    let status: Option<String> = row.try_get("", "admission_status").map_err(storage_error)?;
    let admission_digest: Option<String> = row.try_get("", "admission_payload_digest").map_err(storage_error)?;
    let status = status.ok_or(LoadAdmittedDescriptorError::InvalidIdentity)?;
    if admission_digest.is_none() {
        return Err(LoadAdmittedDescriptorError::InvalidIdentity);
    }
    // Identity must be checked before interpreting security or lifecycle
    // facts keyed by that identity; otherwise corruption can cancel work for
    // the wrong release.
    let descriptor = crate::installation_selection::descriptor_with_identity(&row)
        .map_err(storage_error)?
        .ok_or(LoadAdmittedDescriptorError::InvalidIdentity)?;
    let enabled: bool = row.try_get("", "tenant_enabled").map_err(storage_error)?;
    let uninstalled: bool = row.try_get("", "uninstalled").map_err(storage_error)?;
    let retired: bool = row.try_get("", "retired").map_err(storage_error)?;
    let blocked: bool = row.try_get("", "blocked").map_err(storage_error)?;
    if status != "active" || !enabled || uninstalled || retired || blocked {
        return Err(LoadAdmittedDescriptorError::Unavailable);
    }
    Ok(descriptor)
}
