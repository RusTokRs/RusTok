use std::collections::BTreeMap;

use async_trait::async_trait;
use rustok_core::ModuleRuntimeExtensions;
use rustok_index::{
    DomainError, EntityKey, EntityName, FieldCardinality, FieldName, IndexField, IndexMutation,
    IndexRecord, IndexSchema, IndexSource, IndexSourceCursor, IndexSourceFailure,
    IndexSourceLoadBatch, IndexSourceLoadRequest, IndexSourcePage, IndexSourceScanRequest,
    IndexValue, IndexValueType, LocaleKey, LocaleMode, ModuleName, PostgresIndexSourceFactory,
    SchemaRef, SchemaVersion, derive_index_source_event_id, register_index_schema_source,
    register_index_source, register_postgres_index_source_factory,
};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, QueryResult, Statement, Value};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

pub(crate) const PRODUCT_INDEX_SOURCE: &str = "product-postgres-primary";
const PRODUCT_INDEX_FACTORY: &str = "product-postgres-primary";
const PRODUCT_EVENT_DOMAIN: &str = "rustok-product.product-replay-v1";

const PRODUCT_SELECT: &str = r#"
SELECT
    p.tenant_id,
    p.id AS product_id,
    p.index_revision,
    p.status::text AS status,
    p.vendor,
    p.product_type,
    p.primary_category_id,
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
enum ProductIndexBridgeError {
    #[error("Product Index contract is invalid")]
    InvalidContract(#[source] DomainError),
    #[error("Product Index cursor is invalid")]
    InvalidCursor,
    #[error("Product Index source row is invalid")]
    InvalidRow,
}

pub(crate) fn register(extensions: &mut ModuleRuntimeExtensions) -> rustok_core::Result<()> {
    let schema = product_schema().map_err(|error| {
        rustok_core::Error::Validation(format!(
            "selected Product Index schema construction failed: {error}"
        ))
    })?;
    register_index_schema_source(extensions, "product", schema).map_err(|error| {
        rustok_core::Error::Validation(format!(
            "selected Product Index schema registration failed: {error}"
        ))
    })?;
    register_postgres_index_source_factory(
        extensions,
        "product",
        PRODUCT_INDEX_FACTORY,
        ProductPostgresIndexSourceFactory,
    )
    .map_err(|error| {
        rustok_core::Error::Validation(format!(
            "selected Product Index source factory registration failed: {error}"
        ))
    })
}

fn product_schema() -> Result<IndexSchema, ProductIndexBridgeError> {
    let schema = IndexSchema {
        reference: product_schema_ref().map_err(ProductIndexBridgeError::InvalidContract)?,
        locale_mode: LocaleMode::Required,
        fields: vec![
            field("status", IndexValueType::String, false, true, true)?,
            field("title", IndexValueType::String, false, true, true)?,
            field("handle", IndexValueType::String, false, true, true)?,
            field("description", IndexValueType::String, true, false, false)?,
            field("vendor", IndexValueType::String, true, true, true)?,
            field("product_type", IndexValueType::String, true, true, true)?,
            field(
                "primary_category_id",
                IndexValueType::Uuid,
                true,
                true,
                false,
            )?,
        ],
        links: Vec::new(),
    };
    schema
        .validate()
        .map_err(ProductIndexBridgeError::InvalidContract)?;
    Ok(schema)
}

fn product_schema_ref() -> Result<SchemaRef, DomainError> {
    Ok(SchemaRef {
        module: ModuleName::new("rustok-product")?,
        entity: EntityName::new("product")?,
        version: SchemaVersion::INITIAL,
    })
}

fn field(
    name: &str,
    value_type: IndexValueType,
    nullable: bool,
    filterable: bool,
    sortable: bool,
) -> Result<IndexField, ProductIndexBridgeError> {
    Ok(IndexField {
        name: FieldName::new(name).map_err(ProductIndexBridgeError::InvalidContract)?,
        value_type,
        cardinality: FieldCardinality::One,
        nullable,
        selectable: true,
        filterable,
        sortable,
    })
}

#[derive(Clone, Copy, Debug)]
struct ProductPostgresIndexSourceFactory;

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
            ProductPostgresIndexSource { db },
        )
        .map_err(|error| error.to_string())
    }
}

#[derive(Clone, Debug)]
struct ProductPostgresIndexSource {
    db: DatabaseConnection,
}

impl ProductPostgresIndexSource {
    fn validate_request(&self, schema: &SchemaRef) -> Result<(), IndexSourceFailure> {
        if self.db.get_database_backend() != DbBackend::Postgres {
            return Err(permanent("product_index_backend_unsupported"));
        }
        match product_schema_ref() {
            Ok(expected) if &expected == schema => Ok(()),
            Ok(_) => Err(permanent("product_index_schema_mismatch")),
            Err(error) => {
                tracing::error!(error = %error, "selected Product Index identity is invalid");
                Err(permanent("product_index_contract_invalid"))
            }
        }
    }

    async fn scan_rows(
        &self,
        request: &IndexSourceScanRequest,
        cursor: Option<&ProductCursor>,
    ) -> Result<Vec<QueryResult>, IndexSourceFailure> {
        let fetch_limit = i64::try_from(request.limit() + 1)
            .expect("Index source page limit is bounded below i64::MAX");
        let (sql, values): (String, Vec<Value>) = match cursor {
            Some(cursor) => (
                format!(
                    "{PRODUCT_SELECT}\nWHERE p.tenant_id = $1\n  AND (p.id, t.locale) > ($2, $3)\nORDER BY p.id ASC, t.locale ASC\nLIMIT $4"
                ),
                vec![
                    request.tenant_id().into(),
                    cursor.product_id.into(),
                    cursor.locale.clone().into(),
                    fetch_limit.into(),
                ],
            ),
            None => (
                format!(
                    "{PRODUCT_SELECT}\nWHERE p.tenant_id = $1\nORDER BY p.id ASC, t.locale ASC\nLIMIT $2"
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
                    "selected Product Index scan failed"
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
            let locale = key
                .locale
                .as_ref()
                .ok_or_else(|| permanent("product_index_locale_required"))?;
            tuples.push(format!("(${parameter}::uuid, ${}::text)", parameter + 1));
            values.push(key.entity_id.into());
            values.push(locale.as_str().to_owned().into());
            parameter += 2;
        }
        let sql = format!(
            "WITH requested(product_id, locale) AS (VALUES {})\n{PRODUCT_SELECT}\nJOIN requested r ON r.product_id = p.id AND r.locale = t.locale\nWHERE p.tenant_id = $1\nORDER BY p.id ASC, t.locale ASC",
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
                    "selected Product Index targeted load failed"
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
        self.validate_request(request.schema())?;
        let cursor = request
            .cursor()
            .map(ProductCursor::decode)
            .transpose()
            .map_err(|error| {
                tracing::warn!(error = %error, "selected Product Index cursor was rejected");
                permanent("product_index_cursor_invalid")
            })?;
        let rows = self.scan_rows(&request, cursor.as_ref()).await?;
        let has_more = rows.len() > request.limit();
        let mut mutations = Vec::with_capacity(rows.len().min(request.limit()));
        let mut next_cursor = None;
        for row in rows.into_iter().take(request.limit()) {
            let decoded = ProductRow::decode(row, request.tenant_id()).map_err(|error| {
                tracing::error!(error = %error, "selected Product Index row was rejected");
                permanent("product_index_record_invalid")
            })?;
            if has_more {
                next_cursor = Some(decoded.cursor().encode().map_err(|error| {
                    tracing::error!(error = %error, "selected Product Index cursor encoding failed");
                    permanent("product_index_cursor_invalid")
                })?);
            }
            mutations.push(decoded.into_mutation().map_err(|error| {
                tracing::error!(error = %error, "selected Product Index mutation construction failed");
                permanent("product_index_record_invalid")
            })?);
        }
        IndexSourcePage::new(&request, mutations, next_cursor).map_err(|error| {
            tracing::error!(error = %error, "selected Product Index page validation failed");
            permanent("product_index_page_invalid")
        })
    }

    async fn load(
        &self,
        request: IndexSourceLoadRequest,
    ) -> Result<IndexSourceLoadBatch, IndexSourceFailure> {
        self.validate_request(request.schema())?;
        let mutations = self
            .load_rows(&request)
            .await?
            .into_iter()
            .map(|row| {
                ProductRow::decode(row, request.tenant_id())
                    .and_then(ProductRow::into_mutation)
                    .map_err(|error| {
                        tracing::error!(error = %error, "selected Product Index targeted row was rejected");
                        permanent("product_index_record_invalid")
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        IndexSourceLoadBatch::new(&request, mutations).map_err(|error| {
            tracing::error!(error = %error, "selected Product Index targeted batch validation failed");
            permanent("product_index_batch_invalid")
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProductCursor {
    product_id: Uuid,
    locale: String,
}

impl ProductCursor {
    fn decode(cursor: &IndexSourceCursor) -> Result<Self, ProductIndexBridgeError> {
        let decoded: Self = serde_json::from_value(cursor.value().clone())
            .map_err(|_| ProductIndexBridgeError::InvalidCursor)?;
        if decoded.product_id.is_nil() {
            return Err(ProductIndexBridgeError::InvalidCursor);
        }
        let canonical = LocaleKey::new(&decoded.locale)
            .map_err(ProductIndexBridgeError::InvalidContract)?;
        if canonical.as_str() != decoded.locale {
            return Err(ProductIndexBridgeError::InvalidCursor);
        }
        Ok(decoded)
    }

    fn encode(&self) -> Result<IndexSourceCursor, ProductIndexBridgeError> {
        let value = serde_json::to_value(self).map_err(|_| ProductIndexBridgeError::InvalidCursor)?;
        IndexSourceCursor::new(value).map_err(|_| ProductIndexBridgeError::InvalidCursor)
    }
}

#[derive(Debug)]
struct ProductRow {
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
}

impl ProductRow {
    fn decode(row: QueryResult, expected_tenant: Uuid) -> Result<Self, ProductIndexBridgeError> {
        let tenant_id = row
            .try_get::<Uuid>("", "tenant_id")
            .map_err(|_| ProductIndexBridgeError::InvalidRow)?;
        let product_id = row
            .try_get::<Uuid>("", "product_id")
            .map_err(|_| ProductIndexBridgeError::InvalidRow)?;
        let revision = row
            .try_get::<i64>("", "index_revision")
            .map_err(|_| ProductIndexBridgeError::InvalidRow)?;
        let locale = row
            .try_get::<String>("", "locale")
            .map_err(|_| ProductIndexBridgeError::InvalidRow)?;
        if tenant_id != expected_tenant || tenant_id.is_nil() || product_id.is_nil() || revision <= 0 {
            return Err(ProductIndexBridgeError::InvalidRow);
        }
        let locale = LocaleKey::new(&locale).map_err(ProductIndexBridgeError::InvalidContract)?;
        let primary_category_id = row
            .try_get::<Option<Uuid>>("", "primary_category_id")
            .map_err(|_| ProductIndexBridgeError::InvalidRow)?;
        if primary_category_id.is_some_and(|id| id.is_nil()) {
            return Err(ProductIndexBridgeError::InvalidRow);
        }
        Ok(Self {
            tenant_id,
            product_id,
            source_version: u64::try_from(revision)
                .map_err(|_| ProductIndexBridgeError::InvalidRow)?,
            locale,
            status: required_string(&row, "status")?,
            title: required_string(&row, "title")?,
            handle: required_string(&row, "handle")?,
            description: optional_string(&row, "description")?,
            vendor: optional_string(&row, "vendor")?,
            product_type: optional_string(&row, "product_type")?,
            primary_category_id,
        })
    }

    fn cursor(&self) -> ProductCursor {
        ProductCursor {
            product_id: self.product_id,
            locale: self.locale.as_str().to_owned(),
        }
    }

    fn into_mutation(mut self) -> Result<IndexMutation, ProductIndexBridgeError> {
        let event_id = derive_index_source_event_id(
            PRODUCT_EVENT_DOMAIN,
            self.tenant_id,
            self.product_id,
            Some(&self.locale),
            self.source_version,
        )
        .map_err(|_| ProductIndexBridgeError::InvalidRow)?;
        let fields = BTreeMap::from([
            (field_name("status")?, IndexValue::String(self.status)),
            (field_name("title")?, IndexValue::String(self.title)),
            (field_name("handle")?, IndexValue::String(self.handle)),
            (
                field_name("description")?,
                self.description
                    .take()
                    .map(IndexValue::String)
                    .unwrap_or(IndexValue::Null),
            ),
            (
                field_name("vendor")?,
                self.vendor
                    .take()
                    .map(IndexValue::String)
                    .unwrap_or(IndexValue::Null),
            ),
            (
                field_name("product_type")?,
                self.product_type
                    .take()
                    .map(IndexValue::String)
                    .unwrap_or(IndexValue::Null),
            ),
            (
                field_name("primary_category_id")?,
                self.primary_category_id
                    .map(IndexValue::Uuid)
                    .unwrap_or(IndexValue::Null),
            ),
        ]);
        Ok(IndexMutation::Upsert {
            event_id,
            record: IndexRecord {
                key: EntityKey {
                    tenant_id: self.tenant_id,
                    schema: product_schema_ref().map_err(ProductIndexBridgeError::InvalidContract)?,
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

fn field_name(name: &str) -> Result<FieldName, ProductIndexBridgeError> {
    FieldName::new(name).map_err(ProductIndexBridgeError::InvalidContract)
}

fn required_string(row: &QueryResult, column: &str) -> Result<String, ProductIndexBridgeError> {
    let value = row
        .try_get::<String>("", column)
        .map_err(|_| ProductIndexBridgeError::InvalidRow)?;
    if value.is_empty() {
        Err(ProductIndexBridgeError::InvalidRow)
    } else {
        Ok(value)
    }
}

fn optional_string(
    row: &QueryResult,
    column: &str,
) -> Result<Option<String>, ProductIndexBridgeError> {
    row.try_get::<Option<String>>("", column)
        .map_err(|_| ProductIndexBridgeError::InvalidRow)
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
    fn selected_product_schema_is_locale_required_and_scalar_only() {
        let schema = product_schema().unwrap();
        assert_eq!(schema.reference, product_schema_ref().unwrap());
        assert_eq!(schema.locale_mode, LocaleMode::Required);
        assert_eq!(schema.fields.len(), 7);
        assert!(schema.links.is_empty());
        assert!(schema.fingerprint().is_ok());
    }

    #[test]
    fn selected_product_cursor_rejects_unknown_nil_and_noncanonical_values() {
        for value in [
            serde_json::json!({"product_id": Uuid::nil(), "locale": "en-US"}),
            serde_json::json!({"product_id": Uuid::from_u128(2), "locale": "EN-us"}),
            serde_json::json!({"product_id": Uuid::from_u128(2), "locale": "en-US", "revision": 1}),
        ] {
            let cursor = IndexSourceCursor::new(value).unwrap();
            assert!(ProductCursor::decode(&cursor).is_err());
        }
    }

    #[test]
    fn selected_product_bridge_registers_schema_and_factory() {
        let mut extensions = ModuleRuntimeExtensions::default();
        extensions.insert(rustok_index::IndexSchemaSourceCatalog::new());
        extensions.insert(rustok_index::PostgresIndexSourceFactoryCatalog::new());
        register(&mut extensions).unwrap();
        let schema = product_schema().unwrap();
        assert_eq!(
            extensions
                .get::<rustok_index::IndexSchemaSourceCatalog>()
                .unwrap()
                .get(&schema.reference)
                .unwrap()
                .owner_module,
            "product"
        );
        assert_eq!(
            extensions
                .get::<rustok_index::PostgresIndexSourceFactoryCatalog>()
                .unwrap()
                .len(),
            1
        );
    }
}
