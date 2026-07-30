use std::collections::BTreeMap;

use async_trait::async_trait;
use chrono::{DateTime, FixedOffset, Utc};
use rustok_core::ModuleRuntimeExtensions;
use rustok_index::{
    DomainError, EntityKey, EntityName, FieldCardinality, FieldName, IndexField, IndexMutation,
    IndexRecord, IndexSchema, IndexSource, IndexSourceCursor, IndexSourceFailure,
    IndexSourceLoadBatch, IndexSourceLoadRequest, IndexSourcePage, IndexSourceScanRequest,
    IndexValue, IndexValueType, LocaleKey, LocaleMode, ModuleName, PostgresIndexSourceFactory,
    SchemaRef, SchemaVersion, register_index_source,
};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, QueryResult, Statement, Value};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

pub const PRODUCT_INDEX_MODULE: &str = "rustok-product";
pub const PRODUCT_INDEX_ENTITY: &str = "product";
pub const PRODUCT_INDEX_SOURCE: &str = "product-postgres-primary";
pub const PRODUCT_INDEX_SOURCE_FACTORY: &str = "product-postgres-primary";

const STATUS_FIELD: &str = "status";
const TITLE_FIELD: &str = "title";
const HANDLE_FIELD: &str = "handle";
const DESCRIPTION_FIELD: &str = "description";
const VENDOR_FIELD: &str = "vendor";
const PRODUCT_TYPE_FIELD: &str = "product_type";
const PRIMARY_CATEGORY_ID_FIELD: &str = "primary_category_id";
const PUBLISHED_AT_FIELD: &str = "published_at";
const UPDATED_AT_FIELD: &str = "updated_at";

const PRODUCT_INDEX_SELECT: &str = r#"
SELECT
    p.tenant_id,
    p.id AS product_id,
    p.index_revision,
    p.status::text AS status,
    p.vendor,
    p.product_type,
    p.primary_category_id,
    p.published_at,
    p.updated_at,
    t.locale,
    t.title,
    t.handle,
    t.description
FROM products p
JOIN product_translations t
  ON t.product_id = p.id
 AND t.tenant_id = p.tenant_id
"#;

#[derive(Debug, Error)]
pub enum ProductIndexError {
    #[error("invalid Product Index contract: {0}")]
    InvalidContract(#[from] DomainError),
    #[error("Product Index cursor is invalid")]
    InvalidCursor,
    #[error("Product Index source row is invalid")]
    InvalidRow,
}

/// Owner-published locale-scoped Product schema for structured Index queries.
///
/// This first M7 slice intentionally contains only Product-owned scalar fields. Variant, pricing,
/// inventory, sales-channel, taxonomy-link, and relevance semantics remain separate owner slices.
pub fn product_index_schema() -> Result<IndexSchema, ProductIndexError> {
    let schema = IndexSchema {
        reference: product_schema_ref()?,
        locale_mode: LocaleMode::Required,
        fields: vec![
            field(STATUS_FIELD, IndexValueType::String, false, true, true)?,
            field(TITLE_FIELD, IndexValueType::String, false, true, true)?,
            field(HANDLE_FIELD, IndexValueType::String, false, true, true)?,
            field(
                DESCRIPTION_FIELD,
                IndexValueType::String,
                true,
                false,
                false,
            )?,
            field(VENDOR_FIELD, IndexValueType::String, true, true, true)?,
            field(
                PRODUCT_TYPE_FIELD,
                IndexValueType::String,
                true,
                true,
                true,
            )?,
            field(
                PRIMARY_CATEGORY_ID_FIELD,
                IndexValueType::Uuid,
                true,
                true,
                false,
            )?,
            field(
                PUBLISHED_AT_FIELD,
                IndexValueType::Timestamp,
                true,
                true,
                true,
            )?,
            field(
                UPDATED_AT_FIELD,
                IndexValueType::Timestamp,
                false,
                true,
                true,
            )?,
        ],
        links: Vec::new(),
    };
    schema.validate()?;
    Ok(schema)
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ProductPostgresIndexSourceFactory;

impl PostgresIndexSourceFactory for ProductPostgresIndexSourceFactory {
    fn register_source(
        &self,
        extensions: &mut ModuleRuntimeExtensions,
        db: DatabaseConnection,
    ) -> Result<(), String> {
        register_index_source(
            extensions,
            "product",
            PRODUCT_INDEX_SOURCE,
            [product_schema_ref().map_err(|error| error.to_string())?],
            ProductPostgresIndexSource::new(db),
        )
        .map_err(|error| error.to_string())
    }
}

#[derive(Clone, Debug)]
pub struct ProductPostgresIndexSource {
    db: DatabaseConnection,
}

impl ProductPostgresIndexSource {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    fn ensure_postgres(&self) -> Result<(), IndexSourceFailure> {
        if self.db.get_database_backend() == DbBackend::Postgres {
            Ok(())
        } else {
            Err(permanent("product_index_backend_unsupported"))
        }
    }

    fn validate_schema(&self, schema: &SchemaRef) -> Result<(), IndexSourceFailure> {
        let expected = product_schema_ref().map_err(|error| {
            tracing::error!(error = %error, "Product Index schema identity construction failed");
            permanent("product_index_contract_invalid")
        })?;
        if schema == &expected {
            Ok(())
        } else {
            Err(permanent("product_index_schema_mismatch"))
        }
    }

    async fn scan_rows(
        &self,
        request: &IndexSourceScanRequest,
        cursor: Option<&ProductSourceCursor>,
    ) -> Result<Vec<QueryResult>, IndexSourceFailure> {
        let fetch_limit = i64::try_from(request.limit() + 1)
            .expect("Index source scan limit is bounded below i64::MAX");
        let (sql, values): (String, Vec<Value>) = match cursor {
            Some(cursor) => (
                format!(
                    "{PRODUCT_INDEX_SELECT}\nWHERE p.tenant_id = $1\n  AND (p.index_revision, p.id, t.locale) > ($2, $3, $4)\nORDER BY p.index_revision ASC, p.id ASC, t.locale ASC\nLIMIT $5"
                ),
                vec![
                    request.tenant_id().into(),
                    cursor.database_revision().into(),
                    cursor.product_id.into(),
                    cursor.locale.clone().into(),
                    fetch_limit.into(),
                ],
            ),
            None => (
                format!(
                    "{PRODUCT_INDEX_SELECT}\nWHERE p.tenant_id = $1\nORDER BY p.index_revision ASC, p.id ASC, t.locale ASC\nLIMIT $2"
                ),
                vec![request.tenant_id().into(), fetch_limit.into()],
            ),
        };

        self.db
            .query_all(Statement::from_sql_and_values(DbBackend::Postgres, sql, values))
            .await
            .map_err(|error| {
                tracing::error!(
                    error = ?error,
                    tenant_id = %request.tenant_id(),
                    "Product Index source scan failed"
                );
                retryable("product_index_storage_unavailable")
            })
    }

    async fn load_rows(
        &self,
        request: &IndexSourceLoadRequest,
    ) -> Result<Vec<QueryResult>, IndexSourceFailure> {
        let mut values = Vec::<Value>::with_capacity(1 + request.keys().len() * 2);
        values.push(request.tenant_id().into());
        let mut tuples = Vec::with_capacity(request.keys().len());
        let mut parameter = 2usize;
        for key in request.keys() {
            let Some(locale) = key.locale.as_ref() else {
                return Err(permanent("product_index_locale_required"));
            };
            tuples.push(format!("(${parameter}::uuid, ${}::text)", parameter + 1));
            values.push(key.entity_id.into());
            values.push(locale.as_str().to_owned().into());
            parameter += 2;
        }

        let sql = format!(
            "WITH requested(product_id, locale) AS (VALUES {})\n{PRODUCT_INDEX_SELECT}\nJOIN requested r ON r.product_id = p.id AND r.locale = t.locale\nWHERE p.tenant_id = $1\nORDER BY p.id ASC, t.locale ASC",
            tuples.join(", ")
        );
        self.db
            .query_all(Statement::from_sql_and_values(DbBackend::Postgres, sql, values))
            .await
            .map_err(|error| {
                tracing::error!(
                    error = ?error,
                    tenant_id = %request.tenant_id(),
                    key_count = request.keys().len(),
                    "Product Index targeted source load failed"
                );
                retryable("product_index_storage_unavailable")
            })
    }
}

#[async_trait]
impl IndexSource for ProductPostgresIndexSource {
    async fn scan(
        &self,
        request: IndexSourceScanRequest,
    ) -> Result<IndexSourcePage, IndexSourceFailure> {
        self.ensure_postgres()?;
        self.validate_schema(request.schema())?;
        let cursor = request
            .cursor()
            .map(ProductSourceCursor::decode)
            .transpose()
            .map_err(|error| {
                tracing::warn!(error = %error, "Product Index scan cursor was rejected");
                permanent("product_index_cursor_invalid")
            })?;
        let rows = self.scan_rows(&request, cursor.as_ref()).await?;
        let has_more = rows.len() > request.limit();
        let mut mutations = Vec::with_capacity(rows.len().min(request.limit()));
        let mut next_cursor = None;
        for row in rows.into_iter().take(request.limit()) {
            let decoded = ProductSourceRow::decode(row).map_err(|error| {
                tracing::error!(error = %error, "Product Index source row was rejected");
                permanent("product_index_record_invalid")
            })?;
            if has_more {
                next_cursor = Some(decoded.cursor().encode().map_err(|error| {
                    tracing::error!(error = %error, "Product Index cursor encoding failed");
                    permanent("product_index_cursor_invalid")
                })?);
            }
            mutations.push(decoded.into_mutation().map_err(|error| {
                tracing::error!(error = %error, "Product Index mutation construction failed");
                permanent("product_index_record_invalid")
            })?);
        }

        IndexSourcePage::new(&request, mutations, next_cursor).map_err(|error| {
            tracing::error!(error = %error, "Product Index source page validation failed");
            permanent("product_index_page_invalid")
        })
    }

    async fn load(
        &self,
        request: IndexSourceLoadRequest,
    ) -> Result<IndexSourceLoadBatch, IndexSourceFailure> {
        self.ensure_postgres()?;
        self.validate_schema(request.schema())?;
        let rows = self.load_rows(&request).await?;
        let mutations = rows
            .into_iter()
            .map(|row| {
                ProductSourceRow::decode(row)
                    .and_then(ProductSourceRow::into_mutation)
                    .map_err(|error| {
                        tracing::error!(error = %error, "Product Index targeted row was rejected");
                        permanent("product_index_record_invalid")
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        IndexSourceLoadBatch::new(&request, mutations).map_err(|error| {
            tracing::error!(error = %error, "Product Index targeted batch validation failed");
            permanent("product_index_batch_invalid")
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ProductSourceCursor {
    source_version: u64,
    product_id: Uuid,
    locale: String,
}

impl ProductSourceCursor {
    fn decode(cursor: &IndexSourceCursor) -> Result<Self, ProductIndexError> {
        let decoded: Self = serde_json::from_value(cursor.value().clone())
            .map_err(|_| ProductIndexError::InvalidCursor)?;
        if decoded.source_version == 0
            || decoded.source_version > i64::MAX as u64
            || decoded.product_id.is_nil()
        {
            return Err(ProductIndexError::InvalidCursor);
        }
        let canonical = LocaleKey::new(&decoded.locale)?;
        if canonical.as_str() != decoded.locale {
            return Err(ProductIndexError::InvalidCursor);
        }
        Ok(decoded)
    }

    fn database_revision(&self) -> i64 {
        i64::try_from(self.source_version).expect("validated Product cursor revision")
    }

    fn encode(&self) -> Result<IndexSourceCursor, ProductIndexError> {
        IndexSourceCursor::new(
            serde_json::to_value(self).map_err(|_| ProductIndexError::InvalidCursor)?,
        )
        .map_err(|_| ProductIndexError::InvalidCursor)
    }
}

#[derive(Debug)]
struct ProductSourceRow {
    tenant_id: Uuid,
    product_id: Uuid,
    source_version: u64,
    locale: LocaleKey,
    status: String,
    title: String,
    handle: String,
    description: Option<String>,
    vendor: Option<String>,
    product_type: Option<String>,
    primary_category_id: Option<Uuid>,
    published_at: Option<DateTime<Utc>>,
    updated_at: DateTime<Utc>,
}

impl ProductSourceRow {
    fn decode(row: QueryResult) -> Result<Self, ProductIndexError> {
        let tenant_id = row
            .try_get::<Uuid>("", "tenant_id")
            .map_err(|_| ProductIndexError::InvalidRow)?;
        let product_id = row
            .try_get::<Uuid>("", "product_id")
            .map_err(|_| ProductIndexError::InvalidRow)?;
        let revision = row
            .try_get::<i64>("", "index_revision")
            .map_err(|_| ProductIndexError::InvalidRow)?;
        let locale = row
            .try_get::<String>("", "locale")
            .map_err(|_| ProductIndexError::InvalidRow)?;
        if tenant_id.is_nil() || product_id.is_nil() || revision <= 0 {
            return Err(ProductIndexError::InvalidRow);
        }
        let locale_key = LocaleKey::new(&locale)?;
        if locale_key.as_str() != locale {
            return Err(ProductIndexError::InvalidRow);
        }
        let primary_category_id = row
            .try_get::<Option<Uuid>>("", "primary_category_id")
            .map_err(|_| ProductIndexError::InvalidRow)?;
        if primary_category_id.is_some_and(|id| id.is_nil()) {
            return Err(ProductIndexError::InvalidRow);
        }
        let published_at = row
            .try_get::<Option<DateTime<FixedOffset>>>("", "published_at")
            .map_err(|_| ProductIndexError::InvalidRow)?
            .map(|value| value.with_timezone(&Utc));
        let updated_at = row
            .try_get::<DateTime<FixedOffset>>("", "updated_at")
            .map_err(|_| ProductIndexError::InvalidRow)?
            .with_timezone(&Utc);

        Ok(Self {
            tenant_id,
            product_id,
            source_version: u64::try_from(revision).map_err(|_| ProductIndexError::InvalidRow)?,
            locale: locale_key,
            status: required_string(&row, "status")?,
            title: required_string(&row, "title")?,
            handle: required_string(&row, "handle")?,
            description: optional_string(&row, "description")?,
            vendor: optional_string(&row, "vendor")?,
            product_type: optional_string(&row, "product_type")?,
            primary_category_id,
            published_at,
            updated_at,
        })
    }

    fn cursor(&self) -> ProductSourceCursor {
        ProductSourceCursor {
            source_version: self.source_version,
            product_id: self.product_id,
            locale: self.locale.as_str().to_owned(),
        }
    }

    fn into_mutation(mut self) -> Result<IndexMutation, ProductIndexError> {
        let event_id = product_replay_event_id(
            self.tenant_id,
            self.product_id,
            &self.locale,
            self.source_version,
        );
        let fields = BTreeMap::from([
            (field_name(STATUS_FIELD)?, IndexValue::String(self.status)),
            (field_name(TITLE_FIELD)?, IndexValue::String(self.title)),
            (field_name(HANDLE_FIELD)?, IndexValue::String(self.handle)),
            (
                field_name(DESCRIPTION_FIELD)?,
                self.description
                    .take()
                    .map(IndexValue::String)
                    .unwrap_or(IndexValue::Null),
            ),
            (
                field_name(VENDOR_FIELD)?,
                self.vendor
                    .take()
                    .map(IndexValue::String)
                    .unwrap_or(IndexValue::Null),
            ),
            (
                field_name(PRODUCT_TYPE_FIELD)?,
                self.product_type
                    .take()
                    .map(IndexValue::String)
                    .unwrap_or(IndexValue::Null),
            ),
            (
                field_name(PRIMARY_CATEGORY_ID_FIELD)?,
                self.primary_category_id
                    .map(IndexValue::Uuid)
                    .unwrap_or(IndexValue::Null),
            ),
            (
                field_name(PUBLISHED_AT_FIELD)?,
                self.published_at
                    .map(IndexValue::Timestamp)
                    .unwrap_or(IndexValue::Null),
            ),
            (
                field_name(UPDATED_AT_FIELD)?,
                IndexValue::Timestamp(self.updated_at),
            ),
        ]);
        Ok(IndexMutation::Upsert {
            event_id,
            record: IndexRecord {
                key: EntityKey {
                    tenant_id: self.tenant_id,
                    schema: product_schema_ref()?,
                    entity_id: self.product_id,
                    locale: Some(self.locale),
                },
                source_version: self.source_version,
                fields,
                links: Vec::new(),
            },
        })
    }
}

fn required_string(row: &QueryResult, column: &str) -> Result<String, ProductIndexError> {
    let value = row
        .try_get::<String>("", column)
        .map_err(|_| ProductIndexError::InvalidRow)?;
    if value.is_empty() {
        Err(ProductIndexError::InvalidRow)
    } else {
        Ok(value)
    }
}

fn optional_string(
    row: &QueryResult,
    column: &str,
) -> Result<Option<String>, ProductIndexError> {
    row.try_get::<Option<String>>("", column)
        .map_err(|_| ProductIndexError::InvalidRow)
}

fn product_schema_ref() -> Result<SchemaRef, DomainError> {
    Ok(SchemaRef {
        module: ModuleName::new(PRODUCT_INDEX_MODULE)?,
        entity: EntityName::new(PRODUCT_INDEX_ENTITY)?,
        version: SchemaVersion::INITIAL,
    })
}

fn field(
    name: &str,
    value_type: IndexValueType,
    nullable: bool,
    filterable: bool,
    sortable: bool,
) -> Result<IndexField, DomainError> {
    Ok(IndexField {
        name: field_name(name)?,
        value_type,
        cardinality: FieldCardinality::One,
        nullable,
        selectable: true,
        filterable,
        sortable,
    })
}

fn field_name(name: &str) -> Result<FieldName, DomainError> {
    FieldName::new(name)
}

fn product_replay_event_id(
    tenant_id: Uuid,
    product_id: Uuid,
    locale: &LocaleKey,
    source_version: u64,
) -> Uuid {
    let mut hasher = Sha256::new();
    hasher.update(b"rustok-product-index-replay-event-v1");
    hasher.update(tenant_id.as_bytes());
    hasher.update(product_id.as_bytes());
    hasher.update((locale.as_str().len() as u64).to_be_bytes());
    hasher.update(locale.as_str().as_bytes());
    hasher.update(source_version.to_be_bytes());
    let digest = hasher.finalize();
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x50;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes)
}

fn retryable(code: &'static str) -> IndexSourceFailure {
    IndexSourceFailure::retryable(code).expect("static Product Index retry code must be valid")
}

fn permanent(code: &'static str) -> IndexSourceFailure {
    IndexSourceFailure::permanent(code).expect("static Product Index failure code must be valid")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn product_schema_is_locale_required_and_scalar_only() {
        let schema = product_index_schema().unwrap();
        assert_eq!(schema.reference, product_schema_ref().unwrap());
        assert_eq!(schema.locale_mode, LocaleMode::Required);
        assert_eq!(schema.fields.len(), 9);
        assert!(schema.links.is_empty());
        assert!(schema.fingerprint().is_ok());
    }

    #[test]
    fn replay_event_identity_is_stable_and_revision_sensitive() {
        let tenant_id = Uuid::from_u128(1);
        let product_id = Uuid::from_u128(2);
        let locale = LocaleKey::new("en-US").unwrap();
        let first = product_replay_event_id(tenant_id, product_id, &locale, 7);
        let retry = product_replay_event_id(tenant_id, product_id, &locale, 7);
        let later = product_replay_event_id(tenant_id, product_id, &locale, 8);
        assert_eq!(first, retry);
        assert_ne!(first, later);
        assert!(!first.is_nil());
    }

    #[test]
    fn cursor_rejects_zero_revision_nil_product_and_noncanonical_locale() {
        for cursor in [
            serde_json::json!({"source_version": 0, "product_id": Uuid::from_u128(2), "locale": "en-US"}),
            serde_json::json!({"source_version": 1, "product_id": Uuid::nil(), "locale": "en-US"}),
            serde_json::json!({"source_version": 1, "product_id": Uuid::from_u128(2), "locale": "EN-us"}),
        ] {
            let cursor = IndexSourceCursor::new(cursor).unwrap();
            assert!(ProductSourceCursor::decode(&cursor).is_err());
        }
    }
}
