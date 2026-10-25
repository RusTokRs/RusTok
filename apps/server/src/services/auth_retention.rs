//! Bounded retention for authentication artifacts.
//!
//! Sessions and invite bookkeeping only need to outlive their security
//! purpose: a session row (with the previous-token hash used for reuse
//! detection) is worthless once the whole lineage has expired, and an
//! invite digest only has to survive until the signed token expires.
//! Rows are purged 30 days past expiry, so storage stays bounded
//! without weakening exactly-once acceptance or theft detection. The
//! host runs this daily via the auth retention worker; the
//! `auth sessions-cleanup` CLI remains available for one-off runs.

use chrono::{Duration, Utc};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};

use crate::error::Result;
use crate::models::{auth_invite_consumptions, auth_invites, sessions};

/// How long expired auth artifacts are kept for audit before deletion.
pub const AUTH_ARTIFACT_RETENTION_DAYS: i64 = 30;

#[derive(Debug, Default, Clone, Copy)]
pub struct AuthRetentionPurge {
    pub sessions: u64,
    pub invite_consumptions: u64,
    pub invites: u64,
}

/// Delete sessions and invite bookkeeping whose expiry is older than
/// the retention window. Returns per-table deletion counts.
pub async fn purge_expired_auth_artifacts(db: &DatabaseConnection) -> Result<AuthRetentionPurge> {
    let cutoff = Utc::now() - Duration::days(AUTH_ARTIFACT_RETENTION_DAYS);

    let sessions = sessions::Entity::delete_many()
        .filter(sessions::Column::ExpiresAt.lt(cutoff))
        .exec(db)
        .await?;

    let invite_consumptions = auth_invite_consumptions::Entity::delete_many()
        .filter(auth_invite_consumptions::Column::ExpiresAt.lt(cutoff))
        .exec(db)
        .await?;

    let invites = auth_invites::Entity::delete_many()
        .filter(auth_invites::Column::ExpiresAt.lt(cutoff))
        .exec(db)
        .await?;

    Ok(AuthRetentionPurge {
        sessions: sessions.rows_affected,
        invite_consumptions: invite_consumptions.rows_affected,
        invites: invites.rows_affected,
    })
}
