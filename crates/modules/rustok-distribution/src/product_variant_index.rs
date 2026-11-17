use std::{collections::BTreeMap, sync::Arc, time::Duration};

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
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use rustok_api::{PortActor, PortContext, PortError};
use rustok_product::{
    PRODUCT_VARIANT_REPLAY_READ_CLAIM, PostgresProductVariantReplay, ProductVariantReplayEntry,
    ProductVariantReplayPort, ProductVariantReplayScan, ProductVariantReplayState,
};

#[cfg(test)]
#[path = "product_index/variant_replay_postgres_tests.rs"]
mod variant_replay_postgres_tests;

pub(crate) const PRODUCT_VARIANT_INDEX_SOURCE: &str = "product-variant-postgres-primary";
const PRODUCT_VARIANT_EVENT_DOMAIN: &str = "rustok-product.product-variant-replay";

// Index core requires a numeric SchemaVersion. Only this one current ProductVariant contract is
// registered; there is no runtime compatibility branch for the removed historical contract.
const PRODUCT_VARIANT_SCHEMA_VERSION: u32 = 2;

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
        ProductVariantIndexSourceFactory,
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
            scalar_field(
                "combination_identity",
                IndexValueType::String,
                true,
                true,
                false,
            )?,
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
struct ProductVariantIndexSourceFactory;

impl PostgresIndexSourceFactory for ProductVariantIndexSourceFactory {
    fn register_source(
        &self,
        extensions: &mut ModuleRuntimeExtensions,
        db: DatabaseConnection,
    ) -> Result<(), String> {
        let port = extensions
            .get::<Arc<dyn ProductVariantReplayPort>>()
            .cloned()
            .unwrap_or_else(|| Arc::new(PostgresProductVariantReplay::new(db)));
        register_index_source(
            extensions,
            "product",
            PRODUCT_VARIANT_INDEX_SOURCE,
            [product_variant_schema_ref().map_err(|error| error.to_string())?],
            ProductVariantIndexSource { port },
        )
        .map_err(|error| error.to_string())
    }
}

struct ProductVariantIndexSource {
    port: Arc<dyn ProductVariantReplayPort>,
}

impl ProductVariantIndexSource {
    fn validate_request(schema: &SchemaRef) -> Result<(), IndexSourceFailure> {
        match product_variant_schema_ref() {
            Ok(expected) if expected == *schema => Ok(()),
            _ => Err(permanent("product_variant_index_schema_mismatch")),
        }
    }
}

fn replay_context(tenant_id: Uuid) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::service("product-index"),
        "und",
        Uuid::new_v4().to_string(),
    )
    .with_claim(PRODUCT_VARIANT_REPLAY_READ_CLAIM)
    .with_deadline(Duration::from_secs(10))
}

fn map_port_error(error: PortError) -> IndexSourceFailure {
    if error.retryable {
        retryable("product_variant_index_storage_unavailable")
    } else {
        permanent("product_variant_index_replay_failed")
    }
}

#[async_trait]
impl IndexSource for ProductVariantIndexSource {
    async fn scan(
        &self,
        request: IndexSourceScanRequest,
    ) -> Result<IndexSourcePage, IndexSourceFailure> {
        Self::validate_request(request.schema())?;
        if request.locale().is_some() {
            return Err(permanent("product_variant_index_locale_forbidden"));
        }
        let after_variant_id = request
            .cursor()
            .map(ProductVariantCursor::decode)
            .transpose()
            .map_err(|_| permanent("product_variant_index_cursor_invalid"))?
            .map(|cursor| cursor.variant_id);
        let page = self
            .port
            .scan_variant_replay(
                replay_context(request.tenant_id()),
                ProductVariantReplayScan {
                    after_variant_id,
                    limit: request.limit(),
                },
            )
            .await
            .map_err(map_port_error)?;
        let mut previous = after_variant_id;
        for entry in &page.entries {
            if previous.is_some_and(|id| id >= entry.variant_id) {
                return Err(permanent("product_variant_index_page_invalid"));
            }
            previous = Some(entry.variant_id);
        }
        if let Some(next) = page.next_after_variant_id
            && (previous != Some(next) || page.entries.len() != request.limit())
        {
            return Err(permanent("product_variant_index_page_invalid"));
        }
        let next_cursor = page
            .next_after_variant_id
            .map(|variant_id| ProductVariantCursor { variant_id }.encode())
            .transpose()
            .map_err(|_| permanent("product_variant_index_cursor_invalid"))?;
        let mutations = page
            .entries
            .into_iter()
            .map(|entry| into_mutation(entry, request.tenant_id()))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| permanent("product_variant_index_record_invalid"))?;
        IndexSourcePage::new(&request, mutations, next_cursor)
            .map_err(|_| permanent("product_variant_index_page_invalid"))
    }

    async fn load(
        &self,
        request: IndexSourceLoadRequest,
    ) -> Result<IndexSourceLoadBatch, IndexSourceFailure> {
        Self::validate_request(request.schema())?;
        if request.keys().iter().any(|key| key.locale.is_some()) {
            return Err(permanent("product_variant_index_locale_forbidden"));
        }
        let entries = self
            .port
            .load_variant_replay(
                replay_context(request.tenant_id()),
                request.keys().iter().map(|key| key.entity_id).collect(),
            )
            .await
            .map_err(map_port_error)?;
        let mutations = entries
            .into_iter()
            .map(|entry| into_mutation(entry, request.tenant_id()))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| permanent("product_variant_index_record_invalid"))?;
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

fn into_mutation(
    entry: ProductVariantReplayEntry,
    expected_tenant: Uuid,
) -> Result<IndexMutation, ProductVariantIndexBridgeError> {
    if entry.tenant_id != expected_tenant || entry.variant_id.is_nil() || entry.source_version == 0
    {
        return Err(ProductVariantIndexBridgeError::InvalidRow);
    }
    let event_id = derive_index_source_event_id(
        PRODUCT_VARIANT_EVENT_DOMAIN,
        entry.tenant_id,
        entry.variant_id,
        None,
        entry.source_version,
    )
    .map_err(|_| ProductVariantIndexBridgeError::InvalidRow)?;
    let key = EntityKey {
        tenant_id: entry.tenant_id,
        schema: product_variant_schema_ref()?,
        entity_id: entry.variant_id,
        locale: None,
    };

    let ProductVariantReplayState::Live(live) = entry.state else {
        return Ok(IndexMutation::Delete {
            event_id,
            key,
            source_version: entry.source_version,
        });
    };

    if live.product_id.is_nil()
        || live.inventory_policy.is_empty()
        || live.inventory_management.is_empty()
    {
        return Err(ProductVariantIndexBridgeError::InvalidRow);
    }
    let fields = BTreeMap::from([
        (field_name("id")?, IndexValue::Uuid(entry.variant_id)),
        (field_name("product_id")?, IndexValue::Uuid(live.product_id)),
        (field_name("sku")?, optional_string_value(live.sku)),
        (field_name("barcode")?, optional_string_value(live.barcode)),
        (
            field_name("shipping_profile_slug")?,
            optional_string_value(live.shipping_profile_slug),
        ),
        (field_name("ean")?, optional_string_value(live.ean)),
        (field_name("upc")?, optional_string_value(live.upc)),
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
            IndexValue::Integer(i64::from(live.inventory_quantity)),
        ),
        (
            field_name("weight_unit")?,
            optional_string_value(live.weight_unit),
        ),
        (
            field_name("combination_identity")?,
            optional_string_value(live.combination_identity),
        ),
        (
            field_name("position")?,
            IndexValue::Integer(i64::from(live.position)),
        ),
    ]);
    Ok(IndexMutation::Upsert {
        event_id,
        record: IndexRecord {
            key,
            source_version: entry.source_version,
            fields,
            links: Vec::new(),
        },
    })
}

fn field_name(name: &str) -> Result<FieldName, ProductVariantIndexBridgeError> {
    FieldName::new(name).map_err(ProductVariantIndexBridgeError::InvalidContract)
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
    fn canonical_product_variant_schema_contains_identity_once() {
        let schema = product_variant_schema().unwrap();
        assert_eq!(schema.reference, product_variant_schema_ref().unwrap());
        assert_eq!(schema.fields.len(), 13);
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
        let variant = ProductVariantReplayEntry {
            tenant_id: Uuid::from_u128(1),
            variant_id: Uuid::from_u128(3),
            source_version: 10,
            state: ProductVariantReplayState::Deleted,
        };
        let IndexMutation::Delete {
            key,
            source_version,
            ..
        } = into_mutation(variant, Uuid::from_u128(1)).unwrap()
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
