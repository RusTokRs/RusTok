use async_trait::async_trait;
use sea_orm::{ConnectionTrait, DatabaseBackend, DatabaseTransaction, Statement};
use uuid::Uuid;

use rustok_core::field_schema::FlexError;

use crate::{
    FlexAttachedTranslationError, FlexAttachedTranslationResult, is_valid_flex_entity_type,
};

pub const FLEX_ATTACHED_TRANSLATION_CHANGE_JOURNAL_TABLE: &str =
    "flex_attached_translation_change_journal";
pub const MAX_FLEX_ATTACHED_TRANSLATION_CHANGE_PAGE: u16 = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlexAttachedTranslationChangeKind {
    ResourceChanged,
    ResourceDeleted,
    SchemaChanged,
}

impl FlexAttachedTranslationChangeKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ResourceChanged => "resource_changed",
            Self::ResourceDeleted => "resource_deleted",
            Self::SchemaChanged => "schema_changed",
        }
    }

    pub fn parse(value: &str) -> FlexAttachedTranslationResult<Self> {
        match value {
            "resource_changed" => Ok(Self::ResourceChanged),
            "resource_deleted" => Ok(Self::ResourceDeleted),
            "schema_changed" => Ok(Self::SchemaChanged),
            other => Err(FlexAttachedTranslationError::OwnerInvariant(format!(
                "attached Translation change journal contains invalid change kind `{other}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlexAttachedTranslationChangeRecord {
    pub change_seq: u64,
    pub entity_type: String,
    pub entity_id: Option<Uuid>,
    pub kind: FlexAttachedTranslationChangeKind,
}

impl FlexAttachedTranslationChangeRecord {
    pub fn validate(&self) -> FlexAttachedTranslationResult<()> {
        if self.change_seq == 0 {
            return Err(FlexAttachedTranslationError::OwnerInvariant(
                "attached Translation change cursor must be positive".to_string(),
            ));
        }
        if !is_valid_flex_entity_type(&self.entity_type) {
            return Err(FlexAttachedTranslationError::OwnerInvariant(format!(
                "attached Translation change has invalid entity type `{}`",
                self.entity_type
            )));
        }
        match (self.kind, self.entity_id) {
            (FlexAttachedTranslationChangeKind::SchemaChanged, None) => Ok(()),
            (FlexAttachedTranslationChangeKind::SchemaChanged, Some(_)) => Err(
                FlexAttachedTranslationError::OwnerInvariant(
                    "attached schema change must not contain an entity id".to_string(),
                ),
            ),
            (_, Some(entity_id)) if !entity_id.is_nil() => Ok(()),
            _ => Err(FlexAttachedTranslationError::OwnerInvariant(
                "attached resource change must contain a non-nil entity id".to_string(),
            )),
        }
    }
}

#[async_trait]
pub trait FlexAttachedTranslationChangeOwnerPort: Send + Sync {
    fn entity_type(&self) -> &str;

    async fn read_change_highwater(
        &self,
        tenant_id: Uuid,
    ) -> FlexAttachedTranslationResult<Option<u64>>;

    async fn read_changes(
        &self,
        tenant_id: Uuid,
        after_seq: u64,
        through_seq: u64,
        limit: u16,
    ) -> FlexAttachedTranslationResult<Vec<FlexAttachedTranslationChangeRecord>>;
}

pub fn validate_flex_attached_translation_change_page(
    after_seq: u64,
    through_seq: u64,
    limit: u16,
) -> FlexAttachedTranslationResult<()> {
    if through_seq < after_seq {
        return Err(FlexAttachedTranslationError::Invalid(
            "attached Translation change highwater must not precede the cursor".to_string(),
        ));
    }
    if limit == 0 || limit > MAX_FLEX_ATTACHED_TRANSLATION_CHANGE_PAGE {
        return Err(FlexAttachedTranslationError::Invalid(format!(
            "attached Translation change page limit must be between 1 and {MAX_FLEX_ATTACHED_TRANSLATION_CHANGE_PAGE}"
        )));
    }
    Ok(())
}

pub fn flex_attached_translation_deleted_revision(
    entity_type: &str,
    entity_id: Uuid,
) -> Result<String, FlexError> {
    if !is_valid_flex_entity_type(entity_type) {
        return Err(FlexError::UnknownEntityType(entity_type.to_string()));
    }
    if entity_id.is_nil() {
        return Err(FlexError::Database(
            "attached Translation deleted resource id must not be nil".to_string(),
        ));
    }
    Ok(format!("deleted:{entity_type}:{entity_id}"))
}

/// Append the final donor-owned deletion tombstone in the same transaction that deletes
/// the canonical resource. Value-row triggers may append earlier `resource_changed` entries;
/// append-only ordering intentionally keeps the final `resource_deleted` event authoritative.
pub async fn record_flex_attached_translation_deleted_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    entity_type: &str,
    entity_id: Uuid,
) -> Result<String, FlexError> {
    if tenant_id.is_nil() {
        return Err(FlexError::Database(
            "attached Translation deleted tenant id must not be nil".to_string(),
        ));
    }
    let revision = flex_attached_translation_deleted_revision(entity_type, entity_id)?;

    if txn.get_database_backend() == DatabaseBackend::Postgres {
        txn.execute_raw(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            format!(
                "INSERT INTO {FLEX_ATTACHED_TRANSLATION_CHANGE_JOURNAL_TABLE} \
                 (tenant_id, entity_type, entity_id, change_kind) \
                 VALUES ($1, $2, $3, 'resource_deleted')"
            ),
            vec![
                tenant_id.into(),
                entity_type.to_string().into(),
                entity_id.into(),
            ],
        ))
        .await
        .map_err(database_error)?;
    }

    Ok(revision)
}

fn database_error(error: sea_orm::DbErr) -> FlexError {
    FlexError::Database(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn change_kind_round_trips() {
        for kind in [
            FlexAttachedTranslationChangeKind::ResourceChanged,
            FlexAttachedTranslationChangeKind::ResourceDeleted,
            FlexAttachedTranslationChangeKind::SchemaChanged,
        ] {
            assert_eq!(FlexAttachedTranslationChangeKind::parse(kind.as_str()).unwrap(), kind);
        }
    }

    #[test]
    fn record_shape_distinguishes_schema_and_resource_changes() {
        let resource = FlexAttachedTranslationChangeRecord {
            change_seq: 1,
            entity_type: "taxonomy.category".to_string(),
            entity_id: Some(Uuid::new_v4()),
            kind: FlexAttachedTranslationChangeKind::ResourceChanged,
        };
        assert!(resource.validate().is_ok());

        let schema = FlexAttachedTranslationChangeRecord {
            change_seq: 2,
            entity_type: "taxonomy.category".to_string(),
            entity_id: None,
            kind: FlexAttachedTranslationChangeKind::SchemaChanged,
        };
        assert!(schema.validate().is_ok());
    }

    #[test]
    fn deleted_revision_is_stable_and_scoped() {
        let entity_id = Uuid::new_v4();
        assert_eq!(
            flex_attached_translation_deleted_revision("taxonomy.category", entity_id).unwrap(),
            format!("deleted:taxonomy.category:{entity_id}")
        );
    }
}
