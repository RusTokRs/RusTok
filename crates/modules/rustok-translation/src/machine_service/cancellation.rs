use chrono::{DateTime, FixedOffset, Utc};
use rustok_api::{Action, PortCallPolicy, PortContext, Resource, manifest_hash::hash_manifest};
use rustok_core::{PermissionScope, SecurityContext, generate_id};
use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set, TransactionTrait,
    sea_query::{Expr, OnConflict},
};
use uuid::Uuid;

use crate::{
    MachineTranslationExecutionStatus, MachineTranslationPort, TranslationError, TranslationResult,
    entities::{machine_cancellation, machine_memory_binding, machine_operation, machine_recovery},
};

use super::helpers::{
    actor_kind, child_idempotency_key, find_cancellation_by_idempotency,
    find_cancellation_by_operation, find_operation,
};
use super::types::{
    CancelMachineOperationInput, GenerateMachineProposalInput, MachineCancellationRecord,
    MachineOperationStatusRecord,
};

pub async fn cancel_machine_operation(
    database: &DatabaseConnection,
    machine_port: Option<&dyn MachineTranslationPort>,
    context: PortContext,
    input: CancelMachineOperationInput,
) -> TranslationResult<MachineCancellationRecord> {
    context.require_policy(PortCallPolicy::write())?;
    validate_machine_cancellation_reason(&input.reason)?;
    let tenant_id =
        Uuid::parse_str(&context.tenant_id).map_err(|_| TranslationError::InvalidTenantId)?;
    let security = SecurityContext::try_from_port_context(&context)?;
    if security.get_scope(Resource::Translations, Action::Run) == PermissionScope::None {
        return Err(TranslationError::Forbidden);
    }
    let request_hash = hash_manifest(&input)?;
    let idempotency_key = context.idempotency_key.clone().unwrap_or_default();
    if let Some(existing) =
        find_cancellation_by_idempotency(database, tenant_id, &idempotency_key).await?
    {
        validate_machine_cancellation_replay(&existing, &context, &request_hash)?;
        return refresh_machine_cancellation_provider_evidence(
            database,
            machine_port,
            &context,
            existing,
        )
        .await;
    }

    let operation = find_operation(database, tenant_id, input.operation_id).await?;
    let requested_by_owner = operation.requested_by_actor_kind == actor_kind(&context)
        && operation.requested_by_actor_id == context.actor.id;
    if !requested_by_owner
        && security.get_scope(Resource::Translations, Action::Manage) == PermissionScope::None
    {
        return Err(TranslationError::Forbidden);
    }
    if operation.status == "cancelled" {
        let existing = find_cancellation_by_operation(database, tenant_id, operation.id)
            .await?
            .ok_or(TranslationError::WorkflowRevisionConflict)?;
        return replay_machine_cancellation(existing, &context, &request_hash);
    }
    if operation.status != "registered" {
        return Err(TranslationError::MachineOperationTerminal(operation.status));
    }
    let provider_evidence =
        propagate_machine_cancellation(machine_port, &context, &operation).await;

    let transaction = database.begin().await?;
    let cancellation_id = generate_id();
    let now = Utc::now().fixed_offset();
    machine_cancellation::Entity::insert(machine_cancellation::ActiveModel {
        id: Set(cancellation_id),
        tenant_id: Set(tenant_id),
        operation_id: Set(operation.id),
        reason: Set(input.reason),
        requested_by_actor_kind: Set(actor_kind(&context).to_string()),
        requested_by_actor_id: Set(context.actor.id.clone()),
        idempotency_key: Set(idempotency_key.clone()),
        request_hash: Set(request_hash.clone()),
        provider_execution_id: Set(provider_evidence.execution_id),
        provider_status: Set(provider_evidence.status),
        provider_error_code: Set(provider_evidence.error_code),
        provider_observed_at: Set(provider_evidence.observed_at),
        created_at: Set(now),
    })
    .on_conflict(OnConflict::new().do_nothing().to_owned())
    .exec_without_returning(&transaction)
    .await?;
    let persisted = if let Some(cancellation) =
        find_cancellation_by_idempotency(&transaction, tenant_id, &idempotency_key).await?
    {
        cancellation
    } else {
        transaction.rollback().await?;
        let existing = find_cancellation_by_operation(database, tenant_id, operation.id)
            .await?
            .ok_or(TranslationError::WorkflowRevisionConflict)?;
        return replay_machine_cancellation(existing, &context, &request_hash);
    };
    validate_machine_cancellation_replay(&persisted, &context, &request_hash)?;
    let update = machine_operation::Entity::update_many()
        .col_expr(machine_operation::Column::Status, Expr::value("cancelled"))
        .col_expr(machine_operation::Column::UpdatedAt, Expr::value(now))
        .filter(machine_operation::Column::TenantId.eq(tenant_id))
        .filter(machine_operation::Column::Id.eq(operation.id))
        .filter(machine_operation::Column::Status.eq("registered"))
        .exec(&transaction)
        .await?;
    if update.rows_affected != 1 {
        transaction.rollback().await?;
        let current = find_operation(database, tenant_id, operation.id).await?;
        return Err(TranslationError::MachineOperationTerminal(current.status));
    }
    machine_memory_binding::Entity::delete_many()
        .filter(machine_memory_binding::Column::TenantId.eq(tenant_id))
        .filter(machine_memory_binding::Column::OperationId.eq(operation.id))
        .exec(&transaction)
        .await?;
    transaction.commit().await?;
    Ok(machine_cancellation_record(persisted))
}

pub async fn read_machine_operation_status(
    database: &DatabaseConnection,
    machine_port: Option<&dyn MachineTranslationPort>,
    context: PortContext,
    operation_id: Uuid,
) -> TranslationResult<MachineOperationStatusRecord> {
    context.require_policy(PortCallPolicy::read())?;
    let tenant_id =
        Uuid::parse_str(&context.tenant_id).map_err(|_| TranslationError::InvalidTenantId)?;
    let security = SecurityContext::try_from_port_context(&context)?;
    if security.get_scope(Resource::Translations, Action::Read) == PermissionScope::None {
        return Err(TranslationError::Forbidden);
    }
    let operation = find_operation(database, tenant_id, operation_id).await?;
    if operation.status == "completed" {
        return Ok(MachineOperationStatusRecord {
            operation_id: operation.id,
            item_id: operation.item_id,
            status: operation.status,
            provider_execution_id: operation.execution_id,
            provider_status: "completed".to_string(),
            provider_error_code: None,
            updated_at: operation.updated_at,
        });
    }
    if operation.status == "cancelled" {
        let cancellation = find_cancellation_by_operation(database, tenant_id, operation.id)
            .await?
            .ok_or(TranslationError::WorkflowRevisionConflict)?;
        return Ok(MachineOperationStatusRecord {
            operation_id: operation.id,
            item_id: operation.item_id,
            status: operation.status,
            provider_execution_id: cancellation.provider_execution_id,
            provider_status: cancellation.provider_status,
            provider_error_code: cancellation.provider_error_code,
            updated_at: operation.updated_at,
        });
    }
    let Some(machine_port) = machine_port else {
        return Ok(MachineOperationStatusRecord {
            operation_id: operation.id,
            item_id: operation.item_id,
            status: operation.status,
            provider_execution_id: None,
            provider_status: "unavailable".to_string(),
            provider_error_code: None,
            updated_at: operation.updated_at,
        });
    };
    let execution_idempotency_key =
        child_idempotency_key(&operation.idempotency_key, "machine-port")?;
    let (provider_execution_id, provider_status, provider_error_code) = match machine_port
        .execution_status(context, execution_idempotency_key)
        .await
    {
        Ok(evidence) => (
            evidence.execution_id,
            machine_execution_status(evidence.status).to_string(),
            None,
        ),
        Err(error) => (
            None,
            "unavailable".to_string(),
            Some(error.code.chars().take(128).collect()),
        ),
    };
    Ok(MachineOperationStatusRecord {
        operation_id: operation.id,
        item_id: operation.item_id,
        status: operation.status,
        provider_execution_id,
        provider_status,
        provider_error_code,
        updated_at: operation.updated_at,
    })
}

pub(crate) fn validate_machine_cancellation_reason(reason: &str) -> TranslationResult<()> {
    if reason.trim().is_empty() || reason.trim() != reason || reason.len() > 4_096 {
        return Err(TranslationError::InvalidMachineCancellationReason);
    }
    Ok(())
}

pub(crate) fn validate_machine_recovery_reason(reason: &str) -> TranslationResult<()> {
    if reason.trim().is_empty() || reason.trim() != reason || reason.len() > 4_096 {
        return Err(TranslationError::InvalidMachineRecoveryReason);
    }
    Ok(())
}

pub(crate) fn validate_recovery_proposal_input(
    operation: &machine_operation::Model,
    input: &GenerateMachineProposalInput,
) -> TranslationResult<()> {
    if operation.item_id != input.item_id {
        return Err(TranslationError::IdempotencyConflict);
    }
    if operation.command_hash != hash_manifest(input)? {
        return Err(TranslationError::IdempotencyConflict);
    }
    Ok(())
}

pub(crate) fn validate_machine_recovery_replay(
    recovery: &machine_recovery::Model,
    context: &PortContext,
    request_hash: &str,
) -> TranslationResult<()> {
    if recovery.request_hash != request_hash {
        return Err(TranslationError::IdempotencyConflict);
    }
    if recovery.requested_by_actor_kind != actor_kind(context)
        || recovery.requested_by_actor_id != context.actor.id
    {
        return Err(TranslationError::IdempotencyActorMismatch);
    }
    Ok(())
}

pub(crate) fn replay_machine_cancellation(
    model: machine_cancellation::Model,
    context: &PortContext,
    request_hash: &str,
) -> TranslationResult<MachineCancellationRecord> {
    validate_machine_cancellation_replay(&model, context, request_hash)?;
    Ok(machine_cancellation_record(model))
}

pub(crate) fn validate_machine_cancellation_replay(
    model: &machine_cancellation::Model,
    context: &PortContext,
    request_hash: &str,
) -> TranslationResult<()> {
    if model.idempotency_key != context.idempotency_key.as_deref().unwrap_or_default() {
        return Err(TranslationError::MachineOperationTerminal(
            "cancelled".to_string(),
        ));
    }
    if model.request_hash != request_hash {
        return Err(TranslationError::IdempotencyConflict);
    }
    if model.requested_by_actor_kind != actor_kind(context)
        || model.requested_by_actor_id != context.actor.id
    {
        return Err(TranslationError::IdempotencyActorMismatch);
    }
    Ok(())
}

pub(crate) fn machine_cancellation_record(
    model: machine_cancellation::Model,
) -> MachineCancellationRecord {
    MachineCancellationRecord {
        cancellation_id: model.id,
        operation_id: model.operation_id,
        status: "cancelled".to_string(),
        provider_execution_id: model.provider_execution_id,
        provider_status: model.provider_status,
        provider_error_code: model.provider_error_code,
        provider_observed_at: model.provider_observed_at,
        created_at: model.created_at,
    }
}

pub(crate) struct ProviderCancellationEvidence {
    pub(crate) execution_id: Option<String>,
    pub(crate) status: String,
    pub(crate) error_code: Option<String>,
    pub(crate) observed_at: DateTime<FixedOffset>,
}

pub(crate) async fn propagate_machine_cancellation(
    machine_port: Option<&dyn MachineTranslationPort>,
    context: &PortContext,
    operation: &machine_operation::Model,
) -> ProviderCancellationEvidence {
    let observed_at = Utc::now().fixed_offset();
    let Some(machine_port) = machine_port else {
        return ProviderCancellationEvidence {
            execution_id: None,
            status: "unavailable".to_string(),
            error_code: None,
            observed_at,
        };
    };
    let execution_idempotency_key =
        match child_idempotency_key(&operation.idempotency_key, "machine-port") {
            Ok(key) => key,
            Err(_) => {
                return ProviderCancellationEvidence {
                    execution_id: None,
                    status: "propagation_failed".to_string(),
                    error_code: Some("translation.machine.cancellation_key_invalid".to_string()),
                    observed_at,
                };
            }
        };
    match machine_port
        .cancel_execution(context.clone(), execution_idempotency_key)
        .await
    {
        Ok(evidence) => ProviderCancellationEvidence {
            execution_id: evidence.execution_id,
            status: provider_cancellation_status(evidence.status).to_string(),
            error_code: None,
            observed_at,
        },
        Err(error) => ProviderCancellationEvidence {
            execution_id: None,
            status: "propagation_failed".to_string(),
            error_code: Some(error.code.chars().take(128).collect()),
            observed_at,
        },
    }
}

pub(crate) fn provider_cancellation_status(
    status: MachineTranslationExecutionStatus,
) -> &'static str {
    match status {
        MachineTranslationExecutionStatus::NotRegistered
        | MachineTranslationExecutionStatus::Queued
        | MachineTranslationExecutionStatus::Running
        | MachineTranslationExecutionStatus::CancellationRequested => "cancellation_requested",
        MachineTranslationExecutionStatus::Completed => "completed",
        MachineTranslationExecutionStatus::Failed => "failed",
        MachineTranslationExecutionStatus::Cancelled => "cancelled",
    }
}

pub(crate) fn machine_execution_status(status: MachineTranslationExecutionStatus) -> &'static str {
    match status {
        MachineTranslationExecutionStatus::NotRegistered => "not_registered",
        MachineTranslationExecutionStatus::Queued => "queued",
        MachineTranslationExecutionStatus::Running => "running",
        MachineTranslationExecutionStatus::CancellationRequested => "cancellation_requested",
        MachineTranslationExecutionStatus::Completed => "completed",
        MachineTranslationExecutionStatus::Failed => "failed",
        MachineTranslationExecutionStatus::Cancelled => "cancelled",
    }
}

pub(crate) async fn refresh_machine_cancellation_provider_evidence(
    database: &DatabaseConnection,
    machine_port: Option<&dyn MachineTranslationPort>,
    context: &PortContext,
    existing: machine_cancellation::Model,
) -> TranslationResult<MachineCancellationRecord> {
    if machine_port.is_none()
        || matches!(
            existing.provider_status.as_str(),
            "completed" | "failed" | "cancelled"
        )
    {
        return Ok(machine_cancellation_record(existing));
    }
    let operation = find_operation(database, existing.tenant_id, existing.operation_id).await?;
    let evidence = propagate_machine_cancellation(machine_port, context, &operation).await;
    machine_cancellation::Entity::update_many()
        .col_expr(
            machine_cancellation::Column::ProviderExecutionId,
            Expr::value(evidence.execution_id),
        )
        .col_expr(
            machine_cancellation::Column::ProviderStatus,
            Expr::value(evidence.status),
        )
        .col_expr(
            machine_cancellation::Column::ProviderErrorCode,
            Expr::value(evidence.error_code),
        )
        .col_expr(
            machine_cancellation::Column::ProviderObservedAt,
            Expr::value(evidence.observed_at),
        )
        .filter(machine_cancellation::Column::TenantId.eq(existing.tenant_id))
        .filter(machine_cancellation::Column::Id.eq(existing.id))
        .exec(database)
        .await?;
    find_cancellation_by_idempotency(database, existing.tenant_id, &existing.idempotency_key)
        .await?
        .map(machine_cancellation_record)
        .ok_or(TranslationError::WorkflowRevisionConflict)
}
