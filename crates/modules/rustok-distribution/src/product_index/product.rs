use std::{collections::BTreeMap, sync::Arc, time::Duration};

use async_trait::async_trait;
use rustok_api::{PortActor, PortContext, PortError};
use rustok_core::ModuleRuntimeExtensions;
use rustok_index::{
    DomainError, EntityKey, EntityName, FieldCardinality, FieldName, IndexField, IndexLink,
    IndexLinkValue, IndexMutation, IndexRecord, IndexSchema, IndexSource, IndexSourceCursor,
    IndexSourceFailure, IndexSourceLoadBatch, IndexSourceLoadRequest, IndexSourcePage,
    IndexSourceScanRequest, IndexValue, IndexValueType, LinkCardinality, LinkName, LinkedEntityKey,
    LocaleKey, LocaleMode, ModuleName, PostgresIndexSourceFactory, IndexSourceRegistrationError,
    SchemaRef, SchemaVersion, derive_index_schema_source_event_id, register_index_schema_source, register_index_source,
    register_postgres_index_source_factory,
};
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use rustok_product::{
    PRODUCT_LOCALE_REPLAY_FRESHNESS_PENDING, PRODUCT_LOCALE_REPLAY_READ_CLAIM,
    PostgresProductLocaleReplay, ProductLocaleReplayEntry, ProductLocaleReplayKey,
    ProductLocaleReplayPort, ProductLocaleReplayScan, ProductLocaleReplayState,
};

use super::PRODUCT_SCHEMA_ROUTING_KEY;

pub(crate) const PRODUCT_INDEX_SOURCE: &str = "product-postgres-primary";
const PRODUCT_EVENT_DOMAIN: &str = "rustok-product.product-replay";

#[derive(Debug, Error)]
enum ProductIndexBridgeError {
    #[error("Product Index contract is invalid")]
    InvalidContract(#[source] DomainError),
    #[error("Product Index cursor is invalid")]
    InvalidCursor,
    #[error("Product Index source row is invalid")]
    InvalidRow,
}

impl From<DomainError> for ProductIndexBridgeError {
    fn from(error: DomainError) -> Self {
        Self::InvalidContract(error)
    }
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

fn product_schema_ref() -> Result<SchemaRef, DomainError> {
    Ok(SchemaRef {
        module: ModuleName::new("rustok-product")?,
        entity: EntityName::new("product")?,
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
    ) -> Result<(), IndexSourceRegistrationError> {
        let port = extensions
            .get::<Arc<dyn ProductLocaleReplayPort>>()
            .cloned()
            .unwrap_or_else(|| {
                Arc::new(PostgresProductLocaleReplay::embedded(
                    db,
                    rustok_channel::index_projection::POSTGRES_IDENTITY_GENERATIONS,
                ))
            });
        register_index_source(
            extensions,
            "product",
            PRODUCT_INDEX_SOURCE,
            [product_schema_ref()?],
            ProductPostgresIndexSource { port },
        )?;
        Ok(())
    }
}

struct ProductPostgresIndexSource {
    port: Arc<dyn ProductLocaleReplayPort>,
}

fn validate_schema(schema: &SchemaRef) -> Result<(), IndexSourceFailure> {
    match product_schema_ref() {
        Ok(expected) if expected == *schema => Ok(()),
        _ => Err(permanent("product_index_schema_mismatch")),
    }
}

fn replay_context(tenant_id: Uuid) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::service("product-index"),
        "und",
        Uuid::new_v4().to_string(),
    )
    .with_claim(PRODUCT_LOCALE_REPLAY_READ_CLAIM)
    .with_deadline(Duration::from_secs(10))
}

fn map_port_error(error: PortError) -> IndexSourceFailure {
    if error.code == PRODUCT_LOCALE_REPLAY_FRESHNESS_PENDING {
        return retryable(PRODUCT_LOCALE_REPLAY_FRESHNESS_PENDING);
    }
    if error.retryable {
        retryable("product_index_storage_unavailable")
    } else {
        permanent("product_index_record_invalid")
    }
}

#[async_trait]
impl IndexSource for ProductPostgresIndexSource {
    async fn scan(
        &self,
        request: IndexSourceScanRequest,
    ) -> Result<IndexSourcePage, IndexSourceFailure> {
        validate_schema(request.schema())?;
        let after = request
            .cursor()
            .map(ProductCursor::decode)
            .transpose()
            .map_err(|_| permanent("product_index_cursor_invalid"))?
            .map(|cursor| ProductLocaleReplayKey {
                product_id: cursor.product_id,
                locale: cursor.locale,
            });
        let locale = request.locale().map(|locale| locale.as_str().to_owned());
        if let (Some(locale), Some(after)) = (&locale, &after)
            && *locale != after.locale
        {
            return Err(permanent("product_index_cursor_invalid"));
        }
        let page = self
            .port
            .scan_locale_replay(
                replay_context(request.tenant_id()),
                ProductLocaleReplayScan {
                    locale,
                    after,
                    limit: request.limit(),
                },
            )
            .await
            .map_err(map_port_error)?;
        if let Some(next) = &page.next
            && (page
                .entries
                .last()
                .map(ProductLocaleReplayEntry::key)
                .as_ref()
                != Some(next))
        {
            return Err(permanent("product_index_page_invalid"));
        }
        let next_cursor = page
            .next
            .map(|key| {
                ProductCursor {
                    product_id: key.product_id,
                    locale: key.locale,
                }
                .encode()
            })
            .transpose()
            .map_err(|_| permanent("product_index_cursor_invalid"))?;
        let mutations = map_rows(page.entries, request.tenant_id())?;
        IndexSourcePage::new(&request, mutations, next_cursor)
            .map_err(|_| permanent("product_index_page_invalid"))
    }

    async fn load(
        &self,
        request: IndexSourceLoadRequest,
    ) -> Result<IndexSourceLoadBatch, IndexSourceFailure> {
        validate_schema(request.schema())?;
        let keys = request
            .keys()
            .iter()
            .map(|key| {
                let locale = key
                    .locale
                    .as_ref()
                    .ok_or_else(|| permanent("product_index_locale_required"))?;
                Ok(ProductLocaleReplayKey {
                    product_id: key.entity_id,
                    locale: locale.as_str().to_owned(),
                })
            })
            .collect::<Result<Vec<_>, IndexSourceFailure>>()?;
        let entries = self
            .port
            .load_locale_replay(replay_context(request.tenant_id()), keys)
            .await
            .map_err(map_port_error)?;
        let entries = entries
            .into_iter()
            .map(|entry| {
                let revision = entry.product_source_version;
                into_mutation(entry, request.tenant_id())
                    .map(|mutation| (mutation, revision))
                    .map_err(|_| permanent("product_index_record_invalid"))
            })
            .collect::<Result<Vec<_>, IndexSourceFailure>>()?;
        IndexSourceLoadBatch::with_owner_revisions(&request, entries)
            .map_err(|_| permanent("product_index_batch_invalid"))
    }
}

fn map_rows(
    rows: Vec<ProductLocaleReplayEntry>,
    expected_tenant: Uuid,
) -> Result<Vec<IndexMutation>, IndexSourceFailure> {
    rows.into_iter()
        .map(|row| {
            into_mutation(row, expected_tenant)
                .map_err(|_| permanent("product_index_record_invalid"))
        })
        .collect()
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

fn into_mutation(
    row: ProductLocaleReplayEntry,
    expected_tenant: Uuid,
) -> Result<IndexMutation, ProductIndexBridgeError> {
    let locale = LocaleKey::new(&row.locale).map_err(ProductIndexBridgeError::InvalidContract)?;
    if row.tenant_id != expected_tenant
        || row.product_id.is_nil()
        || row.source_version == 0
        || locale.as_str() != row.locale
    {
        return Err(ProductIndexBridgeError::InvalidRow);
    }
    let schema = product_schema_ref()?;
    let event_id = derive_index_schema_source_event_id(
        PRODUCT_EVENT_DOMAIN,
        row.tenant_id,
        &schema,
        row.product_id,
        Some(&locale),
        row.source_version,
    )
    .map_err(|_| ProductIndexBridgeError::InvalidRow)?;
    let key = EntityKey {
        tenant_id: row.tenant_id,
        schema,
        entity_id: row.product_id,
        locale: Some(locale),
    };

    let ProductLocaleReplayState::Live(mut live) = row.state else {
        return Ok(IndexMutation::Delete {
            event_id,
            key,
            source_version: row.source_version,
        });
    };

    let fields = BTreeMap::from([
        (field_name("id")?, IndexValue::Uuid(row.product_id)),
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
            source_version: row.source_version,
            fields,
            links,
        },
    })
}

fn field_name(name: &str) -> Result<FieldName, ProductIndexBridgeError> {
    FieldName::new(name).map_err(ProductIndexBridgeError::InvalidContract)
}

fn link_name(name: &str) -> Result<LinkName, ProductIndexBridgeError> {
    LinkName::new(name).map_err(ProductIndexBridgeError::InvalidContract)
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
            assert!(
                rustok_product::index_projection::PRODUCT_ROWS_CTE.contains(marker),
                "missing {marker}"
            );
        }
        assert!(
            rustok_product::index_projection::PRODUCT_ATTRIBUTE_TERMS_CTE
                .contains("product_filterable_attribute_values")
        );
    }

    #[test]
    fn relation_freshness_pending_is_retryable() {
        let failure = map_port_error(PortError::unavailable(
            PRODUCT_LOCALE_REPLAY_FRESHNESS_PENDING,
            "pending",
        ));
        assert!(failure.is_retryable());
        assert_eq!(failure.code(), PRODUCT_LOCALE_REPLAY_FRESHNESS_PENDING);
    }

    #[test]
    fn retained_product_row_emits_current_schema_delete_mutation() {
        let product = ProductLocaleReplayEntry {
            tenant_id: Uuid::from_u128(1),
            product_id: Uuid::from_u128(2),
            source_version: 9,
            product_source_version: std::num::NonZeroU64::new(12).unwrap(),
            locale: "en-US".into(),
            state: ProductLocaleReplayState::Deleted,
        };
        let IndexMutation::Delete {
            key,
            source_version,
            ..
        } = into_mutation(product, Uuid::from_u128(1)).unwrap()
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
