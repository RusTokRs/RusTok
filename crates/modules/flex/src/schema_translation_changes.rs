use sea_orm::{ConnectionTrait, DatabaseTransaction, DbBackend, Statement};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{FlexSchemaTranslationError, FlexSchemaTranslationResult};

const DELETED_REVISION_NAMESPACE: &str = "rustok-flex/schema-copy-deleted/v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlexSchemaTranslationChangeLifecycle {
    Active,
    Archived,
    Deleted,
}

impl FlexSchemaTranslationChangeLifecycle {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Archived => "archived",
            Self::Deleted => "deleted",
        }
    }

    pub fn from_is_active(is_active: bool) -> Self {
        if is_active {
            Self::Active
        } else {
            Self::Archived
        }
    }
}

/// Append one durable schema-copy change under the caller's owner transaction.
///
/// SQLite intentionally remains a no-op because ChangeCursor is a PostgreSQL production
/// capability; this keeps lightweight owner tests usable without inventing weaker cursor
/// guarantees. PostgreSQL writes are protected by `(root_event_id, schema_id)` dedupe.
pub async fn record_flex_schema_translation_change_in_tx(
    txn: &DatabaseTransaction,
    root_event_id: Uuid,
    tenant_id: Uuid,
    schema_id: Uuid,
    resource_revision: &str,
    lifecycle: FlexSchemaTranslationChangeLifecycle,
) -> FlexSchemaTranslationResult<()> {
    if txn.get_database_backend() != DbBackend::Postgres {
        return Ok(());
    }
    if root_event_id.is_nil() || tenant_id.is_nil() || schema_id.is_nil() {
        return Err(FlexSchemaTranslationError::OwnerInvariant(
            "Flex schema translation change journal identity must not be nil".to_string(),
        ));
    }
    if resource_revision.trim().is_empty() || resource_revision.len() > 128 {
        return Err(FlexSchemaTranslationError::OwnerInvariant(
            "Flex schema translation change journal revision must contain 1..=128 non-whitespace bytes"
                .to_string(),
        ));
    }

    txn.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
INSERT INTO flex_schema_translation_change_journal (
    root_event_id,
    tenant_id,
    schema_id,
    resource_revision,
    lifecycle,
    created_at
) VALUES ($1, $2, $3, $4, $5, CURRENT_TIMESTAMP)
ON CONFLICT (root_event_id, schema_id) DO NOTHING
"#,
        vec![
            root_event_id.into(),
            tenant_id.into(),
            schema_id.into(),
            resource_revision.to_string().into(),
            lifecycle.as_str().into(),
        ],
    ))
    .await
    .map_err(|error| FlexSchemaTranslationError::Database(error.to_string()))?;
    Ok(())
}

/// Stable tombstone revision for a schema deleted after the journal was introduced.
pub fn flex_schema_translation_deleted_revision(root_event_id: Uuid, schema_id: Uuid) -> String {
    let mut hasher = Sha256::new();
    digest(&mut hasher, DELETED_REVISION_NAMESPACE);
    digest(&mut hasher, &root_event_id.to_string());
    digest(&mut hasher, &schema_id.to_string());
    format!("flex-schema-deleted-v1:{}", hex::encode(hasher.finalize()))
}

fn digest(hasher: &mut Sha256, value: &str) {
    hasher.update((value.len() as u64).to_be_bytes());
    hasher.update(value.as_bytes());
}
