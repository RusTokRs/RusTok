use std::collections::{BTreeMap, BTreeSet};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rustok_core::ModuleRuntimeExtensions;
use rustok_index::{
    DomainError, EntityKey, EntityName, FieldCardinality, FieldName, IndexField, IndexLink,
    IndexLinkValue, IndexMutation, IndexRecord, IndexSchema, IndexSource, IndexSourceCursor,
    IndexSourceFailure, IndexSourceLoadBatch, IndexSourceLoadRequest, IndexSourcePage,
    IndexSourceScanRequest, IndexValue, IndexValueType, LinkCardinality, LinkName, LinkedEntityKey,
    LocaleKey, LocaleMode, ModuleName, PostgresIndexSourceFactory, SchemaRef, SchemaVersion,
    derive_index_schema_source_event_id, register_index_schema_source, register_index_source,
    register_postgres_index_source_factory,
};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, QueryResult, Statement, Value};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use thiserror::Error;
use uuid::Uuid;

use rustok_product::{
    index_channel_visibility::decode_product_visibility,
    index_projection::{PRODUCT_ATTRIBUTE_TERMS_CTE, PRODUCT_ROW_SELECT},
};

use super::{PRODUCT_SCHEMA_ROUTING_KEY, with_channel_identity_source};

static PRODUCT_ROWS_CTE: std::sync::LazyLock<String> = std::sync::LazyLock::new(|| {
    with_channel_identity_source(rustok_product::index_projection::PRODUCT_ROWS_CTE)
});

pub(crate) const PRODUCT_INDEX_SOURCE: &str = "product-postgres-primary";
const PRODUCT_EVENT_DOMAIN: &str = "rustok-product.product-replay";
const PRODUCT_RELATION_FRESHNESS_PENDING_CODE: &str = "product_index_relation_freshness_pending";

#[derive(Debug, Error)]
enum ProductIndexBridgeError {
    #[error("Product Index contract is invalid")]
    InvalidContract(#[source] DomainError),
    #[error("Product Index cursor is invalid")]
    InvalidCursor,
    #[error("Product Index source row is invalid")]
    InvalidRow,
    #[error("Product Index relation freshness is pending")]
    FreshnessPending,
}

pub(crate) fn register(extensions: &mut ModuleRuntimeExtensions) -> rustok_core::Result<()> {
    if !extensions.contains::<rustok_product::ProductRuntimeSelected>() {
        return Ok(());
    }

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
        PRODUCT_INDEX_SOURCE,
        ProductPostgresIndexSourceFactory,
    )
    .map_err(|error| {
        rustok_core::Error::Validation(format!(
            "selected Product Index source factory registration failed: {error}"
        ))
    })
}

fn product_schema() -> Result<IndexSchema, ProductIndexBridgeError> {
    validated_schema(IndexSchema {
        reference: product_schema_ref()?,
        locale_mode: LocaleMode::Required,
        fields: vec![
            scalar_field("id", IndexValueType::Uuid, false, true, true)?,
            scalar_field("status", IndexValueType::String, false, true, true)?,
            scalar_field("title", IndexValueType::String, false, true, true)?,
            scalar_field("handle", IndexValueType::String, false, true, true)?,
            scalar_field("description", IndexValueType::String, true, false, false)?,
            scalar_field("seller_id", IndexValueType::String, true, false, false)?,
            scalar_field("vendor", IndexValueType::String, true, true, true)?,
            scalar_field("product_type", IndexValueType::String, true, true, true)?,
            scalar_field(
                "primary_category_id",
                IndexValueType::Uuid,
                true,
                true,
                false,
            )?,
            many_field("tag_ids", IndexValueType::Uuid, true, false)?,
            scalar_field("created_at", IndexValueType::Timestamp, false, false, true)?,
            scalar_field("published_at", IndexValueType::Timestamp, true, true, true)?,
            many_field("attribute_terms", IndexValueType::String, false, true)?,
            many_field("variant_ids", IndexValueType::Uuid, true, true)?,
            many_field("sales_channel_ids", IndexValueType::Uuid, true, true)?,
        ],
        links: vec![
            IndexLink {
                name: link_name("variants")?,
                source_fields: vec![field_name("variant_ids")?],
                target_schema: product_variant_schema_ref()?,
                target_fields: vec![field_name("id")?],
                cardinality: LinkCardinality::Many,
            },
            IndexLink {
                name: link_name("sales_channels")?,
                source_fields: vec![field_name("sales_channel_ids")?],
                target_schema: sales_channel_schema_ref()?,
                target_fields: vec![field_name("id")?],
                cardinality: LinkCardinality::Many,
            },
        ],
    })
}

fn validated_schema(schema: IndexSchema) -> Result<IndexSchema, ProductIndexBridgeError> {
    schema
        .validate()
        .map_err(ProductIndexBridgeError::InvalidContract)?;
    Ok(schema)
}

fn product_schema_ref() -> Result<SchemaRef, ProductIndexBridgeError> {
    Ok(SchemaRef {
        module: ModuleName::new("rustok-product")
            .map_err(ProductIndexBridgeError::InvalidContract)?,
        entity: EntityName::new("product").map_err(ProductIndexBridgeError::InvalidContract)?,
        version: SchemaVersion::new(PRODUCT_SCHEMA_ROUTING_KEY),
    })
}

fn product_variant_schema_ref() -> Result<SchemaRef, ProductIndexBridgeError> {
    Ok(SchemaRef {
        module: ModuleName::new("rustok-product")
            .map_err(ProductIndexBridgeError::InvalidContract)?,
        entity: EntityName::new("product_variant")
            .map_err(ProductIndexBridgeError::InvalidContract)?,
        version: SchemaVersion::new(2),
    })
}

fn sales_channel_schema_ref() -> Result<SchemaRef, ProductIndexBridgeError> {
    Ok(SchemaRef {
        module: ModuleName::new("rustok-channel")
            .map_err(ProductIndexBridgeError::InvalidContract)?,
        entity: EntityName::new("sales_channel")
            .map_err(ProductIndexBridgeError::InvalidContract)?,
        version: SchemaVersion::INITIAL,
    })
}

fn scalar_field(
    name: &str,
    value_type: IndexValueType,
    nullable: bool,
    filterable: bool,
    sortable: bool,
) -> Result<IndexField, ProductIndexBridgeError> {
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

fn many_field(
    name: &str,
    value_type: IndexValueType,
    selectable: bool,
    filterable: bool,
) -> Result<IndexField, ProductIndexBridgeError> {
    Ok(IndexField {
        name: field_name(name)?,
        value_type,
        cardinality: FieldCardinality::Many,
        nullable: false,
        selectable,
        filterable,
        sortable: false,
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
        require_postgres(&self.db)?;
        match product_schema_ref() {
            Ok(expected) if expected == *schema => Ok(()),
            _ => Err(permanent("product_index_schema_mismatch")),
        }
    }

    async fn scan_rows(
        &self,
        request: &IndexSourceScanRequest,
        cursor: Option<&ProductCursor>,
    ) -> Result<Vec<QueryResult>, IndexSourceFailure> {
        let fetch_limit = match i64::try_from(request.limit().saturating_add(1)) {
            Ok(limit) => limit,
            Err(_) => return Err(permanent("scan_limit_overflow")),
        };
        let rows_cte = PRODUCT_ROWS_CTE.as_str();
        let (sql, values): (String, Vec<Value>) = match (request.locale(), cursor) {
            (Some(locale), Some(cursor)) => (
                format!(
                    "WITH {PRODUCT_ATTRIBUTE_TERMS_CTE},\n{rows_cte}\n{PRODUCT_ROW_SELECT}\nWHERE row.locale = $2 AND row.product_id > $3\nORDER BY row.product_id ASC\nLIMIT $4"
                ),
                vec![
                    request.tenant_id().into(),
                    locale.as_str().to_owned().into(),
                    cursor.product_id.into(),
                    fetch_limit.into(),
                ],
            ),
            (Some(locale), None) => (
                format!(
                    "WITH {PRODUCT_ATTRIBUTE_TERMS_CTE},\n{rows_cte}\n{PRODUCT_ROW_SELECT}\nWHERE row.locale = $2\nORDER BY row.product_id ASC\nLIMIT $3"
                ),
                vec![
                    request.tenant_id().into(),
                    locale.as_str().to_owned().into(),
                    fetch_limit.into(),
                ],
            ),
            (None, Some(cursor)) => (
                format!(
                    "WITH {PRODUCT_ATTRIBUTE_TERMS_CTE},\n{rows_cte}\n{PRODUCT_ROW_SELECT}\nWHERE (row.product_id, row.locale) > ($2, $3)\nORDER BY row.product_id ASC, row.locale ASC\nLIMIT $4"
                ),
                vec![
                    request.tenant_id().into(),
                    cursor.product_id.into(),
                    cursor.locale.clone().into(),
                    fetch_limit.into(),
                ],
            ),
            (None, None) => (
                format!(
                    "WITH {PRODUCT_ATTRIBUTE_TERMS_CTE},\n{rows_cte}\n{PRODUCT_ROW_SELECT}\nORDER BY row.product_id ASC, row.locale ASC\nLIMIT $2"
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
            .map_err(|_| retryable("product_index_storage_unavailable"))
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
        let rows_cte = PRODUCT_ROWS_CTE.as_str();
        let sql = format!(
            "WITH requested(product_id, locale) AS (VALUES {}),\n{PRODUCT_ATTRIBUTE_TERMS_CTE},\n{rows_cte}\n{PRODUCT_ROW_SELECT}\nJOIN requested requested_key ON requested_key.product_id = row.product_id AND requested_key.locale = row.locale\nORDER BY row.product_id ASC, row.locale ASC",
            tuples.join(", ")
        );
        self.db
            .query_all_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                sql,
                values,
            ))
            .await
            .map_err(|_| retryable("product_index_storage_unavailable"))
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
            .map_err(|_| permanent("product_index_cursor_invalid"))?;
        if let (Some(locale), Some(cursor)) = (request.locale(), cursor.as_ref())
            && cursor.locale != locale.as_str()
        {
            return Err(permanent("product_index_cursor_invalid"));
        }
        let rows = self.scan_rows(&request, cursor.as_ref()).await?;
        let has_more = rows.len() > request.limit();
        let mut mutations = Vec::with_capacity(rows.len().min(request.limit()));
        let mut next_cursor = None;
        for row in rows.into_iter().take(request.limit()) {
            let decoded =
                ProductRow::decode(row, request.tenant_id()).map_err(map_product_decode_error)?;
            if has_more {
                next_cursor = Some(
                    decoded
                        .cursor()
                        .encode()
                        .map_err(|_| permanent("product_index_cursor_invalid"))?,
                );
            }
            mutations.push(
                decoded
                    .into_mutation()
                    .map_err(|_| permanent("product_index_record_invalid"))?,
            );
        }
        IndexSourcePage::new(&request, mutations, next_cursor)
            .map_err(|_| permanent("product_index_page_invalid"))
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
                    .map_err(map_product_decode_error)
                    .and_then(|row| {
                        row.into_mutation()
                            .map_err(|_| permanent("product_index_record_invalid"))
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        IndexSourceLoadBatch::new(&request, mutations)
            .map_err(|_| permanent("product_index_batch_invalid"))
    }
}

fn map_product_decode_error(error: ProductIndexBridgeError) -> IndexSourceFailure {
    match error {
        ProductIndexBridgeError::FreshnessPending => {
            retryable(PRODUCT_RELATION_FRESHNESS_PENDING_CODE)
        }
        ProductIndexBridgeError::InvalidContract(_)
        | ProductIndexBridgeError::InvalidCursor
        | ProductIndexBridgeError::InvalidRow => permanent("product_index_record_invalid"),
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
        let canonical =
            LocaleKey::new(&decoded.locale).map_err(ProductIndexBridgeError::InvalidContract)?;
        if canonical.as_str() != decoded.locale {
            return Err(ProductIndexBridgeError::InvalidCursor);
        }
        Ok(decoded)
    }

    fn encode(&self) -> Result<IndexSourceCursor, ProductIndexBridgeError> {
        let value =
            serde_json::to_value(self).map_err(|_| ProductIndexBridgeError::InvalidCursor)?;
        IndexSourceCursor::new(value).map_err(|_| ProductIndexBridgeError::InvalidCursor)
    }
}

#[derive(Debug)]
struct ProductRow {
    tenant_id: Uuid,
    product_id: Uuid,
    source_version: u64,
    locale: LocaleKey,
    state: ProductRowState,
}

#[derive(Debug)]
enum ProductRowState {
    Live(ProductLiveFields),
    Deleted,
}

#[derive(Debug)]
struct ProductLiveFields {
    status: String,
    title: String,
    handle: String,
    description: Option<String>,
    seller_id: Option<String>,
    vendor: Option<String>,
    product_type: Option<String>,
    primary_category_id: Option<Uuid>,
    tag_ids: Vec<Uuid>,
    created_at: DateTime<Utc>,
    published_at: Option<DateTime<Utc>>,
    attribute_terms: Vec<String>,
    variant_ids: Vec<Uuid>,
    sales_channel_ids: Vec<Uuid>,
}

impl ProductRow {
    fn decode(row: QueryResult, expected_tenant: Uuid) -> Result<Self, ProductIndexBridgeError> {
        let is_deleted = row
            .try_get::<bool>("", "is_deleted")
            .map_err(|_| ProductIndexBridgeError::InvalidRow)?;
        let identity_count = row
            .try_get::<i64>("", "identity_count")
            .map_err(|_| ProductIndexBridgeError::InvalidRow)?;
        let tenant_id = row
            .try_get::<Uuid>("", "tenant_id")
            .map_err(|_| ProductIndexBridgeError::InvalidRow)?;
        let product_id = row
            .try_get::<Uuid>("", "product_id")
            .map_err(|_| ProductIndexBridgeError::InvalidRow)?;
        let source_version = positive_u64(&row, "source_version")?;
        let observed_product_source_version =
            positive_u64(&row, "observed_product_source_version")?;
        let projected_product_source_version =
            positive_u64(&row, "projected_product_source_version")?;
        let _relation_epoch = positive_u64(&row, "relation_epoch")?;
        let raw_locale = row
            .try_get::<String>("", "locale")
            .map_err(|_| ProductIndexBridgeError::InvalidRow)?;
        if identity_count != 1
            || tenant_id != expected_tenant
            || tenant_id.is_nil()
            || product_id.is_nil()
        {
            return Err(ProductIndexBridgeError::InvalidRow);
        }
        let locale =
            LocaleKey::new(&raw_locale).map_err(ProductIndexBridgeError::InvalidContract)?;
        if locale.as_str() != raw_locale {
            return Err(ProductIndexBridgeError::InvalidRow);
        }

        // Projection and retained relation membership remain mandatory for delete replay. A deleted
        // Product does not require a live freshness witness (does not require live Storefront fields or a live freshness witness)
        // because its graph is being removed.
        let sales_channel_ids = decode_uuid_json_list(&row, "sales_channel_ids")?;

        if is_deleted {
            if projected_product_source_version < observed_product_source_version {
                return Err(ProductIndexBridgeError::InvalidRow);
            }
            return Ok(Self {
                tenant_id,
                product_id,
                source_version,
                locale,
                state: ProductRowState::Deleted,
            });
        }

        if projected_product_source_version != observed_product_source_version {
            return Err(ProductIndexBridgeError::InvalidRow);
        }

        let metadata = row
            .try_get::<JsonValue>("", "metadata")
            .map_err(|_| ProductIndexBridgeError::InvalidRow)?;
        let current_visibility_key = decode_product_visibility(&metadata)
            .map_err(|_| ProductIndexBridgeError::InvalidRow)?
            .freshness_key();
        let freshness_product_source_version =
            optional_positive_u64(&row, "freshness_product_source_version")?
                .ok_or(ProductIndexBridgeError::FreshnessPending)?;
        let freshness_visibility_key = row
            .try_get::<Option<String>>("", "freshness_visibility_key")
            .map_err(|_| ProductIndexBridgeError::InvalidRow)?
            .filter(|value| !value.is_empty())
            .ok_or(ProductIndexBridgeError::FreshnessPending)?;
        let freshness_channel_identity_generation =
            optional_non_negative_u64(&row, "freshness_channel_identity_generation")?
                .ok_or(ProductIndexBridgeError::FreshnessPending)?;
        let current_channel_identity_generation =
            non_negative_u64(&row, "current_channel_identity_generation")?;

        // Stale witness where freshness_channel_identity_generation != current_channel_identity_generation is rejected:
        if freshness_product_source_version > observed_product_source_version
            || freshness_channel_identity_generation > current_channel_identity_generation
        {
            return Err(ProductIndexBridgeError::InvalidRow);
        }
        if freshness_visibility_key != current_visibility_key
            || freshness_channel_identity_generation < current_channel_identity_generation
        {
            return Err(ProductIndexBridgeError::FreshnessPending);
        }

        let primary_category_id = row
            .try_get::<Option<Uuid>>("", "primary_category_id")
            .map_err(|_| ProductIndexBridgeError::InvalidRow)?;
        if primary_category_id.is_some_and(|id| id.is_nil()) {
            return Err(ProductIndexBridgeError::InvalidRow);
        }
        Ok(Self {
            tenant_id,
            product_id,
            source_version,
            locale,
            state: ProductRowState::Live(ProductLiveFields {
                status: required_string(&row, "status")?,
                title: required_string(&row, "title")?,
                handle: required_string(&row, "handle")?,
                description: optional_string(&row, "description")?,
                seller_id: optional_string(&row, "seller_id")?,
                vendor: optional_string(&row, "vendor")?,
                product_type: optional_string(&row, "product_type")?,
                primary_category_id,
                tag_ids: decode_uuid_json_list(&row, "tag_ids")?,
                created_at: required_timestamp(&row, "created_at")?,
                published_at: optional_timestamp(&row, "published_at")?,
                attribute_terms: decode_string_json_list(&row, "attribute_terms")?,
                variant_ids: decode_uuid_json_list(&row, "variant_ids")?,
                sales_channel_ids,
            }),
        })
    }

    fn cursor(&self) -> ProductCursor {
        ProductCursor {
            product_id: self.product_id,
            locale: self.locale.as_str().to_owned(),
        }
    }

    fn into_mutation(self) -> Result<IndexMutation, ProductIndexBridgeError> {
        let schema = product_schema_ref()?;
        let event_id = derive_index_schema_source_event_id(
            PRODUCT_EVENT_DOMAIN,
            self.tenant_id,
            &schema,
            self.product_id,
            Some(&self.locale),
            self.source_version,
        )
        .map_err(|_| ProductIndexBridgeError::InvalidRow)?;
        let key = EntityKey {
            tenant_id: self.tenant_id,
            schema,
            entity_id: self.product_id,
            locale: Some(self.locale),
        };

        let ProductRowState::Live(mut live) = self.state else {
            return Ok(IndexMutation::Delete {
                event_id,
                key,
                source_version: self.source_version,
            });
        };

        let fields = BTreeMap::from([
            (field_name("id")?, IndexValue::Uuid(self.product_id)),
            (field_name("status")?, IndexValue::String(live.status)),
            (field_name("title")?, IndexValue::String(live.title)),
            (field_name("handle")?, IndexValue::String(live.handle)),
            (
                field_name("description")?,
                optional_string_value(live.description.take()),
            ),
            (
                field_name("seller_id")?,
                optional_string_value(live.seller_id.take()),
            ),
            (
                field_name("vendor")?,
                optional_string_value(live.vendor.take()),
            ),
            (
                field_name("product_type")?,
                optional_string_value(live.product_type.take()),
            ),
            (
                field_name("primary_category_id")?,
                live.primary_category_id
                    .map(IndexValue::Uuid)
                    .unwrap_or(IndexValue::Null),
            ),
            (
                field_name("tag_ids")?,
                IndexValue::List(live.tag_ids.iter().copied().map(IndexValue::Uuid).collect()),
            ),
            (
                field_name("created_at")?,
                IndexValue::Timestamp(live.created_at),
            ),
            (
                field_name("published_at")?,
                live.published_at
                    .map(IndexValue::Timestamp)
                    .unwrap_or(IndexValue::Null),
            ),
            (
                field_name("attribute_terms")?,
                IndexValue::List(
                    live.attribute_terms
                        .iter()
                        .cloned()
                        .map(IndexValue::String)
                        .collect(),
                ),
            ),
            (
                field_name("variant_ids")?,
                IndexValue::List(
                    live.variant_ids
                        .iter()
                        .copied()
                        .map(IndexValue::Uuid)
                        .collect(),
                ),
            ),
            (
                field_name("sales_channel_ids")?,
                IndexValue::List(
                    live.sales_channel_ids
                        .iter()
                        .copied()
                        .map(IndexValue::Uuid)
                        .collect(),
                ),
            ),
        ]);
        let variant_schema = product_variant_schema_ref()?;
        let sales_channel_schema = sales_channel_schema_ref()?;
        let links = vec![
            IndexLinkValue {
                name: link_name("variants")?,
                targets: live
                    .variant_ids
                    .into_iter()
                    .map(|variant_id| LinkedEntityKey {
                        schema: variant_schema.clone(),
                        entity_id: variant_id,
                        locale: None,
                    })
                    .collect(),
            },
            IndexLinkValue {
                name: link_name("sales_channels")?,
                targets: live
                    .sales_channel_ids
                    .into_iter()
                    .map(|channel_id| LinkedEntityKey {
                        schema: sales_channel_schema.clone(),
                        entity_id: channel_id,
                        locale: None,
                    })
                    .collect(),
            },
        ];
        Ok(IndexMutation::Upsert {
            event_id,
            record: IndexRecord {
                key,
                source_version: self.source_version,
                fields,
                links,
            },
        })
    }
}

fn decode_uuid_json_list(
    row: &QueryResult,
    column: &str,
) -> Result<Vec<Uuid>, ProductIndexBridgeError> {
    let value = row
        .try_get::<JsonValue>("", column)
        .map_err(|_| ProductIndexBridgeError::InvalidRow)?;
    let values = value
        .as_array()
        .ok_or(ProductIndexBridgeError::InvalidRow)?;
    let mut unique = BTreeSet::new();
    for value in values {
        let raw = value.as_str().ok_or(ProductIndexBridgeError::InvalidRow)?;
        let id = Uuid::parse_str(raw).map_err(|_| ProductIndexBridgeError::InvalidRow)?;
        if id.is_nil() || !unique.insert(id) {
            return Err(ProductIndexBridgeError::InvalidRow);
        }
    }
    Ok(unique.into_iter().collect())
}

fn decode_string_json_list(
    row: &QueryResult,
    column: &str,
) -> Result<Vec<String>, ProductIndexBridgeError> {
    let value = row
        .try_get::<JsonValue>("", column)
        .map_err(|_| ProductIndexBridgeError::InvalidRow)?;
    let values = value
        .as_array()
        .ok_or(ProductIndexBridgeError::InvalidRow)?;
    let mut unique = BTreeSet::new();
    for value in values {
        let raw = value.as_str().ok_or(ProductIndexBridgeError::InvalidRow)?;
        if raw.is_empty() || !unique.insert(raw.to_owned()) {
            return Err(ProductIndexBridgeError::InvalidRow);
        }
    }
    Ok(unique.into_iter().collect())
}

fn positive_u64(row: &QueryResult, column: &str) -> Result<u64, ProductIndexBridgeError> {
    let value = row
        .try_get::<i64>("", column)
        .map_err(|_| ProductIndexBridgeError::InvalidRow)?;
    if value <= 0 {
        return Err(ProductIndexBridgeError::InvalidRow);
    }
    u64::try_from(value).map_err(|_| ProductIndexBridgeError::InvalidRow)
}

fn optional_positive_u64(
    row: &QueryResult,
    column: &str,
) -> Result<Option<u64>, ProductIndexBridgeError> {
    row.try_get::<Option<i64>>("", column)
        .map_err(|_| ProductIndexBridgeError::InvalidRow)?
        .map(|value| {
            if value <= 0 {
                return Err(ProductIndexBridgeError::InvalidRow);
            }
            u64::try_from(value).map_err(|_| ProductIndexBridgeError::InvalidRow)
        })
        .transpose()
}

fn non_negative_u64(row: &QueryResult, column: &str) -> Result<u64, ProductIndexBridgeError> {
    let value = row
        .try_get::<i64>("", column)
        .map_err(|_| ProductIndexBridgeError::InvalidRow)?;
    if value < 0 {
        return Err(ProductIndexBridgeError::InvalidRow);
    }
    u64::try_from(value).map_err(|_| ProductIndexBridgeError::InvalidRow)
}

fn optional_non_negative_u64(
    row: &QueryResult,
    column: &str,
) -> Result<Option<u64>, ProductIndexBridgeError> {
    row.try_get::<Option<i64>>("", column)
        .map_err(|_| ProductIndexBridgeError::InvalidRow)?
        .map(|value| {
            if value < 0 {
                return Err(ProductIndexBridgeError::InvalidRow);
            }
            u64::try_from(value).map_err(|_| ProductIndexBridgeError::InvalidRow)
        })
        .transpose()
}

fn require_postgres(db: &DatabaseConnection) -> Result<(), IndexSourceFailure> {
    if db.get_database_backend() == DbBackend::Postgres {
        Ok(())
    } else {
        Err(permanent("product_index_backend_unsupported"))
    }
}

fn field_name(name: &str) -> Result<FieldName, ProductIndexBridgeError> {
    FieldName::new(name).map_err(ProductIndexBridgeError::InvalidContract)
}

fn link_name(name: &str) -> Result<LinkName, ProductIndexBridgeError> {
    LinkName::new(name).map_err(ProductIndexBridgeError::InvalidContract)
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

fn required_timestamp(
    row: &QueryResult,
    column: &str,
) -> Result<DateTime<Utc>, ProductIndexBridgeError> {
    row.try_get::<DateTime<Utc>>("", column)
        .map_err(|_| ProductIndexBridgeError::InvalidRow)
}

fn optional_timestamp(
    row: &QueryResult,
    column: &str,
) -> Result<Option<DateTime<Utc>>, ProductIndexBridgeError> {
    row.try_get::<Option<DateTime<Utc>>>("", column)
        .map_err(|_| ProductIndexBridgeError::InvalidRow)
}

fn optional_string_value(value: Option<String>) -> IndexValue {
    value.map(IndexValue::String).unwrap_or(IndexValue::Null)
}

fn retryable(code: &'static str) -> IndexSourceFailure {
    match IndexSourceFailure::retryable(code) {
        Ok(failure) => failure,
        Err(_) => match IndexSourceFailure::retryable("internal_error") {
            Ok(fallback) => fallback,
            Err(_) => unreachable!("internal_error is a valid machine name"),
        },
    }
}

fn permanent(code: &'static str) -> IndexSourceFailure {
    match IndexSourceFailure::permanent(code) {
        Ok(failure) => failure,
        Err(_) => match IndexSourceFailure::permanent("internal_error") {
            Ok(fallback) => fallback,
            Err(_) => unreachable!("internal_error is a valid machine name"),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_product_schema_contains_only_current_storefront_graph_contract() {
        let schema = product_schema().unwrap();
        assert_eq!(schema.reference, product_schema_ref().unwrap());
        assert_eq!(schema.reference.version.get(), PRODUCT_SCHEMA_ROUTING_KEY);
        assert_eq!(schema.fields.len(), 15);
        assert_eq!(schema.links.len(), 2);
        for field in [
            "id",
            "status",
            "title",
            "handle",
            "description",
            "seller_id",
            "vendor",
            "product_type",
            "primary_category_id",
            "tag_ids",
            "created_at",
            "published_at",
            "attribute_terms",
            "variant_ids",
            "sales_channel_ids",
        ] {
            assert!(
                schema
                    .fields
                    .iter()
                    .any(|candidate| candidate.name.as_str() == field)
            );
        }
        let attribute_terms = schema
            .fields
            .iter()
            .find(|field| field.name.as_str() == "attribute_terms")
            .unwrap();
        assert_eq!(attribute_terms.cardinality, FieldCardinality::Many);
        assert!(attribute_terms.filterable);
        assert!(!attribute_terms.selectable);
        let published_at = schema
            .fields
            .iter()
            .find(|field| field.name.as_str() == "published_at")
            .unwrap();
        assert!(published_at.nullable);
        assert!(published_at.filterable);
        assert!(published_at.sortable);
        assert!(
            !schema
                .fields
                .iter()
                .any(|field| field.name.as_str() == "channel_restricted")
        );
        assert!(
            !schema
                .fields
                .iter()
                .any(|field| field.name.as_str() == "allowed_channel_slugs")
        );
        assert_eq!(schema.links[0].name.as_str(), "variants");
        assert_eq!(schema.links[1].name.as_str(), "sales_channels");
    }

    #[test]
    fn canonical_product_registration_publishes_one_schema_and_one_source_factory() {
        let mut extensions = ModuleRuntimeExtensions::default();
        extensions.insert(rustok_product::ProductRuntimeSelected);
        extensions.insert(rustok_index::IndexSchemaSourceCatalog::new());
        extensions.insert(rustok_index::PostgresIndexSourceFactoryCatalog::new());
        register(&mut extensions).unwrap();

        assert_eq!(
            extensions
                .get::<rustok_index::IndexSchemaSourceCatalog>()
                .unwrap()
                .len(),
            1
        );
        let factories = extensions
            .get::<rustok_index::PostgresIndexSourceFactoryCatalog>()
            .unwrap();
        assert_eq!(factories.len(), 1);
        assert!(factories.iter().any(|factory| {
            factory.owner_module() == "product" && factory.factory_name() == PRODUCT_INDEX_SOURCE
        }));
    }

    #[test]
    fn canonical_product_sql_materializes_storefront_graph_and_eav_state() {
        for marker in [
            "product_index_graph_projection_snapshots",
            "product_sales_channel_index_relation_snapshots",
            "product_sales_channel_index_relation_freshness_snapshots",
            "channel_index_identity_generations",
            "projection.projection_epoch AS source_version",
            "projection.channel_ids AS sales_channel_ids",
            "product_tags product_tag",
            "COALESCE(tags.tag_ids, '[]'::jsonb) AS tag_ids",
            "COALESCE(attributes.attribute_terms, '[]'::jsonb) AS attribute_terms",
            "p.seller_id",
            "p.created_at",
            "p.published_at",
            "COUNT(*) OVER",
            "product_index_tombstones",
        ] {
            assert!(PRODUCT_ROWS_CTE.contains(marker), "missing {marker}");
        }
        assert!(PRODUCT_ATTRIBUTE_TERMS_CTE.contains("product_filterable_attribute_values"));
    }

    #[test]
    fn relation_freshness_pending_is_retryable() {
        let failure = map_product_decode_error(ProductIndexBridgeError::FreshnessPending);
        assert!(failure.is_retryable());
        assert_eq!(failure.code(), PRODUCT_RELATION_FRESHNESS_PENDING_CODE);
    }

    #[test]
    fn retained_product_row_emits_current_schema_delete_mutation() {
        let product = ProductRow {
            tenant_id: Uuid::from_u128(1),
            product_id: Uuid::from_u128(2),
            source_version: 9,
            locale: LocaleKey::new("en-US").unwrap(),
            state: ProductRowState::Deleted,
        };
        let IndexMutation::Delete {
            key,
            source_version,
            ..
        } = product.into_mutation().unwrap()
        else {
            panic!("retained Product row must emit a delete");
        };
        assert_eq!(key.schema, product_schema_ref().unwrap());
        assert_eq!(key.locale.unwrap().as_str(), "en-US");
        assert_eq!(source_version, 9);
    }

    #[test]
    fn canonical_cursor_rejects_nil_noncanonical_and_unknown_fields() {
        for value in [
            serde_json::json!({"product_id": Uuid::nil(), "locale": "en-US"}),
            serde_json::json!({"product_id": Uuid::from_u128(2), "locale": "EN-us"}),
            serde_json::json!({"product_id": Uuid::from_u128(2), "locale": "en-US", "revision": 1}),
        ] {
            let cursor = IndexSourceCursor::new(value).unwrap();
            assert!(ProductCursor::decode(&cursor).is_err());
        }
    }
}
