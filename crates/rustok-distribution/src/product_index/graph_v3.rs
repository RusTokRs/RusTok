use std::collections::BTreeMap;

use rustok_index::{
    DomainError, EntityKey, EntityName, FieldCardinality, FieldName, IndexField, IndexLink,
    IndexLinkValue, IndexMutation, IndexRecord, IndexSchema, IndexSourceCursor, IndexSourceFailure,
    IndexSourceLoadBatch, IndexSourceLoadRequest, IndexSourcePage, IndexSourceScanRequest, IndexValue,
    IndexValueType, LinkCardinality, LinkName, LinkedEntityKey, LocaleKey, LocaleMode, ModuleName,
    SchemaRef, SchemaVersion, derive_index_source_event_id,
};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, QueryResult, Statement, Value};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use thiserror::Error;
use uuid::Uuid;

const PRODUCT_EVENT_DOMAIN_V3: &str = "rustok-product.product-replay-v3";

const PRODUCT_V3_ROWS_CTE: &str = r#"
product_v3_owner_versions AS (
    SELECT
        product.tenant_id,
        product.id AS product_id,
        product.index_revision AS product_source_version
    FROM products product
    WHERE product.tenant_id = $1

    UNION ALL

    SELECT
        tombstone.tenant_id,
        tombstone.product_id,
        MAX(tombstone.source_version) AS product_source_version
    FROM product_index_tombstones tombstone
    WHERE tombstone.tenant_id = $1
      AND NOT EXISTS (
          SELECT 1
          FROM products product
          WHERE product.tenant_id = tombstone.tenant_id
            AND product.id = tombstone.product_id
      )
    GROUP BY tombstone.tenant_id, tombstone.product_id
),
product_v3_projection AS (
    SELECT DISTINCT ON (projection.product_id)
        projection.tenant_id,
        projection.product_id,
        projection.projection_epoch,
        projection.product_source_version,
        projection.relation_epoch
    FROM product_index_graph_v3_projection_snapshots projection
    WHERE projection.tenant_id = $1
    ORDER BY projection.product_id ASC, projection.projection_epoch DESC
),
product_v3_relation AS (
    SELECT DISTINCT ON (relation.product_id)
        relation.tenant_id,
        relation.product_id,
        relation.relation_epoch,
        relation.channel_ids
    FROM product_sales_channel_index_relation_snapshots relation
    WHERE relation.tenant_id = $1
    ORDER BY relation.product_id ASC, relation.relation_epoch DESC
),
product_v3_union AS (
    SELECT
        FALSE AS is_deleted,
        product.tenant_id,
        product.id AS product_id,
        projection.projection_epoch,
        projection.product_source_version AS projected_product_source_version,
        owner_version.product_source_version AS observed_product_source_version,
        projection.relation_epoch AS projected_relation_epoch,
        relation.relation_epoch AS observed_relation_epoch,
        relation.channel_ids,
        product.status::text AS status,
        product.vendor,
        product.product_type,
        product.primary_category_id,
        product.metadata,
        translation.locale,
        translation.title,
        translation.handle,
        translation.description,
        COALESCE(
            (
                SELECT jsonb_agg(variant.id ORDER BY variant.id)
                FROM product_variants variant
                WHERE variant.tenant_id = product.tenant_id
                  AND variant.product_id = product.id
            ),
            '[]'::jsonb
        ) AS variant_ids
    FROM products product
    JOIN product_translations translation
      ON translation.product_id = product.id
     AND translation.tenant_id = product.tenant_id
    JOIN product_v3_owner_versions owner_version
      ON owner_version.tenant_id = product.tenant_id
     AND owner_version.product_id = product.id
    JOIN product_v3_projection projection
      ON projection.tenant_id = product.tenant_id
     AND projection.product_id = product.id
    JOIN product_v3_relation relation
      ON relation.tenant_id = product.tenant_id
     AND relation.product_id = product.id
    WHERE product.tenant_id = $1

    UNION ALL

    SELECT
        TRUE AS is_deleted,
        tombstone.tenant_id,
        tombstone.product_id,
        projection.projection_epoch,
        projection.product_source_version AS projected_product_source_version,
        owner_version.product_source_version AS observed_product_source_version,
        projection.relation_epoch AS projected_relation_epoch,
        relation.relation_epoch AS observed_relation_epoch,
        relation.channel_ids,
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
    JOIN product_v3_owner_versions owner_version
      ON owner_version.tenant_id = tombstone.tenant_id
     AND owner_version.product_id = tombstone.product_id
    JOIN product_v3_projection projection
      ON projection.tenant_id = tombstone.tenant_id
     AND projection.product_id = tombstone.product_id
    JOIN product_v3_relation relation
      ON relation.tenant_id = tombstone.tenant_id
     AND relation.product_id = tombstone.product_id
    WHERE tombstone.tenant_id = $1
),
product_v3_rows AS (
    SELECT
        row.*,
        COUNT(*) OVER (
            PARTITION BY row.tenant_id, row.product_id, row.locale
        ) AS identity_count
    FROM product_v3_union row
)
"#;

const PRODUCT_V3_ROW_SELECT: &str = r#"
SELECT
    row.is_deleted,
    row.identity_count,
    row.tenant_id,
    row.product_id,
    row.projection_epoch,
    row.projected_product_source_version,
    row.observed_product_source_version,
    row.projected_relation_epoch,
    row.observed_relation_epoch,
    row.channel_ids,
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
FROM product_v3_rows row
"#;

#[derive(Debug, Error)]
pub(super) enum ProductV3IndexBridgeError {
    #[error("Product v3 Index contract is invalid")]
    InvalidContract(#[source] DomainError),
    #[error("Product v3 Index cursor is invalid")]
    InvalidCursor,
    #[error("Product v3 Index source row is invalid")]
    InvalidRow,
}

pub(super) fn product_v3_schema() -> Result<IndexSchema, ProductV3IndexBridgeError> {
    let schema = IndexSchema {
        reference: product_v3_schema_ref()?,
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
            many_field("sales_channel_ids", IndexValueType::Uuid, true)?,
        ],
        links: vec![
            IndexLink {
                name: link_name("variants")?,
                source_fields: vec![field_name("variant_ids")?],
                target_schema: product_variant_v2_schema_ref()?,
                target_fields: vec![field_name("id")?],
                cardinality: LinkCardinality::Many,
            },
            IndexLink {
                name: link_name("sales_channels")?,
                source_fields: vec![field_name("sales_channel_ids")?],
                target_schema: sales_channel_v1_schema_ref()?,
                target_fields: vec![field_name("id")?],
                cardinality: LinkCardinality::Many,
            },
        ],
    };
    schema
        .validate()
        .map_err(ProductV3IndexBridgeError::InvalidContract)?;
    Ok(schema)
}

pub(super) async fn scan_product_v3(
    db: &DatabaseConnection,
    request: IndexSourceScanRequest,
) -> Result<IndexSourcePage, IndexSourceFailure> {
    validate_request(db, request.schema())?;
    let cursor = request
        .cursor()
        .map(ProductV3Cursor::decode)
        .transpose()
        .map_err(|_| permanent("product_index_v3_cursor_invalid"))?;
    let rows = scan_rows(db, &request, cursor.as_ref()).await?;
    let has_more = rows.len() > request.limit();
    let mut mutations = Vec::with_capacity(rows.len().min(request.limit()));
    let mut next_cursor = None;
    for row in rows.into_iter().take(request.limit()) {
        let decoded = ProductV3Row::decode(row, request.tenant_id())
            .map_err(|_| permanent("product_index_v3_record_invalid"))?;
        if has_more {
            next_cursor = Some(
                decoded
                    .cursor()
                    .encode()
                    .map_err(|_| permanent("product_index_v3_cursor_invalid"))?,
            );
        }
        mutations.push(
            decoded
                .into_mutation()
                .map_err(|_| permanent("product_index_v3_record_invalid"))?,
        );
    }
    IndexSourcePage::new(&request, mutations, next_cursor)
        .map_err(|_| permanent("product_index_v3_page_invalid"))
}

pub(super) async fn load_product_v3(
    db: &DatabaseConnection,
    request: IndexSourceLoadRequest,
) -> Result<IndexSourceLoadBatch, IndexSourceFailure> {
    validate_request(db, request.schema())?;
    let rows = load_rows(db, &request).await?;
    let mutations = rows
        .into_iter()
        .map(|row| {
            ProductV3Row::decode(row, request.tenant_id())
                .and_then(ProductV3Row::into_mutation)
                .map_err(|_| permanent("product_index_v3_record_invalid"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    IndexSourceLoadBatch::new(&request, mutations)
        .map_err(|_| permanent("product_index_v3_batch_invalid"))
}

async fn scan_rows(
    db: &DatabaseConnection,
    request: &IndexSourceScanRequest,
    cursor: Option<&ProductV3Cursor>,
) -> Result<Vec<QueryResult>, IndexSourceFailure> {
    let fetch_limit = i64::try_from(request.limit() + 1)
        .expect("Index source page limit is bounded below i64::MAX");
    let (sql, values): (String, Vec<Value>) = match cursor {
        Some(cursor) => (
            format!(
                "WITH {PRODUCT_V3_ROWS_CTE}\n{PRODUCT_V3_ROW_SELECT}\nWHERE (row.product_id, row.locale) > ($2, $3)\nORDER BY row.product_id ASC, row.locale ASC\nLIMIT $4"
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
                "WITH {PRODUCT_V3_ROWS_CTE}\n{PRODUCT_V3_ROW_SELECT}\nORDER BY row.product_id ASC, row.locale ASC\nLIMIT $2"
            ),
            vec![request.tenant_id().into(), fetch_limit.into()],
        ),
    };
    db.query_all(Statement::from_sql_and_values(
        DbBackend::Postgres,
        sql,
        values,
    ))
    .await
    .map_err(|_| retryable("product_index_v3_storage_unavailable"))
}

async fn load_rows(
    db: &DatabaseConnection,
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
            .ok_or_else(|| permanent("product_index_v3_locale_required"))?;
        tuples.push(format!("(${parameter}::uuid, ${}::text)", parameter + 1));
        values.push(key.entity_id.into());
        values.push(locale.as_str().to_owned().into());
        parameter += 2;
    }
    let sql = format!(
        "WITH requested(product_id, locale) AS (VALUES {}),\n{PRODUCT_V3_ROWS_CTE}\n{PRODUCT_V3_ROW_SELECT}\nJOIN requested requested_key ON requested_key.product_id = row.product_id AND requested_key.locale = row.locale\nORDER BY row.product_id ASC, row.locale ASC",
        tuples.join(", ")
    );
    db.query_all(Statement::from_sql_and_values(
        DbBackend::Postgres,
        sql,
        values,
    ))
    .await
    .map_err(|_| retryable("product_index_v3_storage_unavailable"))
}

fn validate_request(
    db: &DatabaseConnection,
    schema: &SchemaRef,
) -> Result<(), IndexSourceFailure> {
    if db.get_database_backend() != DbBackend::Postgres {
        return Err(permanent("product_index_v3_backend_unsupported"));
    }
    match product_v3_schema_ref() {
        Ok(expected) if expected == *schema => Ok(()),
        _ => Err(permanent("product_index_v3_schema_mismatch")),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProductV3Cursor {
    product_id: Uuid,
    locale: String,
}

impl ProductV3Cursor {
    fn decode(cursor: &IndexSourceCursor) -> Result<Self, ProductV3IndexBridgeError> {
        let decoded: Self = serde_json::from_value(cursor.value().clone())
            .map_err(|_| ProductV3IndexBridgeError::InvalidCursor)?;
        if decoded.product_id.is_nil() {
            return Err(ProductV3IndexBridgeError::InvalidCursor);
        }
        let canonical = LocaleKey::new(&decoded.locale)
            .map_err(ProductV3IndexBridgeError::InvalidContract)?;
        if canonical.as_str() != decoded.locale {
            return Err(ProductV3IndexBridgeError::InvalidCursor);
        }
        Ok(decoded)
    }

    fn encode(&self) -> Result<IndexSourceCursor, ProductV3IndexBridgeError> {
        let value = serde_json::to_value(self)
            .map_err(|_| ProductV3IndexBridgeError::InvalidCursor)?;
        IndexSourceCursor::new(value).map_err(|_| ProductV3IndexBridgeError::InvalidCursor)
    }
}

#[derive(Debug)]
struct ProductV3Row {
    tenant_id: Uuid,
    product_id: Uuid,
    source_version: u64,
    locale: LocaleKey,
    state: ProductV3RowState,
}

#[derive(Debug)]
enum ProductV3RowState {
    Live(ProductV3LiveFields),
    Deleted,
}

#[derive(Debug)]
struct ProductV3LiveFields {
    status: String,
    title: String,
    handle: String,
    description: Option<String>,
    vendor: Option<String>,
    product_type: Option<String>,
    primary_category_id: Option<Uuid>,
    allowed_channel_slugs: Vec<String>,
    variant_ids: Vec<Uuid>,
    sales_channel_ids: Vec<Uuid>,
}

impl ProductV3Row {
    fn decode(
        row: QueryResult,
        expected_tenant: Uuid,
    ) -> Result<Self, ProductV3IndexBridgeError> {
        let is_deleted = row
            .try_get::<bool>("", "is_deleted")
            .map_err(|_| ProductV3IndexBridgeError::InvalidRow)?;
        let identity_count = row
            .try_get::<i64>("", "identity_count")
            .map_err(|_| ProductV3IndexBridgeError::InvalidRow)?;
        let tenant_id = row
            .try_get::<Uuid>("", "tenant_id")
            .map_err(|_| ProductV3IndexBridgeError::InvalidRow)?;
        let product_id = row
            .try_get::<Uuid>("", "product_id")
            .map_err(|_| ProductV3IndexBridgeError::InvalidRow)?;
        let projection_epoch = row
            .try_get::<i64>("", "projection_epoch")
            .map_err(|_| ProductV3IndexBridgeError::InvalidRow)?;
        let projected_product_source_version = row
            .try_get::<i64>("", "projected_product_source_version")
            .map_err(|_| ProductV3IndexBridgeError::InvalidRow)?;
        let observed_product_source_version = row
            .try_get::<i64>("", "observed_product_source_version")
            .map_err(|_| ProductV3IndexBridgeError::InvalidRow)?;
        let projected_relation_epoch = row
            .try_get::<i64>("", "projected_relation_epoch")
            .map_err(|_| ProductV3IndexBridgeError::InvalidRow)?;
        let observed_relation_epoch = row
            .try_get::<i64>("", "observed_relation_epoch")
            .map_err(|_| ProductV3IndexBridgeError::InvalidRow)?;
        let raw_locale = row
            .try_get::<String>("", "locale")
            .map_err(|_| ProductV3IndexBridgeError::InvalidRow)?;

        if identity_count != 1
            || tenant_id != expected_tenant
            || tenant_id.is_nil()
            || product_id.is_nil()
            || projection_epoch <= 0
            || projected_product_source_version <= 0
            || projected_relation_epoch <= 0
            || projected_product_source_version != observed_product_source_version
            || projected_relation_epoch != observed_relation_epoch
        {
            return Err(ProductV3IndexBridgeError::InvalidRow);
        }
        let locale = LocaleKey::new(&raw_locale)
            .map_err(ProductV3IndexBridgeError::InvalidContract)?;
        if locale.as_str() != raw_locale {
            return Err(ProductV3IndexBridgeError::InvalidRow);
        }
        let source_version =
            u64::try_from(projection_epoch).map_err(|_| ProductV3IndexBridgeError::InvalidRow)?;

        if is_deleted {
            return Ok(Self {
                tenant_id,
                product_id,
                source_version,
                locale,
                state: ProductV3RowState::Deleted,
            });
        }

        let primary_category_id = row
            .try_get::<Option<Uuid>>("", "primary_category_id")
            .map_err(|_| ProductV3IndexBridgeError::InvalidRow)?;
        if primary_category_id.is_some_and(|id| id.is_nil()) {
            return Err(ProductV3IndexBridgeError::InvalidRow);
        }
        let metadata = row
            .try_get::<JsonValue>("", "metadata")
            .map_err(|_| ProductV3IndexBridgeError::InvalidRow)?;
        Ok(Self {
            tenant_id,
            product_id,
            source_version,
            locale,
            state: ProductV3RowState::Live(ProductV3LiveFields {
                status: required_string(&row, "status")?,
                title: required_string(&row, "title")?,
                handle: required_string(&row, "handle")?,
                description: optional_string(&row, "description")?,
                vendor: optional_string(&row, "vendor")?,
                product_type: optional_string(&row, "product_type")?,
                primary_category_id,
                allowed_channel_slugs: extract_allowed_channel_slugs(&metadata),
                variant_ids: decode_canonical_uuid_json_list(&row, "variant_ids", None)?,
                sales_channel_ids: decode_canonical_uuid_json_list(
                    &row,
                    "channel_ids",
                    Some(rustok_product::MAX_PRODUCT_SALES_CHANNEL_RELATION_CHANNELS),
                )?,
            }),
        })
    }

    fn cursor(&self) -> ProductV3Cursor {
        ProductV3Cursor {
            product_id: self.product_id,
            locale: self.locale.as_str().to_owned(),
        }
    }

    fn into_mutation(self) -> Result<IndexMutation, ProductV3IndexBridgeError> {
        let event_id = derive_index_source_event_id(
            PRODUCT_EVENT_DOMAIN_V3,
            self.tenant_id,
            self.product_id,
            Some(&self.locale),
            self.source_version,
        )
        .map_err(|_| ProductV3IndexBridgeError::InvalidRow)?;
        let key = EntityKey {
            tenant_id: self.tenant_id,
            schema: product_v3_schema_ref()?,
            entity_id: self.product_id,
            locale: Some(self.locale),
        };

        let ProductV3RowState::Live(mut live) = self.state else {
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
                field_name("channel_restricted")?,
                IndexValue::Boolean(!live.allowed_channel_slugs.is_empty()),
            ),
            (
                field_name("allowed_channel_slugs")?,
                IndexValue::List(
                    live.allowed_channel_slugs
                        .into_iter()
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
        let variant_schema = product_variant_v2_schema_ref()?;
        let sales_channel_schema = sales_channel_v1_schema_ref()?;
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

fn product_v3_schema_ref() -> Result<SchemaRef, ProductV3IndexBridgeError> {
    Ok(SchemaRef {
        module: ModuleName::new("rustok-product")
            .map_err(ProductV3IndexBridgeError::InvalidContract)?,
        entity: EntityName::new("product")
            .map_err(ProductV3IndexBridgeError::InvalidContract)?,
        version: SchemaVersion::new(3),
    })
}

fn product_variant_v2_schema_ref() -> Result<SchemaRef, ProductV3IndexBridgeError> {
    Ok(SchemaRef {
        module: ModuleName::new("rustok-product")
            .map_err(ProductV3IndexBridgeError::InvalidContract)?,
        entity: EntityName::new("product_variant")
            .map_err(ProductV3IndexBridgeError::InvalidContract)?,
        version: SchemaVersion::new(2),
    })
}

fn sales_channel_v1_schema_ref() -> Result<SchemaRef, ProductV3IndexBridgeError> {
    Ok(SchemaRef {
        module: ModuleName::new("rustok-channel")
            .map_err(ProductV3IndexBridgeError::InvalidContract)?,
        entity: EntityName::new("sales_channel")
            .map_err(ProductV3IndexBridgeError::InvalidContract)?,
        version: SchemaVersion::INITIAL,
    })
}

fn scalar_field(
    name: &str,
    value_type: IndexValueType,
    nullable: bool,
    filterable: bool,
    sortable: bool,
) -> Result<IndexField, ProductV3IndexBridgeError> {
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
) -> Result<IndexField, ProductV3IndexBridgeError> {
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

fn field_name(name: &str) -> Result<FieldName, ProductV3IndexBridgeError> {
    FieldName::new(name).map_err(ProductV3IndexBridgeError::InvalidContract)
}

fn link_name(name: &str) -> Result<LinkName, ProductV3IndexBridgeError> {
    LinkName::new(name).map_err(ProductV3IndexBridgeError::InvalidContract)
}

fn required_string(
    row: &QueryResult,
    column: &str,
) -> Result<String, ProductV3IndexBridgeError> {
    let value = row
        .try_get::<String>("", column)
        .map_err(|_| ProductV3IndexBridgeError::InvalidRow)?;
    if value.is_empty() {
        Err(ProductV3IndexBridgeError::InvalidRow)
    } else {
        Ok(value)
    }
}

fn optional_string(
    row: &QueryResult,
    column: &str,
) -> Result<Option<String>, ProductV3IndexBridgeError> {
    row.try_get::<Option<String>>("", column)
        .map_err(|_| ProductV3IndexBridgeError::InvalidRow)
}

fn optional_string_value(value: Option<String>) -> IndexValue {
    value.map(IndexValue::String).unwrap_or(IndexValue::Null)
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

    let mut normalized = std::collections::BTreeSet::new();
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

fn decode_canonical_uuid_json_list(
    row: &QueryResult,
    column: &str,
    max: Option<usize>,
) -> Result<Vec<Uuid>, ProductV3IndexBridgeError> {
    let value = row
        .try_get::<JsonValue>("", column)
        .map_err(|_| ProductV3IndexBridgeError::InvalidRow)?;
    let values = value
        .as_array()
        .ok_or(ProductV3IndexBridgeError::InvalidRow)?;
    if max.is_some_and(|max| values.len() > max) {
        return Err(ProductV3IndexBridgeError::InvalidRow);
    }

    let mut decoded = Vec::with_capacity(values.len());
    for value in values {
        let raw = value
            .as_str()
            .ok_or(ProductV3IndexBridgeError::InvalidRow)?;
        let id = Uuid::parse_str(raw).map_err(|_| ProductV3IndexBridgeError::InvalidRow)?;
        if id.is_nil()
            || decoded
                .last()
                .is_some_and(|previous| id <= *previous)
        {
            return Err(ProductV3IndexBridgeError::InvalidRow);
        }
        decoded.push(id);
    }
    Ok(decoded)
}

fn retryable(code: &'static str) -> IndexSourceFailure {
    IndexSourceFailure::retryable(code).expect("static Product v3 source retry code must be valid")
}

fn permanent(code: &'static str) -> IndexSourceFailure {
    IndexSourceFailure::permanent(code).expect("static Product v3 source failure code must be valid")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn product_v3_adds_sales_channel_link_without_mutating_v2_shape() {
        let schema = product_v3_schema().unwrap();
        assert_eq!(schema.reference.version, SchemaVersion::new(3));
        assert_eq!(schema.locale_mode, LocaleMode::Required);
        assert_eq!(schema.fields.len(), 12);
        assert_eq!(schema.links.len(), 2);
        assert_eq!(schema.links[0].name.as_str(), "variants");
        assert_eq!(schema.links[1].name.as_str(), "sales_channels");
        assert_eq!(schema.links[1].target_schema, sales_channel_v1_schema_ref().unwrap());
        assert_eq!(schema.links[1].cardinality, LinkCardinality::Many);
    }

    #[test]
    fn product_v3_source_uses_projection_epoch_and_owner_relation_snapshot_only() {
        assert!(PRODUCT_V3_ROWS_CTE.contains("product_index_graph_v3_projection_snapshots"));
        assert!(PRODUCT_V3_ROWS_CTE.contains("product_sales_channel_index_relation_snapshots"));
        assert!(PRODUCT_V3_ROWS_CTE.contains("projection.projection_epoch"));
        assert!(PRODUCT_V3_ROWS_CTE.contains("projection.product_source_version"));
        assert!(PRODUCT_V3_ROWS_CTE.contains("projection.relation_epoch"));
        assert!(!PRODUCT_V3_ROWS_CTE.contains("FROM channels"));
        assert!(!PRODUCT_V3_ROWS_CTE.contains("JOIN channels"));
    }

    #[test]
    fn v3_cursor_rejects_nil_noncanonical_and_unknown_fields() {
        for value in [
            serde_json::json!({"product_id": Uuid::nil(), "locale": "en-US"}),
            serde_json::json!({"product_id": Uuid::from_u128(2), "locale": "EN-us"}),
            serde_json::json!({"product_id": Uuid::from_u128(2), "locale": "en-US", "projection_epoch": 1}),
        ] {
            let cursor = IndexSourceCursor::new(value).unwrap();
            assert!(ProductV3Cursor::decode(&cursor).is_err());
        }
    }
}
