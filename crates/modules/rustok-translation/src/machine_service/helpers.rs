use rustok_api::{PortActorKind, PortContext, manifest_hash::hash_manifest};
use sea_orm::{ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, QueryFilter};
use uuid::Uuid;

use crate::{
    TranslationError, TranslationResult,
    entities::{job, job_item, machine_cancellation, machine_operation, machine_recovery},
};

pub(crate) fn actor_kind(context: &PortContext) -> &'static str {
    match context.actor.kind {
        PortActorKind::User => "user",
        PortActorKind::Service => "service",
        PortActorKind::System => "system",
    }
}

pub(crate) fn child_write_context(
    context: &PortContext,
    operation: &str,
) -> TranslationResult<PortContext> {
    let parent_key = context.idempotency_key.as_deref().unwrap_or_default();
    let idempotency_key = child_idempotency_key(parent_key, operation)?;
    let mut child = context.clone();
    child.causation_id = Some(context.correlation_id.clone());
    child.idempotency_key = Some(idempotency_key);
    Ok(child)
}

pub(crate) fn child_idempotency_key(
    parent_key: &str,
    operation: &str,
) -> TranslationResult<String> {
    let digest = hash_manifest(&(parent_key, operation))?;
    Ok(format!("translation-machine:{operation}:{digest}"))
}

pub(crate) fn is_digest(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

pub(crate) fn validate_operation_replay(
    operation: &machine_operation::Model,
    context: &PortContext,
    command_hash: &str,
) -> TranslationResult<()> {
    if operation.command_hash != command_hash {
        return Err(TranslationError::IdempotencyConflict);
    }
    if operation.requested_by_actor_kind != actor_kind(context)
        || operation.requested_by_actor_id != context.actor.id
    {
        return Err(TranslationError::IdempotencyActorMismatch);
    }
    Ok(())
}

pub(crate) async fn find_operation_by_idempotency<C>(
    database: &C,
    tenant_id: Uuid,
    idempotency_key: &str,
) -> TranslationResult<Option<machine_operation::Model>>
where
    C: ConnectionTrait,
{
    Ok(machine_operation::Entity::find()
        .filter(machine_operation::Column::TenantId.eq(tenant_id))
        .filter(machine_operation::Column::IdempotencyKey.eq(idempotency_key))
        .one(database)
        .await?)
}

pub(crate) async fn find_operation<C>(
    database: &C,
    tenant_id: Uuid,
    operation_id: Uuid,
) -> TranslationResult<machine_operation::Model>
where
    C: ConnectionTrait,
{
    machine_operation::Entity::find_by_id(operation_id)
        .filter(machine_operation::Column::TenantId.eq(tenant_id))
        .one(database)
        .await?
        .ok_or(TranslationError::MachineOperationNotFound)
}

pub(crate) async fn find_cancellation_by_idempotency<C>(
    database: &C,
    tenant_id: Uuid,
    idempotency_key: &str,
) -> TranslationResult<Option<machine_cancellation::Model>>
where
    C: ConnectionTrait,
{
    Ok(machine_cancellation::Entity::find()
        .filter(machine_cancellation::Column::TenantId.eq(tenant_id))
        .filter(machine_cancellation::Column::IdempotencyKey.eq(idempotency_key))
        .one(database)
        .await?)
}

pub(crate) async fn find_cancellation_by_operation<C>(
    database: &C,
    tenant_id: Uuid,
    operation_id: Uuid,
) -> TranslationResult<Option<machine_cancellation::Model>>
where
    C: ConnectionTrait,
{
    Ok(machine_cancellation::Entity::find()
        .filter(machine_cancellation::Column::TenantId.eq(tenant_id))
        .filter(machine_cancellation::Column::OperationId.eq(operation_id))
        .one(database)
        .await?)
}

pub(crate) async fn find_machine_recovery_by_idempotency<C>(
    database: &C,
    tenant_id: Uuid,
    idempotency_key: &str,
) -> TranslationResult<Option<machine_recovery::Model>>
where
    C: ConnectionTrait,
{
    Ok(machine_recovery::Entity::find()
        .filter(machine_recovery::Column::TenantId.eq(tenant_id))
        .filter(machine_recovery::Column::IdempotencyKey.eq(idempotency_key))
        .one(database)
        .await?)
}

pub(crate) async fn find_item(
    database: &DatabaseConnection,
    tenant_id: Uuid,
    item_id: Uuid,
) -> TranslationResult<job_item::Model> {
    job_item::Entity::find_by_id(item_id)
        .filter(job_item::Column::TenantId.eq(tenant_id))
        .one(database)
        .await?
        .ok_or(TranslationError::ItemNotFound)
}

pub(crate) async fn find_job(
    database: &DatabaseConnection,
    tenant_id: Uuid,
    job_id: Uuid,
) -> TranslationResult<job::Model> {
    job::Entity::find_by_id(job_id)
        .filter(job::Column::TenantId.eq(tenant_id))
        .one(database)
        .await?
        .ok_or(TranslationError::JobNotFound)
}
