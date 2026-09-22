use std::collections::BTreeMap;

use async_trait::async_trait;
use rustok_core::ModuleRuntimeExtensions;
use rustok_index::{
    DomainError, EntityKey, EntityName, FieldCardinality, FieldName, IndexField, IndexMutation,
    IndexRecord, IndexSchema, IndexSource, IndexSourceCursor, IndexSourceFailure,
    IndexSourceLoadBatch, IndexSourceLoadRequest, IndexSourcePage, IndexSourceScanRequest,
    IndexValue, IndexValueType, LocaleMode, ModuleName, PostgresIndexSourceFactory, SchemaRef,
    SchemaVersion, derive_index_source_event_id, register_index_schema_source,
    register_index_source, register_postgres_index_source_factory,
};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, QueryResult, Statement, Value};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

pub(crate) const SALES_CHANNEL_INDEX_SOURCE: &str = "sales-channel-postgres-primary";
const SALES_CHANNEL_INDEX_FACTORY: &str = "sales-channel-postgres-primary";
const SALES_CHANNEL_EVENT_DOMAIN: &str = "rustok-channel.sales-channel-replay-v1";

const SALES_CHANNEL_ROWS_CTE: &str = r#"
sales_channel_index_union AS (
    SELECT
        FALSE AS is_deleted,
        c.tenant_id,
        c.id AS channel_id,
        c.index_revision,
        c.slug,
        c.name,
        c.is_active,
        c.is_default,
        c.status
    FROM channels c
    WHERE c.tenant_id = $1

    UNION ALL

    SELECT
        TRUE AS is_deleted,
        tombstone.tenant_id,
        tombstone.channel_id,
        tombstone.source_version AS index_revision,
        NULL::text AS slug,
        NULL::text AS name,
        NULL::boolean AS is_active,
        NULL::boolean AS is_default,
        NULL::text AS status
    FROM channel_index_tombstones tombstone
    WHERE tombstone.tenant_id = $1
),
sales_channel_index_rows AS (
    SELECT
        row.*,
        COUNT(*) OVER (
            PARTITION BY row.tenant_id, row.channel_id
        ) AS identity_count
    FROM sales_channel_index_union row
)
"#;

const SALES_CHANNEL_ROW_SELECT: &str = r#"
SELECT
    row.is_deleted,
    row.identity_count,
    row.tenant_id,
    row.channel_id,
    row.index_revision,
    row.slug,
    row.name,
    row.is_active,
    row.is_default,
    row.status
FROM sales_channel_index_rows row
"#;

#[derive(Debug, Error)]
enum SalesChannelIndexBridgeError {
    #[error("SalesChannel Index contract is invalid")]
    InvalidContract(#[source] DomainError),
    #[error("SalesChannel Index cursor is invalid")]
    InvalidCursor,
    #[error("SalesChannel Index source row is invalid")]
    InvalidRow,
}

pub(crate) fn register(extensions: &mut ModuleRuntimeExtensions) -> rustok_core::Result<()> {
    if !extensions.contains::<rustok_channel::ChannelRuntimeSelected>() {
        return Ok(());
    }

    let schema = sales_channel_schema().map_err(|error| {
        rustok_core::Error::Validation(format!(
            "selected SalesChannel Index schema construction failed: {error}"
        ))
    })?;
    register_index_schema_source(extensions, "channel", schema).map_err(|error| {
        rustok_core::Error::Validation(format!(
            "selected SalesChannel Index schema registration failed: {error}"
        ))
    })?;
    register_postgres_index_source_factory(
        extensions,
        "channel",
        SALES_CHANNEL_INDEX_FACTORY,
        SalesChannelPostgresIndexSourceFactory,
    )
    .map_err(|error| {
        rustok_core::Error::Validation(format!(
            "selected SalesChannel Index source factory registration failed: {error}"
        ))
    })
}

fn sales_channel_schema() -> Result<IndexSchema, SalesChannelIndexBridgeError> {
    let schema = IndexSchema {
        reference: sales_channel_schema_ref()
            .map_err(SalesChannelIndexBridgeError::InvalidContract)?,
        locale_mode: LocaleMode::None,
        fields: vec![
            field("id", IndexValueType::Uuid, true, true)?,
            field("slug", IndexValueType::String, true, true)?,
            field("name", IndexValueType::String, true, true)?,
            field("is_active", IndexValueType::Boolean, true, true)?,
            field("is_default", IndexValueType::Boolean, true, true)?,
            field("status", IndexValueType::String, true, true)?,
        ],
        links: Vec::new(),
    };
    schema
        .validate()
        .map_err(SalesChannelIndexBridgeError::InvalidContract)?;
    Ok(schema)
}

fn sales_channel_schema_ref() -> Result<SchemaRef, DomainError> {
    Ok(SchemaRef {
        module: ModuleName::new("rustok-channel")?,
        entity: EntityName::new("sales_channel")?,
        version: SchemaVersion::INITIAL,
    })
}

fn field(
    name: &str,
    value_type: IndexValueType,
    filterable: bool,
    sortable: bool,
) -> Result<IndexField, SalesChannelIndexBridgeError> {
    Ok(IndexField {
        name: FieldName::new(name).map_err(SalesChannelIndexBridgeError::InvalidContract)?,
        value_type,
        cardinality: FieldCardinality::One,
        nullable: false,
        selectable: true,
        filterable,
        sortable,
    })
}

#[derive(Clone, Copy, Debug)]
struct SalesChannelPostgresIndexSourceFactory;

impl PostgresIndexSourceFactory for SalesChannelPostgresIndexSourceFactory {
    fn register_source(
        &self,
        extensions: &mut ModuleRuntimeExtensions,
        db: DatabaseConnection,
    ) -> Result<(), String> {
        register_index_source(
            extensions,
            "channel",
            SALES_CHANNEL_INDEX_SOURCE,
            [sales_channel_schema_ref().map_err(|error| error.to_string())?],
            SalesChannelPostgresIndexSource { db },
        )
        .map_err(|error| error.to_string())
    }
}

#[derive(Clone, Debug)]
struct SalesChannelPostgresIndexSource {
    db: DatabaseConnection,
}

impl SalesChannelPostgresIndexSource {
    fn validate_request(&self, schema: &SchemaRef) -> Result<(), IndexSourceFailure> {
        if self.db.get_database_backend() != DbBackend::Postgres {
            return Err(permanent("sales_channel_index_backend_unsupported"));
        }
        match sales_channel_schema_ref() {
            Ok(expected) if &expected == schema => Ok(()),
            Ok(_) => Err(permanent("sales_channel_index_schema_mismatch")),
            Err(_) => Err(permanent("sales_channel_index_contract_invalid")),
        }
    }

    async fn scan_rows(
        &self,
        request: &IndexSourceScanRequest,
        cursor: Option<&SalesChannelCursor>,
    ) -> Result<Vec<QueryResult>, IndexSourceFailure> {
        let fetch_limit = i64::try_from(request.limit() + 1)
            .expect("Index source page limit is bounded below i64::MAX");
        let (sql, values): (String, Vec<Value>) = match cursor {
            Some(cursor) => (
                format!(
                    "WITH {SALES_CHANNEL_ROWS_CTE}\n{SALES_CHANNEL_ROW_SELECT}\nWHERE row.channel_id > $2\nORDER BY row.channel_id ASC\nLIMIT $3"
                ),
                vec![
                    request.tenant_id().into(),
                    cursor.channel_id.into(),
                    fetch_limit.into(),
                ],
            ),
            None => (
                format!(
                    "WITH {SALES_CHANNEL_ROWS_CTE}\n{SALES_CHANNEL_ROW_SELECT}\nORDER BY row.channel_id ASC\nLIMIT $2"
                ),
                vec![request.tenant_id().into(), fetch_limit.into()],
            ),
        };
        self.db
            .query_all_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                sql,
                values,
            ))
            .await
            .map_err(|_| retryable("sales_channel_index_storage_unavailable"))
    }

    async fn load_rows(
        &self,
        request: &IndexSourceLoadRequest,
    ) -> Result<Vec<QueryResult>, IndexSourceFailure> {
        let mut values = Vec::<Value>::with_capacity(1 + request.keys().len());
        values.push(request.tenant_id().into());
        let mut rows = Vec::with_capacity(request.keys().len());
        for (offset, key) in request.keys().iter().enumerate() {
            if key.locale.is_some() {
                return Err(permanent("sales_channel_index_locale_forbidden"));
            }
            let parameter = offset + 2;
            rows.push(format!("(${parameter}::uuid)"));
            values.push(key.entity_id.into());
        }
        let sql = format!(
            "WITH requested(channel_id) AS (VALUES {}),\n{SALES_CHANNEL_ROWS_CTE}\n{SALES_CHANNEL_ROW_SELECT}\nJOIN requested requested_key ON requested_key.channel_id = row.channel_id\nORDER BY row.channel_id ASC",
            rows.join(", ")
        );
        self.db
            .query_all_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                sql,
                values,
            ))
            .await
            .map_err(|_| retryable("sales_channel_index_storage_unavailable"))
    }
}

#[async_trait]
impl IndexSource for SalesChannelPostgresIndexSource {
    async fn scan(
        &self,
        request: IndexSourceScanRequest,
    ) -> Result<IndexSourcePage, IndexSourceFailure> {
        self.validate_request(request.schema())?;
        let cursor = request
            .cursor()
            .map(SalesChannelCursor::decode)
            .transpose()
            .map_err(|_| permanent("sales_channel_index_cursor_invalid"))?;
        let rows = self.scan_rows(&request, cursor.as_ref()).await?;
        let has_more = rows.len() > request.limit();
        let mut mutations = Vec::with_capacity(rows.len().min(request.limit()));
        let mut next_cursor = None;
        for row in rows.into_iter().take(request.limit()) {
            let decoded = SalesChannelRow::decode(row, request.tenant_id())
                .map_err(|_| permanent("sales_channel_index_record_invalid"))?;
            if has_more {
                next_cursor = Some(
                    decoded
                        .cursor()
                        .encode()
                        .map_err(|_| permanent("sales_channel_index_cursor_invalid"))?,
                );
            }
            mutations.push(
                decoded
                    .into_mutation()
                    .map_err(|_| permanent("sales_channel_index_record_invalid"))?,
            );
        }
        IndexSourcePage::new(&request, mutations, next_cursor)
            .map_err(|_| permanent("sales_channel_index_page_invalid"))
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
                SalesChannelRow::decode(row, request.tenant_id())
                    .and_then(SalesChannelRow::into_mutation)
                    .map_err(|_| permanent("sales_channel_index_record_invalid"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        IndexSourceLoadBatch::new(&request, mutations)
            .map_err(|_| permanent("sales_channel_index_batch_invalid"))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SalesChannelCursor {
    channel_id: Uuid,
}

impl SalesChannelCursor {
    fn decode(cursor: &IndexSourceCursor) -> Result<Self, SalesChannelIndexBridgeError> {
        let decoded: Self = serde_json::from_value(cursor.value().clone())
            .map_err(|_| SalesChannelIndexBridgeError::InvalidCursor)?;
        if decoded.channel_id.is_nil() {
            return Err(SalesChannelIndexBridgeError::InvalidCursor);
        }
        Ok(decoded)
    }

    fn encode(&self) -> Result<IndexSourceCursor, SalesChannelIndexBridgeError> {
        let value =
            serde_json::to_value(self).map_err(|_| SalesChannelIndexBridgeError::InvalidCursor)?;
        IndexSourceCursor::new(value).map_err(|_| SalesChannelIndexBridgeError::InvalidCursor)
    }
}

#[derive(Debug)]
struct SalesChannelRow {
    tenant_id: Uuid,
    channel_id: Uuid,
    source_version: u64,
    state: SalesChannelRowState,
}

#[derive(Debug)]
enum SalesChannelRowState {
    Live(SalesChannelLiveFields),
    Deleted,
}

#[derive(Debug)]
struct SalesChannelLiveFields {
    slug: String,
    name: String,
    is_active: bool,
    is_default: bool,
    status: String,
}

impl SalesChannelRow {
    fn decode(
        row: QueryResult,
        expected_tenant: Uuid,
    ) -> Result<Self, SalesChannelIndexBridgeError> {
        let is_deleted = row
            .try_get::<bool>("", "is_deleted")
            .map_err(|_| SalesChannelIndexBridgeError::InvalidRow)?;
        let identity_count = row
            .try_get::<i64>("", "identity_count")
            .map_err(|_| SalesChannelIndexBridgeError::InvalidRow)?;
        let tenant_id = row
            .try_get::<Uuid>("", "tenant_id")
            .map_err(|_| SalesChannelIndexBridgeError::InvalidRow)?;
        let channel_id = row
            .try_get::<Uuid>("", "channel_id")
            .map_err(|_| SalesChannelIndexBridgeError::InvalidRow)?;
        let revision = row
            .try_get::<i64>("", "index_revision")
            .map_err(|_| SalesChannelIndexBridgeError::InvalidRow)?;
        if identity_count != 1
            || tenant_id != expected_tenant
            || tenant_id.is_nil()
            || channel_id.is_nil()
            || revision <= 0
        {
            return Err(SalesChannelIndexBridgeError::InvalidRow);
        }
        let source_version =
            u64::try_from(revision).map_err(|_| SalesChannelIndexBridgeError::InvalidRow)?;

        if is_deleted {
            return Ok(Self {
                tenant_id,
                channel_id,
                source_version,
                state: SalesChannelRowState::Deleted,
            });
        }

        Ok(Self {
            tenant_id,
            channel_id,
            source_version,
            state: SalesChannelRowState::Live(SalesChannelLiveFields {
                slug: required_string(&row, "slug")?,
                name: required_string(&row, "name")?,
                is_active: row
                    .try_get::<bool>("", "is_active")
                    .map_err(|_| SalesChannelIndexBridgeError::InvalidRow)?,
                is_default: row
                    .try_get::<bool>("", "is_default")
                    .map_err(|_| SalesChannelIndexBridgeError::InvalidRow)?,
                status: required_string(&row, "status")?,
            }),
        })
    }

    fn cursor(&self) -> SalesChannelCursor {
        SalesChannelCursor {
            channel_id: self.channel_id,
        }
    }

    fn into_mutation(self) -> Result<IndexMutation, SalesChannelIndexBridgeError> {
        let event_id = derive_index_source_event_id(
            SALES_CHANNEL_EVENT_DOMAIN,
            self.tenant_id,
            self.channel_id,
            None,
            self.source_version,
        )
        .map_err(|_| SalesChannelIndexBridgeError::InvalidRow)?;
        let key = EntityKey {
            tenant_id: self.tenant_id,
            schema: sales_channel_schema_ref()
                .map_err(SalesChannelIndexBridgeError::InvalidContract)?,
            entity_id: self.channel_id,
            locale: None,
        };

        let SalesChannelRowState::Live(live) = self.state else {
            return Ok(IndexMutation::Delete {
                event_id,
                key,
                source_version: self.source_version,
            });
        };

        let fields = BTreeMap::from([
            (field_name("id")?, IndexValue::Uuid(self.channel_id)),
            (field_name("slug")?, IndexValue::String(live.slug)),
            (field_name("name")?, IndexValue::String(live.name)),
            (
                field_name("is_active")?,
                IndexValue::Boolean(live.is_active),
            ),
            (
                field_name("is_default")?,
                IndexValue::Boolean(live.is_default),
            ),
            (field_name("status")?, IndexValue::String(live.status)),
        ]);
        Ok(IndexMutation::Upsert {
            event_id,
            record: IndexRecord {
                key,
                source_version: self.source_version,
                fields,
                links: Vec::new(),
            },
        })
    }
}

fn field_name(name: &str) -> Result<FieldName, SalesChannelIndexBridgeError> {
    FieldName::new(name).map_err(SalesChannelIndexBridgeError::InvalidContract)
}

fn required_string(
    row: &QueryResult,
    column: &str,
) -> Result<String, SalesChannelIndexBridgeError> {
    let value = row
        .try_get::<String>("", column)
        .map_err(|_| SalesChannelIndexBridgeError::InvalidRow)?;
    if value.is_empty() {
        Err(SalesChannelIndexBridgeError::InvalidRow)
    } else {
        Ok(value)
    }
}

fn retryable(code: &'static str) -> IndexSourceFailure {
    IndexSourceFailure::retryable(code).expect("static SalesChannel retry code must be valid")
}

fn permanent(code: &'static str) -> IndexSourceFailure {
    IndexSourceFailure::permanent(code).expect("static SalesChannel failure code must be valid")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selected_sales_channel_schema_is_nonlocalized_and_link_free() {
        let schema = sales_channel_schema().unwrap();
        assert_eq!(schema.reference, sales_channel_schema_ref().unwrap());
        assert_eq!(schema.locale_mode, LocaleMode::None);
        assert_eq!(schema.fields.len(), 6);
        assert!(schema.links.is_empty());
        assert!(
            schema
                .fields
                .iter()
                .any(|field| field.name.as_str() == "id")
        );
        assert!(
            schema
                .fields
                .iter()
                .any(|field| field.name.as_str() == "slug")
        );
        assert!(schema.fingerprint().is_ok());
    }

    #[test]
    fn selected_sales_channel_cursor_rejects_nil_and_unknown_fields() {
        for value in [
            serde_json::json!({"channel_id": Uuid::nil()}),
            serde_json::json!({"channel_id": Uuid::from_u128(2), "revision": 1}),
        ] {
            let cursor = IndexSourceCursor::new(value).unwrap();
            assert!(SalesChannelCursor::decode(&cursor).is_err());
        }
    }

    #[test]
    fn selected_sales_channel_tombstone_emits_delete_with_stable_identity() {
        let tenant_id = Uuid::from_u128(1);
        let channel_id = Uuid::from_u128(2);
        let mutation = SalesChannelRow {
            tenant_id,
            channel_id,
            source_version: 7,
            state: SalesChannelRowState::Deleted,
        }
        .into_mutation()
        .unwrap();

        match mutation {
            IndexMutation::Delete {
                event_id,
                key,
                source_version,
            } => {
                assert_eq!(key.tenant_id, tenant_id);
                assert_eq!(key.entity_id, channel_id);
                assert_eq!(key.locale, None);
                assert_eq!(source_version, 7);
                assert_eq!(
                    event_id,
                    derive_index_source_event_id(
                        SALES_CHANNEL_EVENT_DOMAIN,
                        tenant_id,
                        channel_id,
                        None,
                        7,
                    )
                    .unwrap()
                );
            }
            IndexMutation::Upsert { .. } => panic!("retained delete must not become an upsert"),
        }
    }

    #[test]
    fn selected_sales_channel_bridge_skips_partial_registry_without_channel_module() {
        let mut extensions = ModuleRuntimeExtensions::default();
        extensions.insert(rustok_index::IndexSchemaSourceCatalog::new());
        extensions.insert(rustok_index::PostgresIndexSourceFactoryCatalog::new());
        register(&mut extensions).unwrap();
        assert!(
            extensions
                .get::<rustok_index::IndexSchemaSourceCatalog>()
                .unwrap()
                .is_empty()
        );
        assert!(
            extensions
                .get::<rustok_index::PostgresIndexSourceFactoryCatalog>()
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn selected_sales_channel_bridge_registers_schema_and_factory() {
        let mut extensions = ModuleRuntimeExtensions::default();
        extensions.insert(rustok_channel::ChannelRuntimeSelected);
        extensions.insert(rustok_index::IndexSchemaSourceCatalog::new());
        extensions.insert(rustok_index::PostgresIndexSourceFactoryCatalog::new());
        register(&mut extensions).unwrap();
        let schema = sales_channel_schema().unwrap();
        assert_eq!(
            extensions
                .get::<rustok_index::IndexSchemaSourceCatalog>()
                .unwrap()
                .get(&schema.reference)
                .unwrap()
                .owner_module,
            "channel"
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
