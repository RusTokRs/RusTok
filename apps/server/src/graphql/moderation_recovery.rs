use std::time::Duration;

use async_graphql::{Context, FieldError, Object, Result, SimpleObject};
use rustok_api::graphql::{GraphQLError, require_module_enabled};
use rustok_api::{
    AuthContext, ChannelContext, Permission, PortContext, PortError, PortErrorKind, RequestContext,
    TenantContext, has_effective_permission,
};
use rustok_moderation::{
    ModerationApplicationRecoveryRecord, ModerationRecoveryCommandPort, ModerationService,
    ReconcileLegacyModerationApplicationCommand, RequeueModerationApplicationCommand,
};
use sea_orm::DatabaseConnection;
use uuid::Uuid;

const MODULE_SLUG: &str = "moderation";
const RECOVERY_PORT_DEADLINE: Duration = Duration::from_secs(5);

#[derive(Default)]
pub struct ModerationRecoveryMutation;

#[Object]
impl ModerationRecoveryMutation {
    /// Requeue the same immutable decision after an explicit operator review.
    async fn requeue_moderation_application(
        &self,
        ctx: &Context<'_>,
        idempotency_key: Uuid,
        decision_id: Uuid,
        expected_case_revision: i64,
        reason: String,
    ) -> Result<ModerationApplicationRecoveryPayload> {
        let auth = require_recovery_authority(ctx)?;
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let service = moderation_service(ctx)?;

        ModerationRecoveryCommandPort::requeue_application(
            &service,
            recovery_port_context(ctx, auth, idempotency_key)?,
            RequeueModerationApplicationCommand {
                decision_id,
                expected_case_revision,
                reason,
            },
        )
        .await
        .map(Into::into)
        .map_err(map_port_error)
    }

    /// Align a legacy terminal application with its Moderation case without invoking a domain adapter.
    async fn reconcile_legacy_moderation_application(
        &self,
        ctx: &Context<'_>,
        idempotency_key: Uuid,
        decision_id: Uuid,
        expected_case_revision: i64,
        reason: String,
    ) -> Result<ModerationApplicationRecoveryPayload> {
        let auth = require_recovery_authority(ctx)?;
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let service = moderation_service(ctx)?;

        ModerationRecoveryCommandPort::reconcile_legacy_application(
            &service,
            recovery_port_context(ctx, auth, idempotency_key)?,
            ReconcileLegacyModerationApplicationCommand {
                decision_id,
                expected_case_revision,
                reason,
            },
        )
        .await
        .map(Into::into)
        .map_err(map_port_error)
    }
}

#[derive(SimpleObject)]
pub struct ModerationApplicationRecoveryPayload {
    pub decision_id: Uuid,
    pub case_id: Uuid,
    pub operation_status: String,
    pub case_status: String,
    pub case_revision: i64,
    pub changed: bool,
}

impl From<ModerationApplicationRecoveryRecord> for ModerationApplicationRecoveryPayload {
    fn from(value: ModerationApplicationRecoveryRecord) -> Self {
        Self {
            decision_id: value.decision_id,
            case_id: value.case_id,
            operation_status: value.operation_status.as_str().to_string(),
            case_status: value.case_status.as_str().to_string(),
            case_revision: value.case_revision,
            changed: value.changed,
        }
    }
}

fn moderation_service(ctx: &Context<'_>) -> Result<ModerationService> {
    Ok(ModerationService::new(
        ctx.data::<DatabaseConnection>()?.clone(),
    ))
}

fn require_recovery_authority<'a>(ctx: &'a Context<'a>) -> Result<&'a AuthContext> {
    let auth = ctx
        .data::<AuthContext>()
        .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
    let tenant = ctx.data::<TenantContext>().map_err(|_| {
        <FieldError as GraphQLError>::internal_error(
            "Moderation tenant context is not registered",
        )
    })?;

    if auth.tenant_id != tenant.id {
        return Err(<FieldError as GraphQLError>::permission_denied(
            "moderation tenant mismatch",
        ));
    }
    if !auth.is_human_user_principal() {
        return Err(<FieldError as GraphQLError>::permission_denied(
            "moderation application recovery requires a human operator",
        ));
    }
    if !has_recovery_permission(&auth.permissions) {
        return Err(<FieldError as GraphQLError>::permission_denied(
            "Permission denied: moderation_cases:override required",
        ));
    }

    Ok(auth)
}

fn has_recovery_permission(permissions: &[Permission]) -> bool {
    has_effective_permission(permissions, &Permission::MODERATION_CASES_OVERRIDE)
}

fn recovery_port_context(
    ctx: &Context<'_>,
    auth: &AuthContext,
    idempotency_key: Uuid,
) -> Result<PortContext> {
    if idempotency_key.is_nil() {
        return Err(<FieldError as GraphQLError>::bad_user_input(
            "moderation recovery idempotency key must not be nil",
        ));
    }

    let tenant = ctx.data::<TenantContext>().map_err(|_| {
        <FieldError as GraphQLError>::internal_error(
            "Moderation tenant context is not registered",
        )
    })?;
    let locale = ctx
        .data_opt::<RequestContext>()
        .map(|request| request.locale.clone())
        .filter(|locale| !locale.trim().is_empty())
        .unwrap_or_else(|| tenant.default_locale.clone());

    let mut context = PortContext::new(
        tenant.id.to_string(),
        auth.port_actor(),
        locale,
        format!("graphql-moderation-recovery-{}", Uuid::new_v4()),
    )
    .with_deadline(RECOVERY_PORT_DEADLINE)
    .with_idempotency_key(idempotency_key.to_string());

    for permission in &auth.permissions {
        context = context.with_claim(permission.to_string());
    }
    if let Some(channel) = ctx.data_opt::<ChannelContext>() {
        context = context.with_channel(channel.slug.clone());
    }

    Ok(context)
}

fn map_port_error(error: PortError) -> FieldError {
    match error.kind {
        PortErrorKind::Validation | PortErrorKind::Conflict => {
            <FieldError as GraphQLError>::bad_user_input(&error.message)
        }
        PortErrorKind::NotFound => <FieldError as GraphQLError>::not_found(&error.message),
        PortErrorKind::Forbidden => <FieldError as GraphQLError>::permission_denied(&error.message),
        PortErrorKind::Unavailable | PortErrorKind::Timeout => {
            <FieldError as GraphQLError>::internal_error(
                "Moderation recovery service is temporarily unavailable",
            )
        }
        PortErrorKind::InvariantViolation => <FieldError as GraphQLError>::internal_error(
            "Moderation recovery operation requires operator review",
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recovery_permission_is_not_inherited_from_forum_moderation() {
        assert!(has_recovery_permission(&[
            Permission::MODERATION_CASES_OVERRIDE
        ]));
        assert!(has_recovery_permission(&[
            Permission::MODERATION_CASES_MANAGE
        ]));
        assert!(!has_recovery_permission(&[
            Permission::FORUM_TOPICS_MODERATE,
            Permission::FORUM_REPLIES_MODERATE,
        ]));
    }
}
