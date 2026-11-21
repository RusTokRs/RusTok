//! Shared checkpoint fence for static settings owner mutations.
//!
//! A settings mutation holds the platform slug lock until its write commits.
//! Governed checkpoint staging takes the same lock before creating a checkpoint.
//! The check after the lock must run at READ COMMITTED on PostgreSQL.

use sea_orm::{ConnectionTrait, DatabaseTransaction};
use thiserror::Error;
use uuid::Uuid;

use crate::{ModuleInstallationScope, TransitionCheckpointStore};

#[derive(Debug, Error)]
pub(crate) enum SettingsTransitionFenceError {
    #[error("settings cannot be changed during an unresolved module transition until the applicable release schemas can be verified")]
    TransitionInProgress,
    #[error("settings transition fence failed: {0}")]
    Database(String),
}

pub(crate) async fn check<C: ConnectionTrait>(
    connection: &C,
    tenant_id: Uuid,
    module_slug: &str,
) -> Result<(), SettingsTransitionFenceError> {
    let checkpoints = TransitionCheckpointStore::list_active_checkpoints_for_slug_and_tenant_or_platform(
        connection, module_slug, tenant_id,
    )
    .await
    .map_err(|error| SettingsTransitionFenceError::Database(error.to_string()))?;
    if !checkpoints.is_empty() {
        return Err(SettingsTransitionFenceError::TransitionInProgress);
    }
    Ok(())
}

/// Call only after establishing tenant scope and receipt ownership. The
/// transaction type prevents accidentally dropping the lock before the write.
pub(crate) async fn lock_and_check(
    transaction: &DatabaseTransaction,
    tenant_id: Uuid,
    module_slug: &str,
) -> Result<(), SettingsTransitionFenceError> {
    crate::installation::acquire_artifact_activation_lock(
        transaction,
        &ModuleInstallationScope::Platform,
        module_slug,
    )
    .await
    .map_err(|error| SettingsTransitionFenceError::Database(error.to_string()))?;
    check(transaction, tenant_id, module_slug).await
}
