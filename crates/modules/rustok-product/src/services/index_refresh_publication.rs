use rustok_events::{DomainEvent, EventContract, EventEnvelope};
use rustok_outbox::{ContractEventWriteOnceError, SysEvents, TransactionalEventBus};
use sea_orm::{DatabaseTransaction, EntityTrait};
use thiserror::Error;
use uuid::Uuid;

use super::{ProductIndexLocaleRefreshRecord, ProductIndexVariantRefreshRecord};

/// Exact Product-owned target facts that a typed Index refresh event must expose.
///
/// Envelope identity and causation are deliberately absent: they come from the
/// immutable owner ledger and are never reconstructed from event payload data.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProductIndexRefreshContractTarget {
    Locale {
        product_id: Uuid,
        locale: String,
        source_version: u64,
    },
    Variant {
        product_id: Uuid,
        variant_id: Uuid,
        source_version: u64,
    },
}

/// Product-specific sealed-contract projection used by the canonical writer.
///
/// `EventContract` itself is sealed by `rustok-events`. Because this trait is
/// owned by `rustok-product`, only this crate can implement it for a future
/// Product refresh event family. Other modules therefore cannot route an
/// unrelated typed event through Product ledger identities.
pub trait ProductIndexRefreshContract: EventContract {
    fn product_index_refresh_target(&self) -> ProductIndexRefreshContractTarget;
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum ProductIndexRefreshPublicationError {
    #[error("Product Index refresh event does not match the immutable owner ledger row")]
    ContractMismatch,
    #[error("Product Index refresh ledger causation does not match a Product root envelope")]
    CausationMismatch,
    #[error("Product Index refresh envelope identity is already bound to different facts")]
    Conflict,
    #[error("Product Index refresh canonical publication is unavailable")]
    Unavailable,
}

/// Canonical write-once handoff from Product refresh ledgers to `sys_events`.
///
/// This type does not page ledgers, run a loop, own retry state, or dispatch to
/// a broker. A future relay supplies one validated typed Product refresh event
/// and one caller-owned transaction. Delivery after commit remains owned by the
/// shared `OutboxRelay`.
#[derive(Clone, Copy, Debug, Default)]
pub struct ProductIndexRefreshCanonicalWriter;

impl ProductIndexRefreshCanonicalWriter {
    pub async fn publish_locale_once_in_transaction<E>(
        transaction: &DatabaseTransaction,
        record: &ProductIndexLocaleRefreshRecord,
        event: E,
    ) -> Result<Uuid, ProductIndexRefreshPublicationError>
    where
        E: ProductIndexRefreshContract,
    {
        let expected = ProductIndexRefreshContractTarget::Locale {
            product_id: record.product_id(),
            locale: record.locale().to_owned(),
            source_version: record.source_version(),
        };
        if event.product_index_refresh_target() != expected {
            return Err(ProductIndexRefreshPublicationError::ContractMismatch);
        }

        publish_once(
            transaction,
            record.refresh_id(),
            record.root_event_id(),
            record.tenant_id(),
            record.product_id(),
            event,
        )
        .await
    }

    pub async fn publish_variant_once_in_transaction<E>(
        transaction: &DatabaseTransaction,
        record: &ProductIndexVariantRefreshRecord,
        event: E,
    ) -> Result<Uuid, ProductIndexRefreshPublicationError>
    where
        E: ProductIndexRefreshContract,
    {
        let expected = ProductIndexRefreshContractTarget::Variant {
            product_id: record.product_id(),
            variant_id: record.variant_id(),
            source_version: record.source_version(),
        };
        if event.product_index_refresh_target() != expected {
            return Err(ProductIndexRefreshPublicationError::ContractMismatch);
        }

        publish_once(
            transaction,
            record.refresh_id(),
            record.root_event_id(),
            record.tenant_id(),
            record.product_id(),
            event,
        )
        .await
    }
}

async fn publish_once<E>(
    transaction: &DatabaseTransaction,
    refresh_id: Uuid,
    root_event_id: Uuid,
    tenant_id: Uuid,
    product_id: Uuid,
    event: E,
) -> Result<Uuid, ProductIndexRefreshPublicationError>
where
    E: ProductIndexRefreshContract,
{
    let actor_id =
        load_product_root_actor(transaction, root_event_id, tenant_id, product_id).await?;

    TransactionalEventBus::publish_contract_once_direct_in_tx_with_envelope_id_and_causation(
        transaction,
        refresh_id,
        tenant_id,
        actor_id,
        root_event_id,
        event,
    )
    .await
    .map_err(map_write_once_error)
}

async fn load_product_root_actor(
    transaction: &DatabaseTransaction,
    root_event_id: Uuid,
    tenant_id: Uuid,
    product_id: Uuid,
) -> Result<Option<Uuid>, ProductIndexRefreshPublicationError> {
    let stored = SysEvents::find_by_id(root_event_id)
        .one(transaction)
        .await
        .map_err(|_| ProductIndexRefreshPublicationError::Unavailable)?
        .ok_or(ProductIndexRefreshPublicationError::CausationMismatch)?;
    let envelope: EventEnvelope = serde_json::from_value(stored.payload)
        .map_err(|_| ProductIndexRefreshPublicationError::CausationMismatch)?;
    envelope
        .validate_registered_schema()
        .map_err(|_| ProductIndexRefreshPublicationError::CausationMismatch)?;

    let envelope_schema_version = i16::try_from(envelope.schema_version)
        .map_err(|_| ProductIndexRefreshPublicationError::CausationMismatch)?;
    if envelope.id != stored.id
        || envelope.id != root_event_id
        || envelope.event_type != stored.event_type
        || envelope_schema_version != stored.schema_version
        || envelope.tenant_id != tenant_id
    {
        return Err(ProductIndexRefreshPublicationError::CausationMismatch);
    }

    let root_product_id = match &envelope.event {
        DomainEvent::ProductCreated { product_id }
        | DomainEvent::ProductUpdated { product_id }
        | DomainEvent::ProductPublished { product_id }
        | DomainEvent::ProductDeleted { product_id } => *product_id,
        _ => return Err(ProductIndexRefreshPublicationError::CausationMismatch),
    };
    if root_product_id != product_id {
        return Err(ProductIndexRefreshPublicationError::CausationMismatch);
    }

    Ok(envelope.actor_id)
}

const fn map_write_once_error(
    error: ContractEventWriteOnceError,
) -> ProductIndexRefreshPublicationError {
    match error {
        ContractEventWriteOnceError::Conflict => ProductIndexRefreshPublicationError::Conflict,
        ContractEventWriteOnceError::Unavailable => {
            ProductIndexRefreshPublicationError::Unavailable
        }
    }
}
