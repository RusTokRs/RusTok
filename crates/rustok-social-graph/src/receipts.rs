use chrono::Utc;
use rustok_outbox::TransactionalEventBus;
use sea_orm::prelude::DateTimeWithTimeZone;
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseConnection, DatabaseTransaction, EntityTrait,
    QueryFilter, QueryOrder, QuerySelect, Set, TransactionTrait, sea_query::OnConflict,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::entities::{command_receipt, relation};
use crate::error::{SocialGraphError, SocialGraphResult};
use crate::external_events::event_for_relation;
use crate::model::SocialRelationKind;

const COMMAND_RECEIPT_SCHEMA_VERSION: i32 = 1;
const MAX_IDEMPOTENCY_KEY_BYTES: usize = 191;
const STATUS_PROCESSING: &str = "processing";
const STATUS_COMPLETED: &str = "completed";

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub(crate) struct SocialGraphCommandReceiptRequest {
    pub source_user_id: Uuid,
    pub target_user_id: Uuid,
    pub relation_kind: SocialRelationKind,
    pub active: bool,
    pub expected_revision: Option<i64>,
}

pub(crate) struct NewSocialGraphCommandReceipt {
    pub transaction: DatabaseTransaction,
    receipt_id: Uuid,
    tenant_id: Uuid,
}

pub(crate) enum SocialGraphCommandReceiptAdmission {
    Replay(command_receipt::Model),
    New(NewSocialGraphCommandReceipt),
}

pub(crate) fn normalize_idempotency_key(value: impl Into<String>) -> SocialGraphResult<String> {
    let value = value.into().trim().to_string();
    if value.is_empty() || value.len() > MAX_IDEMPOTENCY_KEY_BYTES {
        return Err(SocialGraphError::IdempotencyKeyInvalid);
    }
    Ok(value)
}

pub(crate) async fn admit(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    idempotency_key: String,
    request: &SocialGraphCommandReceiptRequest,
) -> SocialGraphResult<SocialGraphCommandReceiptAdmission> {
    let idempotency_key = normalize_idempotency_key(idempotency_key)?;
    if let Some(existing) = find_receipt(db, tenant_id, idempotency_key.as_str()).await? {
        return Ok(SocialGraphCommandReceiptAdmission::Replay(existing));
    }

    let request_json =
        serde_json::to_value(request).map_err(|_| SocialGraphError::CommandReceiptCorrupt)?;
    let transaction = db.begin().await?;
    let receipt_id = Uuid::new_v4();
    let now = Utc::now().fixed_offset();
    command_receipt::Entity::insert(command_receipt::ActiveModel {
        id: Set(receipt_id),
        tenant_id: Set(tenant_id),
        idempotency_key: Set(idempotency_key.clone()),
        schema_version: Set(COMMAND_RECEIPT_SCHEMA_VERSION),
        request_json: Set(request_json),
        status: Set(STATUS_PROCESSING.to_string()),
        response_json: Set(None),
        created_at: Set(now.clone()),
        updated_at: Set(now),
        completed_at: Set(None),
    })
    .on_conflict(
        OnConflict::columns([
            command_receipt::Column::TenantId,
            command_receipt::Column::IdempotencyKey,
        ])
        .do_nothing()
        .to_owned(),
    )
    .exec_without_returning(&transaction)
    .await?;

    let stored = find_receipt(&transaction, tenant_id, idempotency_key.as_str())
        .await?
        .ok_or(SocialGraphError::CommandReceiptCorrupt)?;
    if stored.id != receipt_id {
        transaction.rollback().await?;
        return Ok(SocialGraphCommandReceiptAdmission::Replay(stored));
    }

    Ok(SocialGraphCommandReceiptAdmission::New(
        NewSocialGraphCommandReceipt {
            transaction,
            receipt_id,
            tenant_id,
        },
    ))
}

pub(crate) fn replay(
    receipt: command_receipt::Model,
    expected_request: &SocialGraphCommandReceiptRequest,
) -> SocialGraphResult<relation::Model> {
    let expected_json = serde_json::to_value(expected_request)
        .map_err(|_| SocialGraphError::CommandReceiptCorrupt)?;
    if receipt.schema_version != COMMAND_RECEIPT_SCHEMA_VERSION {
        return Err(SocialGraphError::CommandReceiptCorrupt);
    }
    if receipt.request_json != expected_json {
        return Err(SocialGraphError::IdempotencyConflict);
    }
    if receipt.status != STATUS_COMPLETED || receipt.completed_at.is_none() {
        return Err(SocialGraphError::CommandReceiptCorrupt);
    }
    let response = receipt
        .response_json
        .ok_or(SocialGraphError::CommandReceiptCorrupt)?;
    serde_json::from_value(response).map_err(|_| SocialGraphError::CommandReceiptCorrupt)
}

pub(crate) async fn complete(
    receipt: NewSocialGraphCommandReceipt,
    response: &relation::Model,
    state_changed: bool,
    actor_id: Option<Uuid>,
    event_bus: &TransactionalEventBus,
) -> SocialGraphResult<relation::Model> {
    let NewSocialGraphCommandReceipt {
        transaction,
        receipt_id,
        tenant_id,
    } = receipt;
    let response_json =
        serde_json::to_value(response).map_err(|_| SocialGraphError::CommandReceiptCorrupt)?;

    if state_changed {
        let event = event_for_relation(response);
        if let Err(error) = event_bus
            .publish_contract_in_tx(&transaction, tenant_id, actor_id, event)
            .await
        {
            tracing::error!(
                tenant_id = %tenant_id,
                relation_id = %response.id,
                source_user_id = %response.source_user_id,
                target_user_id = %response.target_user_id,
                relation_kind = response.relation_kind.as_str(),
                revision = response.revision,
                error = %error,
                "Social Graph transactional relation event publication failed"
            );
            transaction.rollback().await?;
            return Err(SocialGraphError::EventPublicationUnavailable);
        }
    }

    let now = Utc::now().fixed_offset();
    let updated = command_receipt::Entity::update_many()
        .col_expr(
            command_receipt::Column::Status,
            sea_orm::sea_query::Expr::value(STATUS_COMPLETED),
        )
        .col_expr(
            command_receipt::Column::ResponseJson,
            sea_orm::sea_query::Expr::value(Some(response_json)),
        )
        .col_expr(
            command_receipt::Column::UpdatedAt,
            sea_orm::sea_query::Expr::value(now.clone()),
        )
        .col_expr(
            command_receipt::Column::CompletedAt,
            sea_orm::sea_query::Expr::value(Some(now)),
        )
        .filter(command_receipt::Column::Id.eq(receipt_id))
        .filter(command_receipt::Column::TenantId.eq(tenant_id))
        .filter(command_receipt::Column::SchemaVersion.eq(COMMAND_RECEIPT_SCHEMA_VERSION))
        .filter(command_receipt::Column::Status.eq(STATUS_PROCESSING))
        .exec(&transaction)
        .await?;
    if updated.rows_affected != 1 {
        transaction.rollback().await?;
        return Err(SocialGraphError::CommandReceiptCorrupt);
    }
    transaction.commit().await?;
    Ok(response.clone())
}

pub(crate) async fn rollback<T>(
    receipt: NewSocialGraphCommandReceipt,
    error: SocialGraphError,
) -> SocialGraphResult<T> {
    receipt.transaction.rollback().await?;
    Err(error)
}

pub(crate) async fn cleanup_completed(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    completed_before: DateTimeWithTimeZone,
    limit: u64,
    dry_run: bool,
) -> SocialGraphResult<(u64, u64, Option<i64>)> {
    let candidates = command_receipt::Entity::find()
        .filter(command_receipt::Column::TenantId.eq(tenant_id))
        .filter(command_receipt::Column::SchemaVersion.eq(COMMAND_RECEIPT_SCHEMA_VERSION))
        .filter(command_receipt::Column::Status.eq(STATUS_COMPLETED))
        .filter(command_receipt::Column::CompletedAt.is_not_null())
        .filter(command_receipt::Column::CompletedAt.lt(completed_before.clone()))
        .order_by_asc(command_receipt::Column::CompletedAt)
        .order_by_asc(command_receipt::Column::Id)
        .limit(limit)
        .all(db)
        .await?;
    for receipt in &candidates {
        validate_cleanup_candidate(receipt)?;
    }

    let matched = candidates.len() as u64;
    if dry_run || candidates.is_empty() {
        return Ok((matched, 0, oldest_completed_timestamp(db, tenant_id).await?));
    }

    let candidate_ids = candidates
        .into_iter()
        .map(|receipt| receipt.id)
        .collect::<Vec<_>>();
    let deleted = command_receipt::Entity::delete_many()
        .filter(command_receipt::Column::TenantId.eq(tenant_id))
        .filter(command_receipt::Column::SchemaVersion.eq(COMMAND_RECEIPT_SCHEMA_VERSION))
        .filter(command_receipt::Column::Status.eq(STATUS_COMPLETED))
        .filter(command_receipt::Column::CompletedAt.is_not_null())
        .filter(command_receipt::Column::CompletedAt.lt(completed_before))
        .filter(command_receipt::Column::Id.is_in(candidate_ids))
        .exec(db)
        .await?
        .rows_affected;

    Ok((
        matched,
        deleted,
        oldest_completed_timestamp(db, tenant_id).await?,
    ))
}

fn validate_cleanup_candidate(receipt: &command_receipt::Model) -> SocialGraphResult<()> {
    if receipt.schema_version != COMMAND_RECEIPT_SCHEMA_VERSION
        || receipt.status != STATUS_COMPLETED
        || receipt.completed_at.is_none()
    {
        return Err(SocialGraphError::CommandReceiptCorrupt);
    }
    serde_json::from_value::<SocialGraphCommandReceiptRequest>(receipt.request_json.clone())
        .map_err(|_| SocialGraphError::CommandReceiptCorrupt)?;
    let response = receipt
        .response_json
        .clone()
        .ok_or(SocialGraphError::CommandReceiptCorrupt)?;
    serde_json::from_value::<relation::Model>(response)
        .map_err(|_| SocialGraphError::CommandReceiptCorrupt)?;
    Ok(())
}

async fn oldest_completed_timestamp(
    db: &DatabaseConnection,
    tenant_id: Uuid,
) -> SocialGraphResult<Option<i64>> {
    Ok(command_receipt::Entity::find()
        .filter(command_receipt::Column::TenantId.eq(tenant_id))
        .filter(command_receipt::Column::SchemaVersion.eq(COMMAND_RECEIPT_SCHEMA_VERSION))
        .filter(command_receipt::Column::Status.eq(STATUS_COMPLETED))
        .filter(command_receipt::Column::CompletedAt.is_not_null())
        .order_by_asc(command_receipt::Column::CompletedAt)
        .order_by_asc(command_receipt::Column::Id)
        .one(db)
        .await?
        .and_then(|receipt| receipt.completed_at.map(|value| value.timestamp())))
}

async fn find_receipt<C>(
    connection: &C,
    tenant_id: Uuid,
    idempotency_key: &str,
) -> SocialGraphResult<Option<command_receipt::Model>>
where
    C: ConnectionTrait,
{
    command_receipt::Entity::find()
        .filter(command_receipt::Column::TenantId.eq(tenant_id))
        .filter(command_receipt::Column::IdempotencyKey.eq(idempotency_key))
        .one(connection)
        .await
        .map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idempotency_key_is_trimmed_and_bounded() {
        assert_eq!(
            normalize_idempotency_key("  follow-1  ").unwrap(),
            "follow-1"
        );
        assert!(normalize_idempotency_key(" ").is_err());
        assert!(normalize_idempotency_key("x".repeat(192)).is_err());
    }
}
