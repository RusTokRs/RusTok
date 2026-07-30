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

pub(crate) const PRODUCT_GRAPH_INDEX_SOURCE: &str = "product-graph-postgres-v2";
const PRODUCT_GRAPH_INDEX_FACTORY: &str = "product-graph-postgres-v2";
const PRODUCT_V2_EVENT_DOMAIN: &str = "rustok-product.product-replay-v2";
const PRODUCT_VARIANT_V2_EVENT_DOMAIN: &str = "rustok-product.product-variant-replay-v2";

const PRODUCT_V2_SELECT: &str = r#"
SELECT
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
"#;

const PRODUCT_VARIANT_V2_SELECT: &str = r#"
SELECT
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
"#;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProductGraphSchema {
    Product,
    Variant,
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

pub(crate) fn register(extensions: &mut ModuleRuntimeExtensions) -> rustok_core::Result<()> {
    if !extensions.contains::<rustok_product::ProductRuntimeSelected>() {
        return Ok(());
    }

    let product_schema = product_v2_schema().map_err(|error| {
        rustok_core::Error::Validation(format!(
            "selected Product v2 Index schema construction failed: {error}"
        ))
    })?;
    let variant_schema = product_variant_v2_schema().map_err(|error| {
        rustok_core::Error::Validation(format!(
            "selected ProductVariant v2 Index schema construction failed: {error}"
        ))
    })?;
    register_index_schema_source(extensions, "product", product_schema).map_err(|error| {
        rustok_core::Error::Validation(format!(
            "selected Product v2 Index schema registration failed: {error}"
        ))
    })?;
    register_index_schema_source(extensions, "product", variant_schema).map_err(|error| {
        rustok_core::Error::Validation(format!(
            "selected ProductVariant v2 Index schema registration failed: {error}"
        ))
    })?;
    register_postgres_index_source_factory(
        extensions,
        "product",
        PRODUCT_GRAPH_INDEX_FACTORY,
        ProductGraphPostgresIndexSourceFactory,
    )
    .map_err(|error| {
        rustok_core::Error::Validation(format!(
            "selected Product graph Index source factory registration failed: {error}"
        ))
    })
}

fn product_v2_schema() -> Result<IndexSchema, ProductGraphIndexBridgeError> {
    let schema = IndexSchema {
        reference: product_v2_schema_ref()
            .map_err(ProductGraphIndexBridgeError::InvalidContract)?,
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
            name: LinkName::new("variants")
                .map_err(ProductGraphIndexBridgeError::InvalidContract)?,
            source_fields: vec![
                FieldName::new("variant_ids")
                    .map_err(ProductGraphIndexBridgeError::InvalidContract)?,
            ],
            target_schema: product_variant_v2_schema_ref()
                .map_err(ProductGraphIndexBridgeError::InvalidContract)?,
            target_fields: vec![
                FieldName::new("id")
                    .map_err(ProductGraphIndexBridgeError::InvalidContract)?,
            ],
            cardinality: LinkCardinality::Many,
        }],
    };
    schema
        .validate()
        .map_err(ProductGraphIndexBridgeError::InvalidContract)?;
    Ok(schema)
}

fn product_variant_v2_schema() -> Result<IndexSchema, ProductGraphIndexBridgeError> {
    let schema = IndexSchema {
        reference: product_variant_v2_schema_ref()
            .map_err(ProductGraphIndexBridgeError::InvalidContract)?,
        locale_mode: LocaleMode::None,
        fields: vec![
            scalar_field("id", IndexValueType::Uuid, false, true, true)?,
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
        ],
        links: Vec::new(),
    };
    schema
        .validate()
        .map_err(ProductGraphIndexBridgeError::InvalidContract)?;
    Ok(schema)
}

fn product_v2_schema_ref() -> Result<SchemaRef, DomainError> {
    Ok(SchemaRef {
        module: ModuleName::new("rustok-product")?,
        entity: EntityName::new("product")?,
        version: SchemaVersion::new(2),
    })
}

fn product_variant_v2_schema_ref() -> Result<SchemaRef, DomainError> {
    Ok(SchemaRef {
        module: ModuleName::new("rustok-product")?,
        entity: EntityName::new("product_variant")?,
        version: SchemaVersion::new(2),
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
        name: FieldName::new(name).map_err(ProductGraphIndexBridgeError::InvalidContract)?,
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
        name: FieldName::new(name).map_err(ProductGraphIndexBridgeError::InvalidContract)?,
        value_type,
        cardinality: FieldCardinality::Many,
        nullable: false,
        selectable: true,
        filterable,
        sortable: false,
    })
}

#[derive(Clone, Copy, Debug)]
struct ProductGraphPostgresIndexSourceFactory;

impl PostgresIndexSourceFactory for ProductGraphPostgresIndexSourceFactory {
    fn register_source(
        &self,
        extensions: &mut ModuleRuntimeExtensions,
        db: DatabaseConnection,
    ) -> Result<(), String> {
        register_index_source(
            extensions,
            "product",
            PRODUCT_GRAPH_INDEX_SOURCE,
            [
                product_v2_schema_ref().map_err(|error| error.to_string())?,
                product_variant_v2_schema_ref().map_err(|error| error.to_string())?,
            ],
            ProductGraphPostgresIndexSource { db },
        )
        .map_err(|error| error.to_string())
    }
}

#[derive(Clone, Debug)]
struct ProductGraphPostgresIndexSource {
    db: DatabaseConnection,
}

impl ProductGraphPostgresIndexSource {
    fn validate_request(
        &self,
        schema: &SchemaRef,
    ) -> Result<ProductGraphSchema, IndexSourceFailure> {
        if self.db.get_database_backend() != DbBackend::Postgres {
            return Err(permanent("product_graph_index_backend_unsupported"));
        }
        if let Ok(expected) = product_v2_schema_ref() {
            if &expected == schema {
                return Ok(ProductGraphSchema::Product);
            }
        }
        if let Ok(expected) = product_variant_v2_schema_ref() {
            if &expected == schema {
                return Ok(ProductGraphSchema::Variant);
            }
        }
        Err(permanent("product_graph_index_schema_mismatch"))
    }

    async fn scan_product_rows(
        &self,
        request: &IndexSourceScanRequest,
        cursor: Option<&ProductV2Cursor>,
    ) -> Result<Vec<QueryResult>, IndexSourceFailure> {
        let fetch_limit = i64::try_from(request.limit() + 1)
            .expect("Index source page limit is bounded below i64::MAX");
        let (sql, values): (String, Vec<Value>) = match cursor {
            Some(cursor) => (
                format!(
                    "{PRODUCT_V2_SELECT}\nWHERE p.tenant_id = $1\n  AND (p.id, t.locale) > ($2, $3)\nORDER BY p.id ASC, t.locale ASC\nLIMIT $4"
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
                    "{PRODUCT_V2_SELECT}\nWHERE p.tenant_id = $1\nORDER BY p.id ASC, t.locale ASC\nLIMIT $2"
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
            .map_err(|_| retryable("product_graph_index_storage_unavailable"))
    }

    async fn load_product_rows(
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
                .ok_or_else(|| permanent("product_graph_index_locale_required"))?;
            tuples.push(format!("(${parameter}::uuid, ${}::text)", parameter + 1));
            values.push(key.entity_id.into());
            values.push(locale.as_str().to_owned().into());
            parameter += 2;
        }
        let sql = format!(
            "WITH requested(product_id, locale) AS (VALUES {})\n{PRODUCT_V2_SELECT}\nJOIN requested r ON r.product_id = p.id AND r.locale = t.locale\nWHERE p.tenant_id = $1\nORDER BY p.id ASC, t.locale ASC",
            tuples.join(", ")
        );
        self.db
            .query_all(Statement::from_sql_and_values(
                DbBackend::Postgres,
                sql,
                values,
            ))
            .await
            .map_err(|_| retryable("product_graph_index_storage_unavailable"))
    }

    async fn scan_variant_rows(
        &self,
        request: &IndexSourceScanRequest,
        cursor: Option<&ProductVariantV2Cursor>,
    ) -> Result<Vec<QueryResult>, IndexSourceFailure> {
        let fetch_limit = i64::try_from(request.limit() + 1)
            .expect("Index source page limit is bounded below i64::MAX");
        let (sql, values): (String, Vec<Value>) = match cursor {
            Some(cursor) => (
                format!(
                    "{PRODUCT_VARIANT_V2_SELECT}\nWHERE v.tenant_id = $1\n  AND v.id > $2\nORDER BY v.id ASC\nLIMIT $3"
                ),
                vec![
                    request.tenant_id().into(),
                    cursor.variant_id.into(),
                    fetch_limit.into(),
                ],
            ),
            None => (
                format!(
                    "{PRODUCT_VARIANT_V2_SELECT}\nWHERE v.tenant_id = $1\nORDER BY v.id ASC\nLIMIT $2"
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
            .map_err(|_| retryable("product_graph_index_storage_unavailable"))
    }

    async fn load_variant_rows(
        &self,
        request: &IndexSourceLoadRequest,
    ) -> Result<Vec<QueryResult>, IndexSourceFailure> {
        let mut values = Vec::<Value>::with_capacity(1 + request.keys().len());
        values.push(request.tenant_id().into());
        let mut rows = Vec::with_capacity(request.keys().len());
        for (offset, key) in request.keys().iter().enumerate() {
            if key.locale.is_some() {
                return Err(permanent("product_graph_variant_locale_forbidden"));
            }
            let parameter = offset + 2;
            rows.push(format!("(${parameter}::uuid)"));
            values.push(key.entity_id.into());
        }
        let sql = format!(
            "WITH requested(variant_id) AS (VALUES {})\n{PRODUCT_VARIANT_V2_SELECT}\nJOIN requested r ON r.variant_id = v.id\nWHERE v.tenant_id = $1\nORDER BY v.id ASC",
            rows.join(", ")
        );
        self.db
            .query_all(Statement::from_sql_and_values(
                DbBackend::Postgres,
                sql,
                values,
            ))
            .await
            .map_err(|_| retryable("product_graph_index_storage_unavailable"))
    }

    async fn scan_products(
        &self,
        request: IndexSourceScanRequest,
    ) -> Result<IndexSourcePage, IndexSourceFailure> {
        let cursor = request
            .cursor()
            .map(ProductV2Cursor::decode)
            .transpose()
            .map_err(|_| permanent("product_graph_product_cursor_invalid"))?;
        let rows = self.scan_product_rows(&request, cursor.as_ref()).await?;
        let has_more = rows.len() > request.limit();
        let mut mutations = Vec::with_capacity(rows.len().min(request.limit()));
        let mut next_cursor = None;
        for row in rows.into_iter().take(request.limit()) {
            let decoded = ProductV2Row::decode(row, request.tenant_id())
                .map_err(|_| permanent("product_graph_product_record_invalid"))?;
            if has_more {
                next_cursor = Some(
                    decoded
                        .cursor()
                        .encode()
                        .map_err(|_| permanent("product_graph_product_cursor_invalid"))?,
                );
            }
            mutations.push(
                decoded
                    .into_mutation()
                    .map_err(|_| permanent("product_graph_product_record_invalid"))?,
            );
        }
        IndexSourcePage::new(&request, mutations, next_cursor)
            .map_err(|_| permanent("product_graph_product_page_invalid"))
    }

    async fn scan_variants(
        &self,
        request: IndexSourceScanRequest,
    ) -> Result<IndexSourcePage, IndexSourceFailure> {
        let cursor = request
            .cursor()
            .map(ProductVariantV2Cursor::decode)
            .transpose()
            .map_err(|_| permanent("product_graph_variant_cursor_invalid"))?;
        let rows = self.scan_variant_rows(&request, cursor.as_ref()).await?;
        let has_more = rows.len() > request.limit();
        let mut mutations = Vec::with_capacity(rows.len().min(request.limit()));
        let mut next_cursor = None;
        for row in rows.into_iter().take(request.limit()) {
            let decoded = ProductVariantV2Row::decode(row, request.tenant_id())
                .map_err(|_| permanent("product_graph_variant_record_invalid"))?;
            if has_more {
                next_cursor = Some(
                    decoded
                        .cursor()
                        .encode()
                        .map_err(|_| permanent("product_graph_variant_cursor_invalid"))?,
                );
            }
            mutations.push(
                decoded
                    .into_mutation()
                    .map_err(|_| permanent("product_graph_variant_record_invalid"))?,
            );
        }
        IndexSourcePage::new(&request, mutations, next_cursor)
            .map_err(|_| permanent("product_graph_variant_page_invalid"))
    }

    async fn load_products(
        &self,
        request: IndexSourceLoadRequest,
    ) -> Result<IndexSourceLoadBatch, IndexSourceFailure> {
        let mutations = self
            .load_product_rows(&request)
            .await?
            .into_iter()
            .map(|row| {
                ProductV2Row::decode(row, request.tenant_id())
                    .and_then(ProductV2Row::into_mutation)
                    .map_err(|_| permanent("product_graph_product_record_invalid"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        IndexSourceLoadBatch::new(&request, mutations)
            .map_err(|_| permanent("product_graph_product_batch_invalid"))
    }

    async fn load_variants(
        &self,
        request: IndexSourceLoadRequest,
    ) -> Result<IndexSourceLoadBatch, IndexSourceFailure> {
        let mutations = self
            .load_variant_rows(&request)
            .await?
            .into_iter()
            .map(|row| {
                ProductVariantV2Row::decode(row, request.tenant_id())
                    .and_then(ProductVariantV2Row::into_mutation)
                    .map_err(|_| permanent("product_graph_variant_record_invalid"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        IndexSourceLoadBatch::new(&request, mutations)
            .map_err(|_| permanent("product_graph_variant_batch_invalid"))
    }
}

#[async_trait]
impl IndexSource for ProductGraphPostgresIndexSource {
    async fn scan(
        &self,
        request: IndexSourceScanRequest,
    ) -> Result<IndexSourcePage, IndexSourceFailure> {
        match self.validate_request(request.schema())? {
            ProductGraphSchema::Product => self.scan_products(request).await,
            ProductGraphSchema::Variant => self.scan_variants(request).await,
        }
    }

    async fn load(
        &self,
        request: IndexSourceLoadRequest,
    ) -> Result<IndexSourceLoadBatch, IndexSourceFailure> {
        match self.validate_request(request.schema())? {
            ProductGraphSchema::Product => self.load_products(request).await,
            ProductGraphSchema::Variant => self.load_variants(request).await,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProductV2Cursor {
    product_id: Uuid,
    locale: String,
}

impl ProductV2Cursor {
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
struct ProductVariantV2Cursor {
    variant_id: Uuid,
}

impl ProductVariantV2Cursor {
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
struct ProductV2Row {
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
    allowed_channel_slugs: Vec<String>,
    variant_ids: Vec<Uuid>,
}

impl ProductV2Row {
    fn decode(
        row: QueryResult,
        expected_tenant: Uuid,
    ) -> Result<Self, ProductGraphIndexBridgeError> {
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
        if tenant_id != expected_tenant
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
        let primary_category_id = row
            .try_get::<Option<Uuid>>("", "primary_category_id")
            .map_err(|_| ProductGraphIndexBridgeError::InvalidRow)?;
        if primary_category_id.is_some_and(|id| id.is_nil()) {
            return Err(ProductGraphIndexBridgeError::InvalidRow);
        }
        let metadata = row
            .try_get::<JsonValue>("", "metadata")
            .map_err(|_| ProductGraphIndexBridgeError::InvalidRow)?;
        let variant_ids = decode_uuid_json_list(&row, "variant_ids")?;
        Ok(Self {
            tenant_id,
            product_id,
            source_version: u64::try_from(revision)
                .map_err(|_| ProductGraphIndexBridgeError::InvalidRow)?,
            locale,
            status: required_string(&row, "status")?,
            title: required_string(&row, "title")?,
            handle: required_string(&row, "handle")?,
            description: optional_string(&row, "description")?,
            vendor: optional_string(&row, "vendor")?,
            product_type: optional_string(&row, "product_type")?,
            primary_category_id,
            allowed_channel_slugs: extract_allowed_channel_slugs(&metadata),
            variant_ids,
        })
    }

    fn cursor(&self) -> ProductV2Cursor {
        ProductV2Cursor {
            product_id: self.product_id,
            locale: self.locale.as_str().to_owned(),
        }
    }

    fn into_mutation(mut self) -> Result<IndexMutation, ProductGraphIndexBridgeError> {
        let event_id = derive_index_source_event_id(
            PRODUCT_V2_EVENT_DOMAIN,
            self.tenant_id,
            self.product_id,
            Some(&self.locale),
            self.source_version,
        )
        .map_err(|_| ProductGraphIndexBridgeError::InvalidRow)?;
        let channel_restricted = !self.allowed_channel_slugs.is_empty();
        let fields = BTreeMap::from([
            (field_name("id")?, IndexValue::Uuid(self.product_id)),
            (field_name("status")?, IndexValue::String(self.status)),
            (field_name("title")?, IndexValue::String(self.title)),
            (field_name("handle")?, IndexValue::String(self.handle)),
            (
                field_name("description")?,
                optional_string_value(self.description.take()),
            ),
            (
                field_name("vendor")?,
                optional_string_value(self.vendor.take()),
            ),
            (
                field_name("product_type")?,
                optional_string_value(self.product_type.take()),
            ),
            (
                field_name("primary_category_id")?,
                self.primary_category_id
                    .map(IndexValue::Uuid)
                    .unwrap_or(IndexValue::Null),
            ),
            (
                field_name("channel_restricted")?,
                IndexValue::Boolean(channel_restricted),
            ),
            (
                field_name("allowed_channel_slugs")?,
                IndexValue::List(
                    self.allowed_channel_slugs
                        .into_iter()
                        .map(IndexValue::String)
                        .collect(),
                ),
            ),
            (
                field_name("variant_ids")?,
                IndexValue::List(
                    self.variant_ids
                        .iter()
                        .copied()
                        .map(IndexValue::Uuid)
                        .collect(),
                ),
            ),
        ]);
        let variant_schema = product_variant_v2_schema_ref()
            .map_err(ProductGraphIndexBridgeError::InvalidContract)?;
        let links = vec![IndexLinkValue {
            name: LinkName::new("variants")
                .map_err(ProductGraphIndexBridgeError::InvalidContract)?,
            targets: self
                .variant_ids
                .into_iter()
                .map(|variant_id| LinkedEntityKey {
                    schema: variant_schema.clone(),
                    entity_id: variant_id,
                    locale: None,
                })
                .collect(),
        }];
        Ok(IndexMutation::Upsert {
            event_id,
            record: IndexRecord {
                key: EntityKey {
                    tenant_id: self.tenant_id,
                    schema: product_v2_schema_ref()
                        .map_err(ProductGraphIndexBridgeError::InvalidContract)?,
                    entity_id: self.product_id,
                    locale: Some(self.locale),
                },
                source_version: self.source_version,
                fields,
                links,
            },
        })
    }
}

#[derive(Debug)]
struct ProductVariantV2Row {
    tenant_id: Uuid,
    variant_id: Uuid,
    product_id: Uuid,
    source_version: u64,
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

impl ProductVariantV2Row {
    fn decode(
        row: QueryResult,
        expected_tenant: Uuid,
    ) -> Result<Self, ProductGraphIndexBridgeError> {
        let tenant_id = row
            .try_get::<Uuid>("", "tenant_id")
            .map_err(|_| ProductGraphIndexBridgeError::InvalidRow)?;
        let variant_id = row
            .try_get::<Uuid>("", "variant_id")
            .map_err(|_| ProductGraphIndexBridgeError::InvalidRow)?;
        let product_id = row
            .try_get::<Uuid>("", "product_id")
            .map_err(|_| ProductGraphIndexBridgeError::InvalidRow)?;
        let revision = row
            .try_get::<i64>("", "index_revision")
            .map_err(|_| ProductGraphIndexBridgeError::InvalidRow)?;
        if tenant_id != expected_tenant
            || tenant_id.is_nil()
            || variant_id.is_nil()
            || product_id.is_nil()
            || revision <= 0
        {
            return Err(ProductGraphIndexBridgeError::InvalidRow);
        }
        Ok(Self {
            tenant_id,
            variant_id,
            product_id,
            source_version: u64::try_from(revision)
                .map_err(|_| ProductGraphIndexBridgeError::InvalidRow)?,
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
        })
    }

    fn cursor(&self) -> ProductVariantV2Cursor {
        ProductVariantV2Cursor {
            variant_id: self.variant_id,
        }
    }

    fn into_mutation(mut self) -> Result<IndexMutation, ProductGraphIndexBridgeError> {
        let event_id = derive_index_source_event_id(
            PRODUCT_VARIANT_V2_EVENT_DOMAIN,
            self.tenant_id,
            self.variant_id,
            None,
            self.source_version,
        )
        .map_err(|_| ProductGraphIndexBridgeError::InvalidRow)?;
        let fields = BTreeMap::from([
            (field_name("id")?, IndexValue::Uuid(self.variant_id)),
            (
                field_name("product_id")?,
                IndexValue::Uuid(self.product_id),
            ),
            (field_name("sku")?, optional_string_value(self.sku.take())),
            (
                field_name("barcode")?,
                optional_string_value(self.barcode.take()),
            ),
            (
                field_name("shipping_profile_slug")?,
                optional_string_value(self.shipping_profile_slug.take()),
            ),
            (field_name("ean")?, optional_string_value(self.ean.take())),
            (field_name("upc")?, optional_string_value(self.upc.take())),
            (
                field_name("inventory_policy")?,
                IndexValue::String(self.inventory_policy),
            ),
            (
                field_name("inventory_management")?,
                IndexValue::String(self.inventory_management),
            ),
            (
                field_name("inventory_quantity")?,
                IndexValue::Integer(self.inventory_quantity),
            ),
            (
                field_name("weight_unit")?,
                optional_string_value(self.weight_unit.take()),
            ),
            (
                field_name("option1")?,
                optional_string_value(self.option1.take()),
            ),
            (
                field_name("option2")?,
                optional_string_value(self.option2.take()),
            ),
            (
                field_name("option3")?,
                optional_string_value(self.option3.take()),
            ),
            (
                field_name("position")?,
                IndexValue::Integer(self.position),
            ),
        ]);
        Ok(IndexMutation::Upsert {
            event_id,
            record: IndexRecord {
                key: EntityKey {
                    tenant_id: self.tenant_id,
                    schema: product_variant_v2_schema_ref()
                        .map_err(ProductGraphIndexBridgeError::InvalidContract)?,
                    entity_id: self.variant_id,
                    locale: None,
                },
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

fn field_name(name: &str) -> Result<FieldName, ProductGraphIndexBridgeError> {
    FieldName::new(name).map_err(ProductGraphIndexBridgeError::InvalidContract)
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
    IndexSourceFailure::retryable(code).expect("static Product graph retry code must be valid")
}

fn permanent(code: &'static str) -> IndexSourceFailure {
    IndexSourceFailure::permanent(code).expect("static Product graph failure code must be valid")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn product_graph_schemas_are_version_two_and_link_product_to_variants() {
        let product = product_v2_schema().unwrap();
        let variant = product_variant_v2_schema().unwrap();
        assert_eq!(product.reference.version, SchemaVersion::new(2));
        assert_eq!(variant.reference.version, SchemaVersion::new(2));
        assert_eq!(product.locale_mode, LocaleMode::Required);
        assert_eq!(variant.locale_mode, LocaleMode::None);
        assert_eq!(product.fields.len(), 11);
        assert_eq!(variant.fields.len(), 15);
        assert_eq!(product.links.len(), 1);
        assert_eq!(product.links[0].name.as_str(), "variants");
        assert_eq!(product.links[0].target_schema, variant.reference.clone());
        assert_eq!(product.links[0].cardinality, LinkCardinality::Many);
        assert!(variant.links.is_empty());
        assert!(product.fingerprint().is_ok());
        assert!(variant.fingerprint().is_ok());
    }

    #[test]
    fn product_graph_channel_visibility_matches_storefront_normalization() {
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
    fn product_graph_cursors_reject_nil_noncanonical_and_unknown_fields() {
        for value in [
            serde_json::json!({"product_id": Uuid::nil(), "locale": "en-US"}),
            serde_json::json!({"product_id": Uuid::from_u128(2), "locale": "EN-us"}),
            serde_json::json!({"product_id": Uuid::from_u128(2), "locale": "en-US", "revision": 1}),
        ] {
            let cursor = IndexSourceCursor::new(value).unwrap();
            assert!(ProductV2Cursor::decode(&cursor).is_err());
        }
        for value in [
            serde_json::json!({"variant_id": Uuid::nil()}),
            serde_json::json!({"variant_id": Uuid::from_u128(2), "revision": 1}),
        ] {
            let cursor = IndexSourceCursor::new(value).unwrap();
            assert!(ProductVariantV2Cursor::decode(&cursor).is_err());
        }
    }

    #[test]
    fn product_graph_bridge_registers_two_schemas_and_one_factory() {
        let mut extensions = ModuleRuntimeExtensions::default();
        extensions.insert(rustok_product::ProductRuntimeSelected);
        extensions.insert(rustok_index::IndexSchemaSourceCatalog::new());
        extensions.insert(rustok_index::PostgresIndexSourceFactoryCatalog::new());
        register(&mut extensions).unwrap();

        let catalog = extensions
            .get::<rustok_index::IndexSchemaSourceCatalog>()
            .unwrap();
        assert!(catalog.get(&product_v2_schema_ref().unwrap()).is_some());
        assert!(
            catalog
                .get(&product_variant_v2_schema_ref().unwrap())
                .is_some()
        );
        let factories = extensions
            .get::<rustok_index::PostgresIndexSourceFactoryCatalog>()
            .unwrap();
        assert_eq!(factories.len(), 1);
        let factory = factories.iter().next().unwrap();
        assert_eq!(factory.owner_module(), "product");
        assert_eq!(factory.factory_name(), PRODUCT_GRAPH_INDEX_FACTORY);
    }
}
