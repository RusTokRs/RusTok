//! Idempotent operation receipts for publish request reviews and holds.

use sea_orm::{ConnectionTrait, DatabaseTransaction, Statement, Value};

use super::helpers::*;
use super::*;
use crate::ModuleCommandContext;

pub(crate) struct PublishRequestReviewReceipt<'a> {
    pub(crate) operation_kind: &'static str,
    pub(crate) request_id: &'a str,
    pub(crate) expected_revision: i64,
    pub(crate) context: &'a ModuleCommandContext,
    pub(crate) actor_principal: &'a serde_json::Value,
    pub(crate) reason: &'a str,
    pub(crate) reason_code: &'a str,
}

pub(crate) async fn lock_publish_request(
    tx: &DatabaseTransaction,
    backend: SupportedBackend,
    request_id: &str,
) -> Result<(), ModuleGovernanceError> {
    let mark = if backend == SupportedBackend::Postgres {
        "$1"
    } else {
        "?1"
    };
    let request_lock = if backend == SupportedBackend::Postgres {
        " FOR UPDATE"
    } else {
        ""
    };
    tx.query_one_raw(Statement::from_sql_and_values(
        backend.db_backend(),
        format!("SELECT id FROM registry_publish_requests WHERE id = {mark}{request_lock}"),
        vec![request_id.to_string().into()],
    ))
    .await
    .map_err(store_error)?
    .ok_or(ModuleGovernanceError::PublishRequestNotFound)?;
    Ok(())
}

pub(crate) async fn publish_request_review_replay(
    tx: &DatabaseTransaction,
    backend: SupportedBackend,
    receipt: &PublishRequestReviewReceipt<'_>,
) -> Result<bool, ModuleGovernanceError> {
    let mark = |n| {
        if backend == SupportedBackend::Postgres {
            format!("${n}")
        } else {
            format!("?{n}")
        }
    };
    let existing = tx
        .query_one_raw(Statement::from_sql_and_values(
            backend.db_backend(),
            format!(
                "SELECT operation_kind, expected_revision, CAST(actor_id AS TEXT) AS actor_id, \
                 trace_id, CAST(correlation_id AS TEXT) AS correlation_id, \
                 CAST(actor_principal AS TEXT) AS actor_principal, reason, reason_code \
                 FROM registry_publish_request_review_operations \
                 WHERE request_id = {} AND idempotency_key = {}",
                mark(1),
                mark(2),
            ),
            vec![
                receipt.request_id.to_string().into(),
                backend.uuid_value(receipt.context.idempotency_key),
            ],
        ))
        .await
        .map_err(store_error)?;
    let Some(existing) = existing else {
        return Ok(false);
    };
    let stored_actor: serde_json::Value = serde_json::from_str(
        &existing
            .try_get::<String>("", "actor_principal")
            .map_err(store_error)?,
    )
    .map_err(store_error)?;
    if existing
        .try_get::<String>("", "operation_kind")
        .map_err(store_error)?
        != receipt.operation_kind
        || existing
            .try_get::<i64>("", "expected_revision")
            .map_err(store_error)?
            != receipt.expected_revision
        || existing
            .try_get::<String>("", "actor_id")
            .map_err(store_error)?
            != receipt.context.actor_id.to_string()
        || existing
            .try_get::<String>("", "trace_id")
            .map_err(store_error)?
            != receipt.context.trace_id
        || existing
            .try_get::<String>("", "correlation_id")
            .map_err(store_error)?
            != receipt.context.correlation_id.to_string()
        || stored_actor != *receipt.actor_principal
        || existing
            .try_get::<String>("", "reason")
            .map_err(store_error)?
            != receipt.reason
        || existing
            .try_get::<String>("", "reason_code")
            .map_err(store_error)?
            != receipt.reason_code
    {
        return Err(ModuleGovernanceError::PublishRequestReviewIdempotencyConflict);
    }
    Ok(true)
}

pub(crate) async fn record_publish_request_review_receipt(
    infrastructure: &ControlPlaneInfrastructure,
    tx: &DatabaseTransaction,
    backend: SupportedBackend,
    now: &str,
    receipt: &PublishRequestReviewReceipt<'_>,
    resulting_status: &str,
    resulting_revision: i64,
) -> Result<(), ModuleGovernanceError> {
    let mark = |n| {
        if backend == SupportedBackend::Postgres {
            format!("${n}")
        } else {
            format!("?{n}")
        }
    };
    tx.execute_raw(Statement::from_sql_and_values(
        backend.db_backend(),
        format!(
            "INSERT INTO registry_publish_request_review_operations \
             (operation_id, request_id, operation_kind, idempotency_key, expected_revision, actor_id, \
              trace_id, correlation_id, actor_principal, reason, reason_code, resulting_status, \
              resulting_revision, committed_at) \
             VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {now})",
            mark(1),
            mark(2),
            mark(3),
            mark(4),
            mark(5),
            mark(6),
            mark(7),
            mark(8),
            mark(9),
            mark(10),
            mark(11),
            mark(12),
            mark(13),
        ),
        vec![
            backend.uuid_value(infrastructure.new_id()),
            receipt.request_id.to_string().into(),
            receipt.operation_kind.into(),
            backend.uuid_value(receipt.context.idempotency_key),
            receipt.expected_revision.into(),
            backend.uuid_value(receipt.context.actor_id),
            receipt.context.trace_id.clone().into(),
            backend.uuid_value(receipt.context.correlation_id),
            Value::Json(Some(Box::new(receipt.actor_principal.clone()))),
            receipt.reason.to_string().into(),
            receipt.reason_code.to_string().into(),
            resulting_status.to_string().into(),
            resulting_revision.into(),
        ],
    ))
    .await
    .map_err(store_error)?;
    Ok(())
}
