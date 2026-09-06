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

pub(crate) const PRODUCT_VARIANT_INDEX_SOURCE: &str = "product-variant-postgres-primary";
const PRODUCT_VARIANT_EVENT_DOMAIN: &str = "rustok-product.product-variant-replay";

// Index core requires a numeric SchemaVersion. Only this one current ProductVariant contract is
// registered; there is no runtime compatibility branch for the removed historical contract.
const PRODUCT_VARIANT_SCHEMA_VERSION: u32 = 2;

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

#[derive(Debug, Error)]
enum ProductVariantIndexBridgeError {
    #[error("ProductVariant Index contract is invalid")]
    InvalidContract(#[source] DomainError),
    #[error("ProductVariant Index cursor is invalid")]
    InvalidCursor,
    #[error("ProductVariant Index source row is invalid")]
    InvalidRow,
}

pub(crate) fn register(extensions: &mut ModuleRuntimeExtensions) -> rustok_core::Result<()> {
    if !extensions.contains::<rustok_product::ProductRuntimeSelected>() {
        return Ok(());
    }

    let schema = product_variant_schema().map_err(|error| {
        rustok_core::Error::Validation(format!(
            "selected ProductVariant Index schema construction failed: {error}"
        ))
    })?;
    register_index_schema_source(extensions, "product", schema).map_err(|error| {
        rustok_core::Error::Validation(format!(
            "selected ProductVariant Index schema registration failed: {error}"
        ))
    })?;
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

fn product_variant_schema() -> Result<IndexSchema, ProductVariantIndexBridgeError> {
    let schema = IndexSchema {
        reference: product_variant_schema_ref()?,
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
        .map_err(ProductVariantIndexBridgeError::InvalidContract)?;
    Ok(schema)
}

fn product_variant_schema_ref() -> Result<SchemaRef, ProductVariantIndexBridgeError> {
    Ok(SchemaRef {
        module: ModuleName::new("rustok-product")
            .map_err(ProductVariantIndexBridgeError::InvalidContract)?,
        entity: EntityName::new("product_variant")
            .map_err(ProductVariantIndexBridgeError::InvalidContract)?,
        version: SchemaVersion::new(PRODUCT_VARIANT_SCHEMA_VERSION),
    })
}

fn scalar_field(
    name: &str,
    value_type: IndexValueType,
    nullable: bool,
    filterable: bool,
    sortable: bool,
) -> Result<IndexField, ProductVariantIndexBridgeError> {
    Ok(IndexField {
        name: FieldName::new(name).map_err(ProductVariantIndexBridgeError::InvalidContract)?,
        value_type,
        cardinality: FieldCardinality::One,
        nullable,
        selectable: true,
        filterable,
        sortable,
    })
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
            [product_variant_schema_ref().map_err(|error| error.to_string())?],
            ProductVariantPostgresIndexSource { db },
        )
        .map_err(|error| error.to_string())
    }
}

#[derive(Clone, Debug)]
struct ProductVariantPostgresIndexSource {
    db: DatabaseConnection,
}

impl ProductVariantPostgresIndexSource {
    fn validate_request(&self, schema: &SchemaRef) -> Result<(), IndexSourceFailure> {
        require_postgres(&self.db)?;
        match product_variant_schema_ref() {
            Ok(expected) if expected == *schema => Ok(()),
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
            .query_all_raw(Statement::from_sql_and_values(
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
            .query_all_raw(Statement::from_sql_and_values(
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
        self.validate_request(request.schema())?;
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
                    .into_mutation()
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
        self.validate_request(request.schema())?;
        let mutations = self
            .load_rows(&request)
            .await?
            .into_iter()
            .map(|row| {
                ProductVariantRow::decode(row, request.tenant_id())
                    .and_then(ProductVariantRow::into_mutation)
                    .map_err(|_| permanent("product_variant_index_record_invalid"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        IndexSourceLoadBatch::new(&request, mutations)
            .map_err(|_| permanent("product_variant_index_batch_invalid"))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProductVariantCursor {
    variant_id: Uuid,
}

impl ProductVariantCursor {
    fn decode(cursor: &IndexSourceCursor) -> Result<Self, ProductVariantIndexBridgeError> {
        let decoded: Self = serde_json::from_value(cursor.value().clone())
            .map_err(|_| ProductVariantIndexBridgeError::InvalidCursor)?;
        if decoded.variant_id.is_nil() {
            return Err(ProductVariantIndexBridgeError::InvalidCursor);
        }
        Ok(decoded)
    }

    fn encode(&self) -> Result<IndexSourceCursor, ProductVariantIndexBridgeError> {
        let value = serde_json::to_value(self)
            .map_err(|_| ProductVariantIndexBridgeError::InvalidCursor)?;
        IndexSourceCursor::new(value).map_err(|_| ProductVariantIndexBridgeError::InvalidCursor)
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
    ) -> Result<Self, ProductVariantIndexBridgeError> {
        let is_deleted = row
            .try_get::<bool>("", "is_deleted")
            .map_err(|_| ProductVariantIndexBridgeError::InvalidRow)?;
        let identity_count = row
            .try_get::<i64>("", "identity_count")
            .map_err(|_| ProductVariantIndexBridgeError::InvalidRow)?;
        let tenant_id = row
            .try_get::<Uuid>("", "tenant_id")
            .map_err(|_| ProductVariantIndexBridgeError::InvalidRow)?;
        let variant_id = row
            .try_get::<Uuid>("", "variant_id")
            .map_err(|_| ProductVariantIndexBridgeError::InvalidRow)?;
        let revision = row
            .try_get::<i64>("", "index_revision")
            .map_err(|_| ProductVariantIndexBridgeError::InvalidRow)?;
        if identity_count != 1
            || tenant_id != expected_tenant
            || tenant_id.is_nil()
            || variant_id.is_nil()
            || revision <= 0
        {
            return Err(ProductVariantIndexBridgeError::InvalidRow);
        }
        let source_version =
            u64::try_from(revision).map_err(|_| ProductVariantIndexBridgeError::InvalidRow)?;

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
            .map_err(|_| ProductVariantIndexBridgeError::InvalidRow)?;
        if product_id.is_nil() {
            return Err(ProductVariantIndexBridgeError::InvalidRow);
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
                        .map_err(|_| ProductVariantIndexBridgeError::InvalidRow)?,
                ),
                weight_unit: optional_string(&row, "weight_unit")?,
                option1: optional_string(&row, "option1")?,
                option2: optional_string(&row, "option2")?,
                option3: optional_string(&row, "option3")?,
                position: i64::from(
                    row.try_get::<i32>("", "position")
                        .map_err(|_| ProductVariantIndexBridgeError::InvalidRow)?,
                ),
            }),
        })
    }

    fn cursor(&self) -> ProductVariantCursor {
        ProductVariantCursor {
            variant_id: self.variant_id,
        }
    }

    fn into_mutation(self) -> Result<IndexMutation, ProductVariantIndexBridgeError> {
        let event_id = derive_index_source_event_id(
            PRODUCT_VARIANT_EVENT_DOMAIN,
            self.tenant_id,
            self.variant_id,
            None,
            self.source_version,
        )
        .map_err(|_| ProductVariantIndexBridgeError::InvalidRow)?;
        let key = EntityKey {
            tenant_id: self.tenant_id,
            schema: product_variant_schema_ref()?,
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

        let fields = BTreeMap::from([
            (field_name("id")?, IndexValue::Uuid(self.variant_id)),
            (field_name("product_id")?, IndexValue::Uuid(live.product_id)),
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
            (field_name("position")?, IndexValue::Integer(live.position)),
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

fn require_postgres(db: &DatabaseConnection) -> Result<(), IndexSourceFailure> {
    if db.get_database_backend() == DbBackend::Postgres {
        Ok(())
    } else {
        Err(permanent("product_variant_index_backend_unsupported"))
    }
}

fn field_name(name: &str) -> Result<FieldName, ProductVariantIndexBridgeError> {
    FieldName::new(name).map_err(ProductVariantIndexBridgeError::InvalidContract)
}

fn required_string(
    row: &QueryResult,
    column: &str,
) -> Result<String, ProductVariantIndexBridgeError> {
    let value = row
        .try_get::<String>("", column)
        .map_err(|_| ProductVariantIndexBridgeError::InvalidRow)?;
    if value.is_empty() {
        Err(ProductVariantIndexBridgeError::InvalidRow)
    } else {
        Ok(value)
    }
}

fn optional_string(
    row: &QueryResult,
    column: &str,
) -> Result<Option<String>, ProductVariantIndexBridgeError> {
    row.try_get::<Option<String>>("", column)
        .map_err(|_| ProductVariantIndexBridgeError::InvalidRow)
}

fn optional_string_value(value: Option<String>) -> IndexValue {
    value.map(IndexValue::String).unwrap_or(IndexValue::Null)
}

fn retryable(code: &'static str) -> IndexSourceFailure {
    IndexSourceFailure::retryable(code).expect("static ProductVariant source retry code is valid")
}

fn permanent(code: &'static str) -> IndexSourceFailure {
    IndexSourceFailure::permanent(code).expect("static ProductVariant source failure code is valid")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_product_variant_schema_contains_identity_once() {
        let schema = product_variant_schema().unwrap();
        assert_eq!(schema.reference, product_variant_schema_ref().unwrap());
        assert_eq!(schema.fields.len(), 15);
        assert!(
            schema
                .fields
                .iter()
                .any(|field| field.name.as_str() == "id")
        );
        assert!(schema.links.is_empty());
    }

    #[test]
    fn canonical_product_variant_registration_publishes_one_schema_and_one_factory() {
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
            factory.owner_module() == "product"
                && factory.factory_name() == PRODUCT_VARIANT_INDEX_SOURCE
        }));
    }

    #[test]
    fn retained_product_variant_row_emits_canonical_delete_mutation() {
        let variant = ProductVariantRow {
            tenant_id: Uuid::from_u128(1),
            variant_id: Uuid::from_u128(3),
            source_version: 10,
            state: ProductVariantRowState::Deleted,
        };
        let IndexMutation::Delete {
            key,
            source_version,
            ..
        } = variant.into_mutation().unwrap()
        else {
            panic!("retained ProductVariant row must emit a delete");
        };
        assert_eq!(key.schema, product_variant_schema_ref().unwrap());
        assert!(key.locale.is_none());
        assert_eq!(source_version, 10);
    }

    #[test]
    fn canonical_product_variant_cursor_rejects_nil_and_unknown_fields() {
        for value in [
            serde_json::json!({"variant_id": Uuid::nil()}),
            serde_json::json!({"variant_id": Uuid::from_u128(2), "revision": 1}),
        ] {
            let cursor = IndexSourceCursor::new(value).unwrap();
            assert!(ProductVariantCursor::decode(&cursor).is_err());
        }
    }
}
