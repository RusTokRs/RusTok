use rustok_core::ModuleRuntimeExtensions;
use rustok_index::{
    DomainError, EntityKey, EntityName, IndexMutationEventAcknowledger, IndexReplayMutationSink,
    IndexSourceRefreshEventDelivery, IndexSourceRefreshEventError,
    IndexSourceRefreshEventProcessError, IndexSourceRefreshEventProcessOutcome,
    IndexSourceRefreshEventWorker, LocaleKey, ModuleName, SchemaRef, SchemaRegistry, SchemaVersion,
    SharedIndexMutationEventRegistry, SharedIndexSourceRegistry, register_index_mutation_event,
};
use thiserror::Error;
use uuid::Uuid;

use super::{
    PRODUCT_SCHEMA_ROUTING_KEY, product::PRODUCT_INDEX_SOURCE,
    variant::PRODUCT_VARIANT_INDEX_SOURCE,
};

pub(crate) const PRODUCT_INDEX_LOCALE_REFRESH_EVENT_DOMAIN: &str =
    "product.index.locale_refresh_requested";
pub(crate) const PRODUCT_INDEX_VARIANT_REFRESH_EVENT_DOMAIN: &str =
    "product.index.variant_refresh_requested";

const PRODUCT_OWNER_MODULE: &str = "product";
const PRODUCT_VARIANT_SCHEMA_VERSION: u32 = 2;

/// Distribution-owned typed boundary between the canonical Product refresh family and the generic
/// Index source-refresh worker.
///
/// Broker/transport code is responsible only for decoding the owner event and retaining its opaque
/// acknowledgement token. This type owns the canonical Product/ProductVariant -> `EntityKey`
/// projection; the generic Index worker still owns authoritative source loading, durable mutation
/// application, replay deduplication and commit-before-ack ordering.
pub(crate) enum ProductIndexRefreshDelivery<T> {
    Locale {
        event_id: Uuid,
        tenant_id: Uuid,
        product_id: Uuid,
        locale: String,
        source_version: u64,
        acknowledgement_token: T,
    },
    Variant {
        event_id: Uuid,
        tenant_id: Uuid,
        product_id: Uuid,
        variant_id: Uuid,
        source_version: u64,
        acknowledgement_token: T,
    },
}

impl<T> ProductIndexRefreshDelivery<T> {
    pub(crate) fn locale(
        event_id: Uuid,
        tenant_id: Uuid,
        product_id: Uuid,
        locale: impl Into<String>,
        source_version: u64,
        acknowledgement_token: T,
    ) -> Self {
        Self::Locale {
            event_id,
            tenant_id,
            product_id,
            locale: locale.into(),
            source_version,
            acknowledgement_token,
        }
    }

    pub(crate) fn variant(
        event_id: Uuid,
        tenant_id: Uuid,
        product_id: Uuid,
        variant_id: Uuid,
        source_version: u64,
        acknowledgement_token: T,
    ) -> Self {
        Self::Variant {
            event_id,
            tenant_id,
            product_id,
            variant_id,
            source_version,
            acknowledgement_token,
        }
    }

    pub(crate) fn into_index_delivery(
        self,
    ) -> Result<IndexSourceRefreshEventDelivery<T>, ProductIndexRefreshDeliveryError> {
        match self {
            Self::Locale {
                event_id,
                tenant_id,
                product_id,
                locale,
                source_version,
                acknowledgement_token,
            } => {
                ensure_product_id(product_id)?;
                let key = EntityKey {
                    tenant_id,
                    schema: product_schema_ref()?,
                    entity_id: product_id,
                    locale: Some(LocaleKey::new(locale)?),
                };
                Ok(IndexSourceRefreshEventDelivery::new(
                    PRODUCT_INDEX_LOCALE_REFRESH_EVENT_DOMAIN,
                    event_id,
                    key,
                    source_version,
                    acknowledgement_token,
                )?)
            }
            Self::Variant {
                event_id,
                tenant_id,
                product_id,
                variant_id,
                source_version,
                acknowledgement_token,
            } => {
                ensure_product_id(product_id)?;
                let key = EntityKey {
                    tenant_id,
                    schema: product_variant_schema_ref()?,
                    entity_id: variant_id,
                    locale: None,
                };
                Ok(IndexSourceRefreshEventDelivery::new(
                    PRODUCT_INDEX_VARIANT_REFRESH_EVENT_DOMAIN,
                    event_id,
                    key,
                    source_version,
                    acknowledgement_token,
                )?)
            }
        }
    }
}

/// Thin production bridge that keeps Product-specific decoding out of `rustok-index` while reusing
/// its fail-closed source-refresh orchestration unchanged.
pub(crate) struct ProductIndexRefreshDeliveryWorker<M, A> {
    inner: IndexSourceRefreshEventWorker<M, A>,
}

impl<M, A> ProductIndexRefreshDeliveryWorker<M, A>
where
    M: IndexReplayMutationSink,
    A: IndexMutationEventAcknowledger,
{
    pub(crate) fn new(mutation_sink: M, acknowledger: A) -> Self {
        Self {
            inner: IndexSourceRefreshEventWorker::new(mutation_sink, acknowledger),
        }
    }

    pub(crate) async fn process(
        &self,
        schema_registry: &SchemaRegistry,
        source_registry: &SharedIndexSourceRegistry,
        event_registry: &SharedIndexMutationEventRegistry,
        delivery: ProductIndexRefreshDelivery<A::Token>,
    ) -> Result<IndexSourceRefreshEventProcessOutcome, ProductIndexRefreshDeliveryProcessError> {
        let delivery = delivery.into_index_delivery()?;
        Ok(self
            .inner
            .process(
                schema_registry,
                source_registry,
                event_registry,
                delivery,
            )
            .await?)
    }
}

pub(crate) fn register(extensions: &mut ModuleRuntimeExtensions) -> rustok_core::Result<()> {
    if !extensions.contains::<rustok_product::ProductRuntimeSelected>() {
        return Ok(());
    }

    register_index_mutation_event(
        extensions,
        PRODUCT_OWNER_MODULE,
        PRODUCT_INDEX_LOCALE_REFRESH_EVENT_DOMAIN,
        PRODUCT_INDEX_SOURCE,
        product_schema_ref().map_err(invalid_contract)?,
    )
    .map_err(|error| {
        rustok_core::Error::Validation(format!(
            "selected Product Index locale refresh route registration failed: {error}"
        ))
    })?;

    register_index_mutation_event(
        extensions,
        PRODUCT_OWNER_MODULE,
        PRODUCT_INDEX_VARIANT_REFRESH_EVENT_DOMAIN,
        PRODUCT_VARIANT_INDEX_SOURCE,
        product_variant_schema_ref().map_err(invalid_contract)?,
    )
    .map_err(|error| {
        rustok_core::Error::Validation(format!(
            "selected ProductVariant Index refresh route registration failed: {error}"
        ))
    })
}

fn product_schema_ref() -> Result<SchemaRef, DomainError> {
    Ok(SchemaRef {
        module: ModuleName::new("rustok-product")?,
        entity: EntityName::new("product")?,
        version: SchemaVersion::new(PRODUCT_SCHEMA_ROUTING_KEY),
    })
}

fn product_variant_schema_ref() -> Result<SchemaRef, DomainError> {
    Ok(SchemaRef {
        module: ModuleName::new("rustok-product")?,
        entity: EntityName::new("product_variant")?,
        version: SchemaVersion::new(PRODUCT_VARIANT_SCHEMA_VERSION),
    })
}

fn ensure_product_id(product_id: Uuid) -> Result<(), ProductIndexRefreshDeliveryError> {
    if product_id.is_nil() {
        Err(ProductIndexRefreshDeliveryError::NilProductId)
    } else {
        Ok(())
    }
}

fn invalid_contract(error: DomainError) -> rustok_core::Error {
    rustok_core::Error::Validation(format!(
        "selected Product Index refresh contract is invalid: {error}"
    ))
}

#[derive(Debug, Error)]
pub(crate) enum ProductIndexRefreshDeliveryError {
    #[error("Product Index refresh product UUID cannot be nil")]
    NilProductId,
    #[error("Product Index refresh target contract is invalid")]
    InvalidContract(#[from] DomainError),
    #[error("Product Index source refresh delivery is invalid")]
    Delivery(#[from] IndexSourceRefreshEventError),
}

#[derive(Debug, Error)]
pub(crate) enum ProductIndexRefreshDeliveryProcessError {
    #[error("Product Index refresh delivery decoding failed")]
    Delivery(#[from] ProductIndexRefreshDeliveryError),
    #[error("Product Index refresh worker failed")]
    Process(#[from] IndexSourceRefreshEventProcessError),
}

#[cfg(test)]
mod tests {
    use rustok_index::IndexMutationEventCatalog;

    use super::*;

    #[test]
    fn locale_delivery_builds_canonical_product_entity_key() {
        let event_id = Uuid::from_u128(10);
        let tenant_id = Uuid::from_u128(20);
        let product_id = Uuid::from_u128(30);

        let delivery = ProductIndexRefreshDelivery::locale(
            event_id,
            tenant_id,
            product_id,
            "en-us",
            7,
            "ack-locale".to_owned(),
        )
        .into_index_delivery()
        .unwrap();

        assert_eq!(
            delivery.event_domain(),
            PRODUCT_INDEX_LOCALE_REFRESH_EVENT_DOMAIN
        );
        assert_eq!(delivery.event_id(), event_id);
        assert_eq!(delivery.minimum_source_version(), 7);
        assert_eq!(delivery.key().tenant_id, tenant_id);
        assert_eq!(delivery.key().entity_id, product_id);
        assert_eq!(delivery.key().schema, product_schema_ref().unwrap());
        assert_eq!(delivery.key().locale.as_ref().unwrap().as_str(), "en-US");
        assert_eq!(delivery.acknowledgement_token(), "ack-locale");
    }

    #[test]
    fn variant_delivery_builds_nonlocalized_variant_entity_key() {
        let event_id = Uuid::from_u128(11);
        let tenant_id = Uuid::from_u128(21);
        let product_id = Uuid::from_u128(31);
        let variant_id = Uuid::from_u128(41);

        let delivery = ProductIndexRefreshDelivery::variant(
            event_id,
            tenant_id,
            product_id,
            variant_id,
            9,
            "ack-variant".to_owned(),
        )
        .into_index_delivery()
        .unwrap();

        assert_eq!(
            delivery.event_domain(),
            PRODUCT_INDEX_VARIANT_REFRESH_EVENT_DOMAIN
        );
        assert_eq!(delivery.event_id(), event_id);
        assert_eq!(delivery.minimum_source_version(), 9);
        assert_eq!(delivery.key().tenant_id, tenant_id);
        assert_eq!(delivery.key().entity_id, variant_id);
        assert_eq!(delivery.key().schema, product_variant_schema_ref().unwrap());
        assert!(delivery.key().locale.is_none());
        assert_eq!(delivery.acknowledgement_token(), "ack-variant");
    }

    #[test]
    fn variant_delivery_rejects_nil_owner_product_id() {
        let result = ProductIndexRefreshDelivery::variant(
            Uuid::from_u128(11),
            Uuid::from_u128(21),
            Uuid::nil(),
            Uuid::from_u128(41),
            9,
            (),
        )
        .into_index_delivery();

        assert!(matches!(
            result,
            Err(ProductIndexRefreshDeliveryError::NilProductId)
        ));
    }

    #[test]
    fn selected_product_registers_exact_locale_and_variant_refresh_routes() {
        let mut extensions = ModuleRuntimeExtensions::default();
        extensions.insert(rustok_product::ProductRuntimeSelected);

        register(&mut extensions).unwrap();

        let routes = extensions
            .get::<IndexMutationEventCatalog>()
            .expect("Product selection must publish refresh event routes");
        assert_eq!(routes.len(), 2);

        let locale = routes
            .get(PRODUCT_INDEX_LOCALE_REFRESH_EVENT_DOMAIN)
            .expect("locale refresh route");
        assert_eq!(locale.owner_module(), PRODUCT_OWNER_MODULE);
        assert_eq!(locale.source_name(), PRODUCT_INDEX_SOURCE);
        assert_eq!(locale.schema(), &product_schema_ref().unwrap());

        let variant = routes
            .get(PRODUCT_INDEX_VARIANT_REFRESH_EVENT_DOMAIN)
            .expect("variant refresh route");
        assert_eq!(variant.owner_module(), PRODUCT_OWNER_MODULE);
        assert_eq!(variant.source_name(), PRODUCT_VARIANT_INDEX_SOURCE);
        assert_eq!(variant.schema(), &product_variant_schema_ref().unwrap());
    }
}
