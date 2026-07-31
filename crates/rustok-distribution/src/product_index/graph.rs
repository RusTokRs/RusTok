use std::collections::{BTreeMap, BTreeSet};

use async_trait::async_trait;
use rustok_core::ModuleRuntimeExtensions;
use rustok_index::{
    DomainError, EntityKey, EntityName, FieldCardinality, FieldName, IndexField, IndexLink,
    IndexLinkValue, IndexMutation, IndexRecord, IndexSchema, IndexSource, IndexSourceCursor,
    IndexSourceFailure, IndexSourceLoadBatch, IndexSourceLoadRequest, IndexSourcePage,
    IndexSourceScanRequest, IndexValue, IndexValueType, LinkCardinality, LinkName, LinkedEntityKey,
    LocaleKey, LocaleMode, ModuleName, PostgresIndexSourceFactory, SchemaRef, SchemaVersion,
    derive_index_source_event_id, register_index_schema_source, register_index_source,
    register_postgres_index_source_factory,
};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, QueryResult, Statement, Value};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use thiserror::Error;
use uuid::Uuid;

pub(crate) const PRODUCT_INDEX_SOURCE: &str = "product-postgres-primary";
pub(crate) const PRODUCT_VARIANT_INDEX_SOURCE: &str = "product-variant-postgres-primary";
const PRODUCT_EVENT_DOMAIN_V1: &str = "rustok-product.product-replay-v1";
const PRODUCT_EVENT_DOMAIN_V2: &str = "rustok-product.product-replay-v2";
const PRODUCT_VARIANT_EVENT_DOMAIN_V1: &str = "rustok-product.product-variant-replay-v1";
const PRODUCT_VARIANT_EVENT_DOMAIN_V2: &str = "rustok-product.product-variant-replay-v2";

const PRODUCT_ROWS_CTE: &str = r#"
product_index_union AS (
    SELECT
        FALSE AS is_deleted,
        p.tenant_id,
        p.id AS product_id,
        p.index_revision,
        p.status::text AS status,
        p.vendor,
        p.product_type,
        p.primary_category_id,
        p.metadata,
        t.locale,
        t.title,
        t.handle,
        t.description,
        COALESCE(
            (
                SELECT jsonb_agg(v.id ORDER BY v.id)
                FROM product_variants v
                WHERE v.tenant_id = p.tenant_id
                  AND v.product_id = p.id
            ),
            '[]'::jsonb
        ) AS variant_ids
    FROM products p
    JOIN product_translations t
      ON t.product_id = p.id
     AND t.tenant_id = p.tenant_id
    WHERE p.tenant_id = $1

    UNION ALL

    SELECT
        TRUE AS is_deleted,
        tombstone.tenant_id,
        tombstone.product_id,
        tombstone.source_version AS index_revision,
        NULL::text AS status,
        NULL::text AS vendor,
        NULL::text AS product_type,
        NULL::uuid AS primary_category_id,
        NULL::jsonb AS metadata,
        tombstone.locale,
        NULL::text AS title,
        NULL::text AS handle,
        NULL::text AS description,
        '[]'::jsonb AS variant_ids
    FROM product_index_tombstones tombstone
    WHERE tombstone.tenant_id = $1
),
product_index_rows AS (
    SELECT
        row.*,
        COUNT(*) OVER (
            PARTITION BY row.tenant_id, row.product_id, row.locale
        ) AS identity_count
    FROM product_index_union row
)
"#;

const PRODUCT_ROW_SELECT: &str = r#"
SELECT
    row.is_deleted,
    row.identity_count,
    row.tenant_id,
    row.product_id,
    row.index_revision,
    row.status,
    row.vendor,
    row.product_type,
    row.primary_category_id,
    row.metadata,
    row.locale,
    row.title,
    row.handle,
    row.description,
    row.variant_ids
FROM product_index_rows row
"#;

const PRODUCT_VARIANT_ROWS_CTE: &str = r#"
product_variant_index_union AS (
    SELECT
        FALSE AS is_deleted,
        v.tenant_id,
        v.id AS variant_id,
        v.product_id,
        v.index_revision,
        v.sku,
        v.barcode,
        v.shipping_profile_slug,
        v.ean,
        v.upc,
        v.inventory_policy,
        v.inventory_management,
        v.inventory_quantity,
        v.weight_unit,
        v.option1,
        v.option2,
        v.option3,
        v.position
    FROM product_variants v
    WHERE v.tenant_id = $1

    UNION ALL

    SELECT
        TRUE AS is_deleted,
        tombstone.tenant_id,
        tombstone.variant_id,
        NULL::uuid AS product_id,
        tombstone.source_version AS index_revision,
        NULL::text AS sku,
        NULL::text AS barcode,
        NULL::text AS shipping_profile_slug,
        NULL::text AS ean,
        NULL::text AS upc,
        NULL::text AS inventory_policy,
        NULL::text AS inventory_management,
        NULL::integer AS inventory_quantity,
        NULL::text AS weight_unit,
        NULL::text AS option1,
        NULL::text AS option2,
        NULL::text AS option3,
        NULL::integer AS position
    FROM product_variant_index_tombstones tombstone
    WHERE tombstone.tenant_id = $1
),
product_variant_index_rows AS (
    SELECT
        row.*,
        COUNT(*) OVER (
            PARTITION BY row.tenant_id, row.variant_id
        ) AS identity_count
    FROM product_variant_index_union row
)
"#;

const PRODUCT_VARIANT_ROW_SELECT: &str = r#"
SELECT
    row.is_deleted,
    row.identity_count,
    row.tenant_id,
    row.variant_id,
    row.product_id,
    row.index_revision,
    row.sku,
    row.barcode,
    row.shipping_profile_slug,
    row.ean,
    row.upc,
    row.inventory_policy,
    row.inventory_management,
    row.inventory_quantity,
    row.weight_unit,
    row.option1,
    row.option2,
    row.option3,
    row.position
FROM product_variant_index_rows row
"#;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProductSchemaVersion {
    V1,
    V2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProductVariantSchemaVersion {
    V1,
    V2,
}

#[derive(Debug, Error)]
enum ProductGraphIndexBridgeError {
    #[error("Product graph Index contract is invalid")]
    InvalidContract(#[source] DomainError),
    #[error("Product graph Index cursor is invalid")]
    InvalidCursor,
    #[error("Product graph Index source row is invalid")]
    InvalidRow,
}

pub(crate) fn register_product(
    extensions: &mut ModuleRuntimeExtensions,
) -> rustok_core::Result<()> {
    if !extensions.contains::<rustok_product::ProductRuntimeSelected>() {
        return Ok(());
    }

    for schema in [product_v1_schema(), product_v2_schema()] {
        let schema = schema.map_err(|error| {
            rustok_core::Error::Validation(format!(
                "selected Product Index schema construction failed: {error}"
            ))
        })?;
        register_index_schema_source(extensions, "product", schema).map_err(|error| {
            rustok_core::Error::Validation(format!(
                "selected Product Index schema registration failed: {error}"
            ))
        })?;
    }
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

pub(crate) fn register_variant(
    extensions: &mut ModuleRuntimeExtensions,
) -> rustok_core::Result<()> {
    if !extensions.contains::<rustok_product::ProductRuntimeSelected>() {
        return Ok(());
    }

    for schema in [product_variant_v1_schema(), product_variant_v2_schema()] {
        let schema = schema.map_err(|error| {
            rustok_core::Error::Validation(format!(
                "selected ProductVariant Index schema construction failed: {error}"
            ))
        })?;
        register_index_schema_source(extensions, "product", schema).map_err(|error| {
            rustok_core::Error::Validation(format!(
                "selected ProductVariant Index schema registration failed: {error}"
            ))
        })?;
    }
    register_postgres_index_source_factory(
        extensions,
        "product",
        PRODUCT_VARIANT_INDEX_SOURCE,
        ProductVariantPostgresIndexSourceFactory,
    )
    .map_err(|error| {
        rustok_core::Error::Validation(format!(
            "selected ProductVariant Index source factory registration failed: {error}"
        ))
    })
}

fn product_v1_schema() -> Result<IndexSchema, ProductGraphIndexBridgeError> {
    validated_schema(IndexSchema {
        reference: product_schema_ref(1)?,
        locale_mode: LocaleMode::Required,
        fields: vec![
            scalar_field("status", IndexValueType::String, false, true, true)?,
            scalar_field("title", IndexValueType::String, false, true, true)?,
            scalar_field("handle", IndexValueType::String, false, true, true)?,
            scalar_field("description", IndexValueType::String, true, false, false)?,
            scalar_field("vendor", IndexValueType::String, true, true, true)?,
            scalar_field("product_type", IndexValueType::String, true, true, true)?,
            scalar_field(
                "primary_category_id",
                IndexValueType::Uuid,
                true,
                true,
                false,
            )?,
        ],
        links: Vec::new(),
    })
}

fn product_v2_schema() -> Result<IndexSchema, ProductGraphIndexBridgeError> {
    validated_schema(IndexSchema {
        reference: product_schema_ref(2)?,
        locale_mode: LocaleMode::Required,
        fields: vec![
            scalar_field("id", IndexValueType::Uuid, false, true, true)?,
            scalar_field("status", IndexValueType::String, false, true, true)?,
            scalar_field("title", IndexValueType::String, false, true, true)?,
            scalar_field("handle", IndexValueType::String, false, true, true)?,
            scalar_field("description", IndexValueType::String, true, false, false)?,
            scalar_field("vendor", IndexValueType::String, true, true, true)?,
            scalar_field("product_type", IndexValueType::String, true, true, true)?,
            scalar_field(
                "primary_category_id",
                IndexValueType::Uuid,
                true,
                true,
                false,
            )?,
            scalar_field(
                "channel_restricted",
                IndexValueType::Boolean,
                false,
                true,
                false,
            )?,
            many_field("allowed_channel_slugs", IndexValueType::String, true)?,
            many_field("variant_ids", IndexValueType::Uuid, true)?,
        ],
        links: vec![IndexLink {
            name: link_name("variants")?,
            source_fields: vec![field_name("variant_ids")?],
            target_schema: product_variant_schema_ref(2)?,
            target_fields: vec![field_name("id")?],
            cardinality: LinkCardinality::Many,
        }],
    })
}

fn product_variant_v1_schema() -> Result<IndexSchema, ProductGraphIndexBridgeError> {
    validated_schema(IndexSchema {
        reference: product_variant_schema_ref(1)?,
        locale_mode: LocaleMode::None,
        fields: product_variant_fields(false)?,
        links: Vec::new(),
    })
}

fn product_variant_v2_schema() -> Result<IndexSchema, ProductGraphIndexBridgeError> {
    validated_schema(IndexSchema {
        reference: product_variant_schema_ref(2)?,
        locale_mode: LocaleMode::None,
        fields: product_variant_fields(true)?,
        links: Vec::new(),
    })
}

fn product_variant_fields(
    include_identity: bool,
) -> Result<Vec<IndexField>, ProductGraphIndexBridgeError> {
    let mut fields = Vec::with_capacity(if include_identity { 15 } else { 14 });
    if include_identity {
        fields.push(scalar_field(
            "id",
            IndexValueType::Uuid,
            false,
            true,
            true,
        )?);
    }
    fields.extend([
        scalar_field("product_id", IndexValueType::Uuid, false, true, true)?,
        scalar_field("sku", IndexValueType::String, true, true, true)?,
        scalar_field("barcode", IndexValueType::String, true, true, false)?,
        scalar_field(
            "shipping_profile_slug",
            IndexValueType::String,
            true,
            true,
            false,
        )?,
        scalar_field("ean", IndexValueType::String, true, true, false)?,
        scalar_field("upc", IndexValueType::String, true, true, false)?,
        scalar_field(
            "inventory_policy",
            IndexValueType::String,
            false,
            true,
            false,
        )?,
        scalar_field(
            "inventory_management",
            IndexValueType::String,
            false,
            true,
            false,
        )?,
        scalar_field(
            "inventory_quantity",
            IndexValueType::Integer,
            false,
            true,
            true,
        )?,
        scalar_field("weight_unit", IndexValueType::String, true, false, false)?,
        scalar_field("option1", IndexValueType::String, true, true, false)?,
        scalar_field("option2", IndexValueType::String, true, true, false)?,
        scalar_field("option3", IndexValueType::String, true, true, false)?,
        scalar_field("position", IndexValueType::Integer, false, true, true)?,
    ]);
    Ok(fields)
}

fn validated_schema(
    schema: IndexSchema,
) -> Result<IndexSchema, ProductGraphIndexBridgeError> {
    schema
        .validate()
        .map_err(ProductGraphIndexBridgeError::InvalidContract)?;
    Ok(schema)
}

fn product_schema_ref(version: u32) -> Result<SchemaRef, ProductGraphIndexBridgeError> {
    Ok(SchemaRef {
        module: ModuleName::new("rustok-product")
            .map_err(ProductGraphIndexBridgeError::InvalidContract)?,
        entity: EntityName::new("product")
            .map_err(ProductGraphIndexBridgeError::InvalidContract)?,
        version: SchemaVersion::new(version),
    })
}

fn product_variant_schema_ref(
    version: u32,
) -> Result<SchemaRef, ProductGraphIndexBridgeError> {
    Ok(SchemaRef {
        module: ModuleName::new("rustok-product")
            .map_err(ProductGraphIndexBridgeError::InvalidContract)?,
        entity: EntityName::new("product_variant")
            .map_err(ProductGraphIndexBridgeError::InvalidContract)?,
        version: SchemaVersion::new(version),
    })
}

fn scalar_field(
    name: &str,
    value_type: IndexValueType,
    nullable: bool,
    filterable: bool,
    sortable: bool,
) -> Result<IndexField, ProductGraphIndexBridgeError> {
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
    filterable: bool,
) -> Result<IndexField, ProductGraphIndexBridgeError> {
    Ok(IndexField {
        name: field_name(name)?,
        value_type,
        cardinality: FieldCardinality::Many,
        nullable: false,
        selectable: true,
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
            [
                product_schema_ref(1).map_err(|error| error.to_string())?,
                product_schema_ref(2).map_err(|error| error.to_string())?,
            ],
            ProductPostgresIndexSource { db },
        )
        .map_err(|error| error.to_string())
    }
}

#[derive(Clone, Copy, Debug)]
struct ProductVariantPostgresIndexSourceFactory;

impl PostgresIndexSourceFactory for ProductVariantPostgresIndexSourceFactory {
    fn register_source(
        &self,
        extensions: &mut ModuleRuntimeExtensions,
        db: DatabaseConnection,
    ) -> Result<(), String> {
        register_index_source(
            extensions,
            "product",
            PRODUCT_VARIANT_INDEX_SOURCE,
            [
                product_variant_schema_ref(1).map_err(|error| error.to_string())?,
                product_variant_schema_ref(2).map_err(|error| error.to_string())?,
            ],
            ProductVariantPostgresIndexSource { db },
        )
        .map_err(|error| error.to_string())
    }
}

#[derive(Clone, Debug)]
struct ProductPostgresIndexSource {
    db: DatabaseConnection,
}

impl ProductPostgresIndexSource {
    fn validate_request(
        &self,
        schema: &SchemaRef,
    ) -> Result<ProductSchemaVersion, IndexSourceFailure> {
        require_postgres(&self.db)?;
        match schema.version.get() {
            1 if product_schema_ref(1).is_ok_and(|expected| expected == *schema) => {
                Ok(ProductSchemaVersion::V1)
            }
            2 if product_schema_ref(2).is_ok_and(|expected| expected == *schema) => {
                Ok(ProductSchemaVersion::V2)
            }
            _ => Err(permanent("product_index_schema_mismatch")),
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
                    "WITH {PRODUCT_ROWS_CTE}\n{PRODUCT_ROW_SELECT}\nWHERE (row.product_id, row.locale) > ($2, $3)\nORDER BY row.product_id ASC, row.locale ASC\nLIMIT $4"
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
                    "WITH {PRODUCT_ROWS_CTE}\n{PRODUCT_ROW_SELECT}\nORDER BY row.product_id ASC, row.locale ASC\nLIMIT $2"
                ),
                vec![request.tenant_id().into(), fetch_limit.into()],
            ),
        };
        self.db
            .query_all(Statement::from_sql_and_values(
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
        let sql = format!(
            "WITH requested(product_id, locale) AS (VALUES {}),\n{PRODUCT_ROWS_CTE}\n{PRODUCT_ROW_SELECT}\nJOIN requested requested_key ON requested_key.product_id = row.product_id AND requested_key.locale = row.locale\nORDER BY row.product_id ASC, row.locale ASC",
            tuples.join(", ")
        );
        self.db
            .query_all(Statement::from_sql_and_values(
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
        let version = self.validate_request(request.schema())?;
        let cursor = request
            .cursor()
            .map(ProductCursor::decode)
            .transpose()
            .map_err(|_| permanent("product_index_cursor_invalid"))?;
        let rows = self.scan_rows(&request, cursor.as_ref()).await?;
        let has_more = rows.len() > request.limit();
        let mut mutations = Vec::with_capacity(rows.len().min(request.limit()));
        let mut next_cursor = None;
        for row in rows.into_iter().take(request.limit()) {
            let decoded = ProductRow::decode(row, request.tenant_id())
                .map_err(|_| permanent("product_index_record_invalid"))?;
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
                    .into_mutation(version)
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
        let version = self.validate_request(request.schema())?;
        let mutations = self
            .load_rows(&request)
            .await?
            .into_iter()
            .map(|row| {
                ProductRow::decode(row, request.tenant_id())
                    .and_then(|row| row.into_mutation(version))
                    .map_err(|_| permanent("product_index_record_invalid"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        IndexSourceLoadBatch::new(&request, mutations)
            .map_err(|_| permanent("product_index_batch_invalid"))
    }
}

#[derive(Clone, Debug)]
struct ProductVariantPostgresIndexSource {
    db: DatabaseConnection,
}

impl ProductVariantPostgresIndexSource {
    fn validate_request(
        &self,
        schema: &SchemaRef,
    ) -> Result<ProductVariantSchemaVersion, IndexSourceFailure> {
        require_postgres(&self.db)?;
        match schema.version.get() {
            1 if product_variant_schema_ref(1).is_ok_and(|expected| expected == *schema) => {
                Ok(ProductVariantSchemaVersion::V1)
            }
            2 if product_variant_schema_ref(2).is_ok_and(|expected| expected == *schema) => {
                Ok(ProductVariantSchemaVersion::V2)
            }
            _ => Err(permanent("product_variant_index_schema_mismatch")),
        }
    }

    async fn scan_rows(
        &self,
        request: &IndexSourceScanRequest,
        cursor: Option<&ProductVariantCursor>,
    ) -> Result<Vec<QueryResult>, IndexSourceFailure> {
        let fetch_limit = i64::try_from(request.limit() + 1)
            .expect("Index source page limit is bounded below i64::MAX");
        let (sql, values): (String, Vec<Value>) = match cursor {
            Some(cursor) => (
                format!(
                    "WITH {PRODUCT_VARIANT_ROWS_CTE}\n{PRODUCT_VARIANT_ROW_SELECT}\nWHERE row.variant_id > $2\nORDER BY row.variant_id ASC\nLIMIT $3"
                ),
                vec![
                    request.tenant_id().into(),
                    cursor.variant_id.into(),
                    fetch_limit.into(),
                ],
            ),
            None => (
                format!(
                    "WITH {PRODUCT_VARIANT_ROWS_CTE}\n{PRODUCT_VARIANT_ROW_SELECT}\nORDER BY row.variant_id ASC\nLIMIT $2"
                ),
                vec![request.tenant_id().into(), fetch_limit.into()],
            ),
        };
        self.db
            .query_all(Statement::from_sql_and_values(
                DbBackend::Postgres,
                sql,
                values,
            ))
            .await
            .map_err(|_| retryable("product_variant_index_storage_unavailable"))
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
                return Err(permanent("product_variant_index_locale_forbidden"));
            }
            let parameter = offset + 2;
            rows.push(format!("(${parameter}::uuid)"));
            values.push(key.entity_id.into());
        }
        let sql = format!(
            "WITH requested(variant_id) AS (VALUES {}),\n{PRODUCT_VARIANT_ROWS_CTE}\n{PRODUCT_VARIANT_ROW_SELECT}\nJOIN requested requested_key ON requested_key.variant_id = row.variant_id\nORDER BY row.variant_id ASC",
            rows.join(", ")
        );
        self.db
            .query_all(Statement::from_sql_and_values(
                DbBackend::Postgres,
                sql,
                values,
            ))
            .await
            .map_err(|_| retryable("product_variant_index_storage_unavailable"))
    }
}

#[async_trait]
impl IndexSource for ProductVariantPostgresIndexSource {
    async fn scan(
        &self,
        request: IndexSourceScanRequest,
    ) -> Result<IndexSourcePage, IndexSourceFailure> {
        let version = self.validate_request(request.schema())?;
        let cursor = request
            .cursor()
            .map(ProductVariantCursor::decode)
            .transpose()
            .map_err(|_| permanent("product_variant_index_cursor_invalid"))?;
        let rows = self.scan_rows(&request, cursor.as_ref()).await?;
        let has_more = rows.len() > request.limit();
        let mut mutations = Vec::with_capacity(rows.len().min(request.limit()));
        let mut next_cursor = None;
        for row in rows.into_iter().take(request.limit()) {
            let decoded = ProductVariantRow::decode(row, request.tenant_id())
                .map_err(|_| permanent("product_variant_index_record_invalid"))?;
            if has_more {
                next_cursor = Some(
                    decoded
                        .cursor()
                        .encode()
                        .map_err(|_| permanent("product_variant_index_cursor_invalid"))?,
                );
            }
            mutations.push(
                decoded
                    .into_mutation(version)
                    .map_err(|_| permanent("product_variant_index_record_invalid"))?,
            );
        }
        IndexSourcePage::new(&request, mutations, next_cursor)
            .map_err(|_| permanent("product_variant_index_page_invalid"))
    }

    async fn load(
        &self,
        request: IndexSourceLoadRequest,
    ) -> Result<IndexSourceLoadBatch, IndexSourceFailure> {
        let version = self.validate_request(request.schema())?;
        let mutations = self
            .load_rows(&request)
            .await?
            .into_iter()
            .map(|row| {
                ProductVariantRow::decode(row, request.tenant_id())
                    .and_then(|row| row.into_mutation(version))
                    .map_err(|_| permanent("product_variant_index_record_invalid"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        IndexSourceLoadBatch::new(&request, mutations)
            .map_err(|_| permanent("product_variant_index_batch_invalid"))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProductCursor {
    product_id: Uuid,
    locale: String,
}

impl ProductCursor {
    fn decode(cursor: &IndexSourceCursor) -> Result<Self, ProductGraphIndexBridgeError> {
        let decoded: Self = serde_json::from_value(cursor.value().clone())
            .map_err(|_| ProductGraphIndexBridgeError::InvalidCursor)?;
        if decoded.product_id.is_nil() {
            return Err(ProductGraphIndexBridgeError::InvalidCursor);
        }
        let canonical = LocaleKey::new(&decoded.locale)
            .map_err(ProductGraphIndexBridgeError::InvalidContract)?;
        if canonical.as_str() != decoded.locale {
            return Err(ProductGraphIndexBridgeError::InvalidCursor);
        }
        Ok(decoded)
    }

    fn encode(&self) -> Result<IndexSourceCursor, ProductGraphIndexBridgeError> {
        let value = serde_json::to_value(self)
            .map_err(|_| ProductGraphIndexBridgeError::InvalidCursor)?;
        IndexSourceCursor::new(value).map_err(|_| ProductGraphIndexBridgeError::InvalidCursor)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProductVariantCursor {
    variant_id: Uuid,
}

impl ProductVariantCursor {
    fn decode(cursor: &IndexSourceCursor) -> Result<Self, ProductGraphIndexBridgeError> {
        let decoded: Self = serde_json::from_value(cursor.value().clone())
            .map_err(|_| ProductGraphIndexBridgeError::InvalidCursor)?;
        if decoded.variant_id.is_nil() {
            return Err(ProductGraphIndexBridgeError::InvalidCursor);
        }
        Ok(decoded)
    }

    fn encode(&self) -> Result<IndexSourceCursor, ProductGraphIndexBridgeError> {
        let value = serde_json::to_value(self)
            .map_err(|_| ProductGraphIndexBridgeError::InvalidCursor)?;
        IndexSourceCursor::new(value).map_err(|_| ProductGraphIndexBridgeError::InvalidCursor)
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
    vendor: Option<String>,
    product_type: Option<String>,
    primary_category_id: Option<Uuid>,
    allowed_channel_slugs: Vec<String>,
    variant_ids: Vec<Uuid>,
}

impl ProductRow {
    fn decode(
        row: QueryResult,
        expected_tenant: Uuid,
    ) -> Result<Self, ProductGraphIndexBridgeError> {
        let is_deleted = row
            .try_get::<bool>("", "is_deleted")
            .map_err(|_| ProductGraphIndexBridgeError::InvalidRow)?;
        let identity_count = row
            .try_get::<i64>("", "identity_count")
            .map_err(|_| ProductGraphIndexBridgeError::InvalidRow)?;
        let tenant_id = row
            .try_get::<Uuid>("", "tenant_id")
            .map_err(|_| ProductGraphIndexBridgeError::InvalidRow)?;
        let product_id = row
            .try_get::<Uuid>("", "product_id")
            .map_err(|_| ProductGraphIndexBridgeError::InvalidRow)?;
        let revision = row
            .try_get::<i64>("", "index_revision")
            .map_err(|_| ProductGraphIndexBridgeError::InvalidRow)?;
        let raw_locale = row
            .try_get::<String>("", "locale")
            .map_err(|_| ProductGraphIndexBridgeError::InvalidRow)?;
        if identity_count != 1
            || tenant_id != expected_tenant
            || tenant_id.is_nil()
            || product_id.is_nil()
            || revision <= 0
        {
            return Err(ProductGraphIndexBridgeError::InvalidRow);
        }
        let locale = LocaleKey::new(&raw_locale)
            .map_err(ProductGraphIndexBridgeError::InvalidContract)?;
        if locale.as_str() != raw_locale {
            return Err(ProductGraphIndexBridgeError::InvalidRow);
        }
        let source_version =
            u64::try_from(revision).map_err(|_| ProductGraphIndexBridgeError::InvalidRow)?;

        if is_deleted {
            return Ok(Self {
                tenant_id,
                product_id,
                source_version,
                locale,
                state: ProductRowState::Deleted,
            });
        }

        let primary_category_id = row
            .try_get::<Option<Uuid>>("", "primary_category_id")
            .map_err(|_| ProductGraphIndexBridgeError::InvalidRow)?;
        if primary_category_id.is_some_and(|id| id.is_nil()) {
            return Err(ProductGraphIndexBridgeError::InvalidRow);
        }
        let metadata = row
            .try_get::<JsonValue>("", "metadata")
            .map_err(|_| ProductGraphIndexBridgeError::InvalidRow)?;
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
                vendor: optional_string(&row, "vendor")?,
                product_type: optional_string(&row, "product_type")?,
                primary_category_id,
                allowed_channel_slugs: extract_allowed_channel_slugs(&metadata),
                variant_ids: decode_uuid_json_list(&row, "variant_ids")?,
            }),
        })
    }

    fn cursor(&self) -> ProductCursor {
        ProductCursor {
            product_id: self.product_id,
            locale: self.locale.as_str().to_owned(),
        }
    }

    fn into_mutation(
        self,
        version: ProductSchemaVersion,
    ) -> Result<IndexMutation, ProductGraphIndexBridgeError> {
        let (schema_version, event_domain) = match version {
            ProductSchemaVersion::V1 => (1, PRODUCT_EVENT_DOMAIN_V1),
            ProductSchemaVersion::V2 => (2, PRODUCT_EVENT_DOMAIN_V2),
        };
        let event_id = derive_index_source_event_id(
            event_domain,
            self.tenant_id,
            self.product_id,
            Some(&self.locale),
            self.source_version,
        )
        .map_err(|_| ProductGraphIndexBridgeError::InvalidRow)?;
        let key = EntityKey {
            tenant_id: self.tenant_id,
            schema: product_schema_ref(schema_version)?,
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

        let mut fields = BTreeMap::from([
            (field_name("status")?, IndexValue::String(live.status)),
            (field_name("title")?, IndexValue::String(live.title)),
            (field_name("handle")?, IndexValue::String(live.handle)),
            (
                field_name("description")?,
                optional_string_value(live.description.take()),
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
        ]);
        let links = if version == ProductSchemaVersion::V2 {
            fields.insert(field_name("id")?, IndexValue::Uuid(self.product_id));
            fields.insert(
                field_name("channel_restricted")?,
                IndexValue::Boolean(!live.allowed_channel_slugs.is_empty()),
            );
            fields.insert(
                field_name("allowed_channel_slugs")?,
                IndexValue::List(
                    live.allowed_channel_slugs
                        .into_iter()
                        .map(IndexValue::String)
                        .collect(),
                ),
            );
            fields.insert(
                field_name("variant_ids")?,
                IndexValue::List(
                    live.variant_ids
                        .iter()
                        .copied()
                        .map(IndexValue::Uuid)
                        .collect(),
                ),
            );
            let variant_schema = product_variant_schema_ref(2)?;
            vec![IndexLinkValue {
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
            }]
        } else {
            Vec::new()
        };
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

#[derive(Debug)]
struct ProductVariantRow {
    tenant_id: Uuid,
    variant_id: Uuid,
    source_version: u64,
    state: ProductVariantRowState,
}

#[derive(Debug)]
enum ProductVariantRowState {
    Live(ProductVariantLiveFields),
    Deleted,
}

#[derive(Debug)]
struct ProductVariantLiveFields {
    product_id: Uuid,
    sku: Option<String>,
    barcode: Option<String>,
    shipping_profile_slug: Option<String>,
    ean: Option<String>,
    upc: Option<String>,
    inventory_policy: String,
    inventory_management: String,
    inventory_quantity: i64,
    weight_unit: Option<String>,
    option1: Option<String>,
    option2: Option<String>,
    option3: Option<String>,
    position: i64,
}

impl ProductVariantRow {
    fn decode(
        row: QueryResult,
        expected_tenant: Uuid,
    ) -> Result<Self, ProductGraphIndexBridgeError> {
        let is_deleted = row
            .try_get::<bool>("", "is_deleted")
            .map_err(|_| ProductGraphIndexBridgeError::InvalidRow)?;
        let identity_count = row
            .try_get::<i64>("", "identity_count")
            .map_err(|_| ProductGraphIndexBridgeError::InvalidRow)?;
        let tenant_id = row
            .try_get::<Uuid>("", "tenant_id")
            .map_err(|_| ProductGraphIndexBridgeError::InvalidRow)?;
        let variant_id = row
            .try_get::<Uuid>("", "variant_id")
            .map_err(|_| ProductGraphIndexBridgeError::InvalidRow)?;
        let revision = row
            .try_get::<i64>("", "index_revision")
            .map_err(|_| ProductGraphIndexBridgeError::InvalidRow)?;
        if identity_count != 1
            || tenant_id != expected_tenant
            || tenant_id.is_nil()
            || variant_id.is_nil()
            || revision <= 0
        {
            return Err(ProductGraphIndexBridgeError::InvalidRow);
        }
        let source_version =
            u64::try_from(revision).map_err(|_| ProductGraphIndexBridgeError::InvalidRow)?;

        if is_deleted {
            return Ok(Self {
                tenant_id,
                variant_id,
                source_version,
                state: ProductVariantRowState::Deleted,
            });
        }

        let product_id = row
            .try_get::<Uuid>("", "product_id")
            .map_err(|_| ProductGraphIndexBridgeError::InvalidRow)?;
        if product_id.is_nil() {
            return Err(ProductGraphIndexBridgeError::InvalidRow);
        }
        Ok(Self {
            tenant_id,
            variant_id,
            source_version,
            state: ProductVariantRowState::Live(ProductVariantLiveFields {
                product_id,
                sku: optional_string(&row, "sku")?,
                barcode: optional_string(&row, "barcode")?,
                shipping_profile_slug: optional_string(&row, "shipping_profile_slug")?,
                ean: optional_string(&row, "ean")?,
                upc: optional_string(&row, "upc")?,
                inventory_policy: required_string(&row, "inventory_policy")?,
                inventory_management: required_string(&row, "inventory_management")?,
                inventory_quantity: i64::from(
                    row.try_get::<i32>("", "inventory_quantity")
                        .map_err(|_| ProductGraphIndexBridgeError::InvalidRow)?,
                ),
                weight_unit: optional_string(&row, "weight_unit")?,
                option1: optional_string(&row, "option1")?,
                option2: optional_string(&row, "option2")?,
                option3: optional_string(&row, "option3")?,
                position: i64::from(
                    row.try_get::<i32>("", "position")
                        .map_err(|_| ProductGraphIndexBridgeError::InvalidRow)?,
                ),
            }),
        })
    }

    fn cursor(&self) -> ProductVariantCursor {
        ProductVariantCursor {
            variant_id: self.variant_id,
        }
    }

    fn into_mutation(
        self,
        version: ProductVariantSchemaVersion,
    ) -> Result<IndexMutation, ProductGraphIndexBridgeError> {
        let (schema_version, event_domain) = match version {
            ProductVariantSchemaVersion::V1 => (1, PRODUCT_VARIANT_EVENT_DOMAIN_V1),
            ProductVariantSchemaVersion::V2 => (2, PRODUCT_VARIANT_EVENT_DOMAIN_V2),
        };
        let event_id = derive_index_source_event_id(
            event_domain,
            self.tenant_id,
            self.variant_id,
            None,
            self.source_version,
        )
        .map_err(|_| ProductGraphIndexBridgeError::InvalidRow)?;
        let key = EntityKey {
            tenant_id: self.tenant_id,
            schema: product_variant_schema_ref(schema_version)?,
            entity_id: self.variant_id,
            locale: None,
        };

        let ProductVariantRowState::Live(mut live) = self.state else {
            return Ok(IndexMutation::Delete {
                event_id,
                key,
                source_version: self.source_version,
            });
        };

        let mut fields = BTreeMap::from([
            (
                field_name("product_id")?,
                IndexValue::Uuid(live.product_id),
            ),
            (field_name("sku")?, optional_string_value(live.sku.take())),
            (
                field_name("barcode")?,
                optional_string_value(live.barcode.take()),
            ),
            (
                field_name("shipping_profile_slug")?,
                optional_string_value(live.shipping_profile_slug.take()),
            ),
            (field_name("ean")?, optional_string_value(live.ean.take())),
            (field_name("upc")?, optional_string_value(live.upc.take())),
            (
                field_name("inventory_policy")?,
                IndexValue::String(live.inventory_policy),
            ),
            (
                field_name("inventory_management")?,
                IndexValue::String(live.inventory_management),
            ),
            (
                field_name("inventory_quantity")?,
                IndexValue::Integer(live.inventory_quantity),
            ),
            (
                field_name("weight_unit")?,
                optional_string_value(live.weight_unit.take()),
            ),
            (
                field_name("option1")?,
                optional_string_value(live.option1.take()),
            ),
            (
                field_name("option2")?,
                optional_string_value(live.option2.take()),
            ),
            (
                field_name("option3")?,
                optional_string_value(live.option3.take()),
            ),
            (
                field_name("position")?,
                IndexValue::Integer(live.position),
            ),
        ]);
        if version == ProductVariantSchemaVersion::V2 {
            fields.insert(field_name("id")?, IndexValue::Uuid(self.variant_id));
        }
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

fn extract_allowed_channel_slugs(metadata: &JsonValue) -> Vec<String> {
    let Some(values) = metadata
        .as_object()
        .and_then(|object| object.get("channel_visibility"))
        .and_then(JsonValue::as_object)
        .and_then(|object| object.get("allowed_channel_slugs"))
        .and_then(JsonValue::as_array)
    else {
        return Vec::new();
    };

    let mut normalized = BTreeSet::new();
    for value in values {
        if let Some(slug) = value
            .as_str()
            .map(str::trim)
            .filter(|slug| !slug.is_empty())
            .map(str::to_ascii_lowercase)
        {
            normalized.insert(slug);
        }
    }
    normalized.into_iter().collect()
}

fn decode_uuid_json_list(
    row: &QueryResult,
    column: &str,
) -> Result<Vec<Uuid>, ProductGraphIndexBridgeError> {
    let value = row
        .try_get::<JsonValue>("", column)
        .map_err(|_| ProductGraphIndexBridgeError::InvalidRow)?;
    let values = value
        .as_array()
        .ok_or(ProductGraphIndexBridgeError::InvalidRow)?;
    let mut unique = BTreeSet::new();
    for value in values {
        let raw = value
            .as_str()
            .ok_or(ProductGraphIndexBridgeError::InvalidRow)?;
        let id = Uuid::parse_str(raw).map_err(|_| ProductGraphIndexBridgeError::InvalidRow)?;
        if id.is_nil() || !unique.insert(id) {
            return Err(ProductGraphIndexBridgeError::InvalidRow);
        }
    }
    Ok(unique.into_iter().collect())
}

fn require_postgres(db: &DatabaseConnection) -> Result<(), IndexSourceFailure> {
    if db.get_database_backend() == DbBackend::Postgres {
        Ok(())
    } else {
        Err(permanent("product_index_backend_unsupported"))
    }
}

fn field_name(name: &str) -> Result<FieldName, ProductGraphIndexBridgeError> {
    FieldName::new(name).map_err(ProductGraphIndexBridgeError::InvalidContract)
}

fn link_name(name: &str) -> Result<LinkName, ProductGraphIndexBridgeError> {
    LinkName::new(name).map_err(ProductGraphIndexBridgeError::InvalidContract)
}

fn required_string(
    row: &QueryResult,
    column: &str,
) -> Result<String, ProductGraphIndexBridgeError> {
    let value = row
        .try_get::<String>("", column)
        .map_err(|_| ProductGraphIndexBridgeError::InvalidRow)?;
    if value.is_empty() {
        Err(ProductGraphIndexBridgeError::InvalidRow)
    } else {
        Ok(value)
    }
}

fn optional_string(
    row: &QueryResult,
    column: &str,
) -> Result<Option<String>, ProductGraphIndexBridgeError> {
    row.try_get::<Option<String>>("", column)
        .map_err(|_| ProductGraphIndexBridgeError::InvalidRow)
}

fn optional_string_value(value: Option<String>) -> IndexValue {
    value.map(IndexValue::String).unwrap_or(IndexValue::Null)
}

fn retryable(code: &'static str) -> IndexSourceFailure {
    IndexSourceFailure::retryable(code).expect("static Product source retry code must be valid")
}

fn permanent(code: &'static str) -> IndexSourceFailure {
    IndexSourceFailure::permanent(code).expect("static Product source failure code must be valid")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versioned_product_graph_preserves_v1_and_adds_product_to_variant_path() {
        let product_v1 = product_v1_schema().unwrap();
        let product_v2 = product_v2_schema().unwrap();
        let variant_v1 = product_variant_v1_schema().unwrap();
        let variant_v2 = product_variant_v2_schema().unwrap();

        assert_eq!(product_v1.reference.version, SchemaVersion::INITIAL);
        assert_eq!(variant_v1.reference.version, SchemaVersion::INITIAL);
        assert!(product_v1.links.is_empty());
        assert!(variant_v1.links.is_empty());
        assert_eq!(product_v1.fields.len(), 7);
        assert_eq!(variant_v1.fields.len(), 14);

        assert_eq!(product_v2.reference.version, SchemaVersion::new(2));
        assert_eq!(variant_v2.reference.version, SchemaVersion::new(2));
        assert_eq!(product_v2.fields.len(), 11);
        assert_eq!(variant_v2.fields.len(), 15);
        assert_eq!(product_v2.links.len(), 1);
        assert_eq!(product_v2.links[0].name.as_str(), "variants");
        assert_eq!(product_v2.links[0].target_schema, variant_v2.reference);
        assert_eq!(product_v2.links[0].cardinality, LinkCardinality::Many);
    }

    #[test]
    fn versioned_sources_keep_one_stable_source_per_schema_identity() {
        let mut extensions = ModuleRuntimeExtensions::default();
        extensions.insert(rustok_product::ProductRuntimeSelected);
        extensions.insert(rustok_index::IndexSchemaSourceCatalog::new());
        extensions.insert(rustok_index::PostgresIndexSourceFactoryCatalog::new());
        register_product(&mut extensions).unwrap();
        register_variant(&mut extensions).unwrap();

        assert_eq!(
            extensions
                .get::<rustok_index::IndexSchemaSourceCatalog>()
                .unwrap()
                .len(),
            4
        );
        let factories = extensions
            .get::<rustok_index::PostgresIndexSourceFactoryCatalog>()
            .unwrap();
        assert_eq!(factories.len(), 2);
        assert!(factories.iter().any(|factory| {
            factory.owner_module() == "product"
                && factory.factory_name() == PRODUCT_INDEX_SOURCE
        }));
        assert!(factories.iter().any(|factory| {
            factory.owner_module() == "product"
                && factory.factory_name() == PRODUCT_VARIANT_INDEX_SOURCE
        }));
    }

    #[test]
    fn retained_rows_emit_versioned_delete_mutations() {
        let product = ProductRow {
            tenant_id: Uuid::from_u128(1),
            product_id: Uuid::from_u128(2),
            source_version: 9,
            locale: LocaleKey::new("en-US").unwrap(),
            state: ProductRowState::Deleted,
        };
        let variant = ProductVariantRow {
            tenant_id: Uuid::from_u128(1),
            variant_id: Uuid::from_u128(3),
            source_version: 10,
            state: ProductVariantRowState::Deleted,
        };

        let IndexMutation::Delete {
            key: product_key,
            source_version: product_version,
            ..
        } = product.into_mutation(ProductSchemaVersion::V2).unwrap()
        else {
            panic!("retained Product row must emit a delete");
        };
        let IndexMutation::Delete {
            key: variant_key,
            source_version: variant_version,
            ..
        } = variant
            .into_mutation(ProductVariantSchemaVersion::V1)
            .unwrap()
        else {
            panic!("retained ProductVariant row must emit a delete");
        };

        assert_eq!(product_key.schema.version, SchemaVersion::new(2));
        assert_eq!(product_key.locale.unwrap().as_str(), "en-US");
        assert_eq!(product_version, 9);
        assert_eq!(variant_key.schema.version, SchemaVersion::INITIAL);
        assert!(variant_key.locale.is_none());
        assert_eq!(variant_version, 10);
    }

    #[test]
    fn tombstone_sql_fails_closed_on_live_identity_coexistence() {
        assert!(PRODUCT_ROWS_CTE.contains("COUNT(*) OVER"));
        assert!(PRODUCT_ROWS_CTE.contains("product_index_tombstones"));
        assert!(PRODUCT_VARIANT_ROWS_CTE.contains("product_variant_index_tombstones"));
        assert!(PRODUCT_ROW_SELECT.contains("identity_count"));
        assert!(PRODUCT_VARIANT_ROW_SELECT.contains("identity_count"));
    }

    #[test]
    fn channel_visibility_matches_storefront_normalization() {
        let metadata = serde_json::json!({
            "channel_visibility": {
                "allowed_channel_slugs": [" Web ", "mobile", "web", "", null]
            }
        });
        assert_eq!(
            extract_allowed_channel_slugs(&metadata),
            vec!["mobile".to_owned(), "web".to_owned()]
        );
        assert!(extract_allowed_channel_slugs(&serde_json::json!({})).is_empty());
    }

    #[test]
    fn versioned_cursors_reject_nil_noncanonical_and_unknown_fields() {
        for value in [
            serde_json::json!({"product_id": Uuid::nil(), "locale": "en-US"}),
            serde_json::json!({"product_id": Uuid::from_u128(2), "locale": "EN-us"}),
            serde_json::json!({"product_id": Uuid::from_u128(2), "locale": "en-US", "revision": 1}),
        ] {
            let cursor = IndexSourceCursor::new(value).unwrap();
            assert!(ProductCursor::decode(&cursor).is_err());
        }
        for value in [
            serde_json::json!({"variant_id": Uuid::nil()}),
            serde_json::json!({"variant_id": Uuid::from_u128(2), "revision": 1}),
        ] {
            let cursor = IndexSourceCursor::new(value).unwrap();
            assert!(ProductVariantCursor::decode(&cursor).is_err());
        }
    }
}
