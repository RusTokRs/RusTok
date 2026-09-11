use async_trait::async_trait;
use sea_orm::{
    ConnectionTrait, DatabaseBackend, DatabaseConnection, FromQueryResult, Statement,
};
use uuid::Uuid;

use crate::{FlexStandaloneTranslationError, FlexStandaloneTranslationResult};

pub const FLEX_STANDALONE_TRANSLATION_RESOURCE_STATE_TABLE: &str =
    "flex_standalone_translation_resource_state";
pub const FLEX_STANDALONE_TRANSLATION_CHANGE_JOURNAL_TABLE: &str =
    "flex_standalone_translation_change_journal";
pub const MAX_FLEX_STANDALONE_TRANSLATION_CHANGE_PAGE: u16 = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlexStandaloneTranslationChangeLifecycle {
    Active,
    Archived,
    Deleted,
}

impl FlexStandaloneTranslationChangeLifecycle {
    pub fn parse(value: &str) -> FlexStandaloneTranslationResult<Self> {
        match value {
            "active" => Ok(Self::Active),
            "archived" => Ok(Self::Archived),
            "deleted" => Ok(Self::Deleted),
            other => Err(FlexStandaloneTranslationError::OwnerInvariant(format!(
                "standalone Translation change journal contains invalid lifecycle `{other}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlexStandaloneTranslationChangeRecord {
    pub change_seq: u64,
    pub schema_id: Uuid,
    pub entry_id: Uuid,
    pub resource_revision: String,
    pub lifecycle: FlexStandaloneTranslationChangeLifecycle,
}

impl FlexStandaloneTranslationChangeRecord {
    pub fn validate(&self) -> FlexStandaloneTranslationResult<()> {
        if self.change_seq == 0 || self.schema_id.is_nil() || self.entry_id.is_nil() {
            return Err(FlexStandaloneTranslationError::OwnerInvariant(
                "standalone Translation change identity is invalid".to_string(),
            ));
        }
        if self.resource_revision.trim().is_empty() {
            return Err(FlexStandaloneTranslationError::OwnerInvariant(
                "standalone Translation change must contain a resource revision".to_string(),
            ));
        }
        Ok(())
    }
}

#[async_trait]
pub trait FlexStandaloneTranslationChangeOwnerPort: Send + Sync {
    async fn read_change_highwater(
        &self,
        tenant_id: Uuid,
    ) -> FlexStandaloneTranslationResult<Option<u64>>;

    async fn read_changes(
        &self,
        tenant_id: Uuid,
        after_seq: u64,
        through_seq: u64,
        limit: u16,
    ) -> FlexStandaloneTranslationResult<Vec<FlexStandaloneTranslationChangeRecord>>;
}

#[derive(Clone)]
pub struct FlexStandaloneTranslationChangeReader {
    db: DatabaseConnection,
}

impl FlexStandaloneTranslationChangeReader {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[derive(Debug, FromQueryResult)]
struct ChangeHighwaterRow {
    highwater: Option<i64>,
}

#[derive(Debug, FromQueryResult)]
struct ChangeRow {
    change_seq: i64,
    schema_id: Uuid,
    entry_id: Uuid,
    resource_revision: String,
    lifecycle: String,
}

#[async_trait]
impl FlexStandaloneTranslationChangeOwnerPort for FlexStandaloneTranslationChangeReader {
    async fn read_change_highwater(
        &self,
        tenant_id: Uuid,
    ) -> FlexStandaloneTranslationResult<Option<u64>> {
        validate_tenant_id(tenant_id)?;
        ensure_postgres(&self.db)?;
        let row = ChangeHighwaterRow::find_by_statement(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            format!(
                "SELECT MAX(change_seq) AS highwater FROM {FLEX_STANDALONE_TRANSLATION_CHANGE_JOURNAL_TABLE} WHERE tenant_id = $1"
            ),
            vec![tenant_id.into()],
        ))
        .one(&self.db)
        .await
        .map_err(database_error)?
        .ok_or_else(|| {
            FlexStandaloneTranslationError::OwnerInvariant(
                "standalone Translation change high-water query returned no row".to_string(),
            )
        })?;
        row.highwater
            .map(|value| positive_sequence(value, "high-water"))
            .transpose()
    }

    async fn read_changes(
        &self,
        tenant_id: Uuid,
        after_seq: u64,
        through_seq: u64,
        limit: u16,
    ) -> FlexStandaloneTranslationResult<Vec<FlexStandaloneTranslationChangeRecord>> {
        validate_tenant_id(tenant_id)?;
        validate_page(after_seq, through_seq, limit)?;
        if through_seq == 0 {
            return Ok(Vec::new());
        }
        ensure_postgres(&self.db)?;
        let rows = ChangeRow::find_by_statement(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            format!(
                r#"
SELECT change_seq, schema_id, entry_id, resource_revision, lifecycle
FROM {FLEX_STANDALONE_TRANSLATION_CHANGE_JOURNAL_TABLE}
WHERE tenant_id = $1
  AND change_seq > $2
  AND change_seq <= $3
ORDER BY change_seq ASC
LIMIT $4
"#
            ),
            vec![
                tenant_id.into(),
                i64::try_from(after_seq).map_err(|_| invalid_sequence("after"))?.into(),
                i64::try_from(through_seq).map_err(|_| invalid_sequence("through"))?.into(),
                i64::from(limit).into(),
            ],
        ))
        .all(&self.db)
        .await
        .map_err(database_error)?;

        rows.into_iter()
            .map(|row| {
                let record = FlexStandaloneTranslationChangeRecord {
                    change_seq: positive_sequence(row.change_seq, "change")?,
                    schema_id: row.schema_id,
                    entry_id: row.entry_id,
                    resource_revision: row.resource_revision,
                    lifecycle: FlexStandaloneTranslationChangeLifecycle::parse(&row.lifecycle)?,
                };
                record.validate()?;
                Ok(record)
            })
            .collect()
    }
}

pub fn validate_page(
    after_seq: u64,
    through_seq: u64,
    limit: u16,
) -> FlexStandaloneTranslationResult<()> {
    if through_seq < after_seq {
        return Err(FlexStandaloneTranslationError::Invalid(
            "standalone Translation change highwater must not precede the cursor".to_string(),
        ));
    }
    if limit == 0 || limit > MAX_FLEX_STANDALONE_TRANSLATION_CHANGE_PAGE {
        return Err(FlexStandaloneTranslationError::Invalid(format!(
            "standalone Translation change page limit must be between 1 and {MAX_FLEX_STANDALONE_TRANSLATION_CHANGE_PAGE}"
        )));
    }
    Ok(())
}

fn validate_tenant_id(tenant_id: Uuid) -> FlexStandaloneTranslationResult<()> {
    if tenant_id.is_nil() {
        return Err(FlexStandaloneTranslationError::Invalid(
            "standalone Translation tenant_id must not be nil".to_string(),
        ));
    }
    Ok(())
}

fn ensure_postgres(db: &DatabaseConnection) -> FlexStandaloneTranslationResult<()> {
    if db.get_database_backend() != DatabaseBackend::Postgres {
        return Err(FlexStandaloneTranslationError::Invalid(
            "standalone Translation ChangeCursor requires PostgreSQL".to_string(),
        ));
    }
    Ok(())
}

fn positive_sequence(value: i64, field: &str) -> FlexStandaloneTranslationResult<u64> {
    let value = u64::try_from(value).map_err(|_| invalid_sequence(field))?;
    if value == 0 {
        return Err(invalid_sequence(field));
    }
    Ok(value)
}

fn invalid_sequence(field: &str) -> FlexStandaloneTranslationError {
    FlexStandaloneTranslationError::OwnerInvariant(format!(
        "standalone Translation change {field} sequence must be positive"
    ))
}

fn database_error(error: sea_orm::DbErr) -> FlexStandaloneTranslationError {
    FlexStandaloneTranslationError::Storage(error.to_string())
}
