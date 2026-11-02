//! Canonical source facts on the actual owner transaction, never a journal replay publisher.
use rustok_events::InventorySourceEvent;
use rustok_outbox::{ContractEventWriteOnceError, TransactionalEventBus};
use sea_orm::DatabaseTransaction;
use uuid::Uuid;

use super::InventorySourceError as E;

pub(super) async fn changed(
    txn: &DatabaseTransaction,
    tenant: Uuid,
    actor: Option<Uuid>,
    baseline_id: Uuid,
    variant_id: Uuid,
    source_version: i64,
    change_id: Uuid,
) -> Result<(), E> {
    if source_version <= 0 {
        return Err(E::StateUnverified);
    }
    let source_version = u64::try_from(source_version).map_err(|_| E::StateUnverified)?;
    publish(
        txn,
        tenant,
        actor,
        change_id,
        InventorySourceEvent::Changed {
            baseline_id,
            variant_id,
            source_version,
        },
    )
    .await
}

pub(super) async fn activated(
    txn: &DatabaseTransaction,
    tenant: Uuid,
    actor: Option<Uuid>,
    baseline_id: Uuid,
) -> Result<(), E> {
    publish(
        txn,
        tenant,
        actor,
        baseline_id,
        InventorySourceEvent::BaselineActivated { baseline_id },
    )
    .await
}

async fn publish(
    txn: &DatabaseTransaction,
    tenant: Uuid,
    actor: Option<Uuid>,
    envelope_id: Uuid,
    event: InventorySourceEvent,
) -> Result<(), E> {
    // The shared write-once helper historically filters nil actors. Never let that turn a
    // supplied invalid human identity into a service/system publication here.
    if tenant.is_nil() || envelope_id.is_nil() || actor.is_some_and(|id| id.is_nil()) {
        return Err(E::StateUnverified);
    }
    let saved = TransactionalEventBus::publish_contract_once_direct_in_tx_with_envelope_id(
        txn,
        envelope_id,
        tenant,
        actor,
        event,
    )
    .await
    .map_err(|error| match error {
        ContractEventWriteOnceError::Conflict => E::StateUnverified,
        ContractEventWriteOnceError::Unavailable => E::PublicationUnavailable,
    })?;
    if saved != envelope_id {
        return Err(E::StateUnverified);
    }
    Ok(())
}
