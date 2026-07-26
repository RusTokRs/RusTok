use chrono::Utc;
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseConnection, DatabaseTransaction, EntityTrait,
    QueryFilter, Set, TransactionTrait, sea_query::OnConflict,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::entities::{command_receipt, relation};
use crate::error::{SocialGraphError, SocialGraphResult};
use crate::model::SocialRelationKind;

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

    let request_json = serde_json::to_value(request)
        .map_err(|_| SocialGraphError::CommandReceiptCorrupt)?;
    let transaction = db.begin().await?;
    let receipt_id = Uuid::new_v4();
    let now = Utc::now().fixed_offset();
    command_receipt::Entity::insert(command_receipt::ActiveModel {
        id: Set(receipt_id),
        tenant_id: Set(tenant_id),
        idempotency_key: Set(idempotency_key.clone()),
        request_json: Set(request_json),
        status: Set(STATUS_PROCESSING.to_string()),
        response_json: Set(None),
        created_at: Set(now),
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
) -> SocialGraphResult<relation::Model> {
    let response_json = serde_json::to_value(response)
        .map_err(|_| SocialGraphError::CommandReceiptCorrupt)?;
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
            sea_orm::sea_query::Expr::value(now),
        )
        .col_expr(
            command_receipt::Column::CompletedAt,
            sea_orm::sea_query::Expr::value(Some(now)),
        )
        .filter(command_receipt::Column::Id.eq(receipt.receipt_id))
        .filter(command_receipt::Column::TenantId.eq(receipt.tenant_id))
        .filter(command_receipt::Column::Status.eq(STATUS_PROCESSING))
        .exec(&receipt.transaction)
        .await?;
    if updated.rows_affected != 1 {
        receipt.transaction.rollback().await?;
        return Err(SocialGraphError::CommandReceiptCorrupt);
    }
    receipt.transaction.commit().await?;
    Ok(response.clone())
}

pub(crate) async fn rollback<T>(
    receipt: NewSocialGraphCommandReceipt,
    error: SocialGraphError,
) -> SocialGraphResult<T> {
    receipt.transaction.rollback().await?;
    Err(error)
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
        assert_eq!(normalize_idempotency_key("  follow-1  ").unwrap(), "follow-1");
        assert!(normalize_idempotency_key(" ").is_err());
        assert!(normalize_idempotency_key("x".repeat(192)).is_err());
    }
}
