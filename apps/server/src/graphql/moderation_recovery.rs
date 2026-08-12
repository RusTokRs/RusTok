use std::time::Duration;

use async_graphql::{Context, FieldError, Json, Object, Result, SimpleObject};
use rustok_api::graphql::{GraphQLError, require_module_enabled};
use rustok_api::{
    AuthContext, ChannelContext, Permission, PortContext, PortError, PortErrorKind, RequestContext,
    TenantContext, has_effective_permission,
};
use rustok_moderation::{
    AssignModerationCaseCommand, DecideModerationCaseCommand, ModerationApplicationRecoveryRecord,
    ModerationCaseRecord, ModerationCaseStatus, ModerationCommandPort, ModerationDecisionEffect,
    ModerationDecisionKind, ModerationReadPort, ModerationReasonCode,
    ModerationRecoveryCommandPort, ModerationService, OpenModerationCaseCommand,
    ReconcileLegacyModerationApplicationCommand, RequeueModerationApplicationCommand,
};
use sea_orm::DatabaseConnection;
use serde_json::{Value as JsonValue, json};
use sha2::{Digest, Sha256};
use uuid::Uuid;

const MODULE_SLUG: &str = "moderation";
const RECOVERY_PORT_DEADLINE: Duration = Duration::from_secs(5);
const MAX_REREVIEW_REASON_BYTES: usize = 1_000;
const REREVIEW_METADATA_KEY: &str = "operator_rereview";

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

    /// Create a new case and immutable decision for a producer-supplied fresh subject revision.
    ///
    /// The historical case/decision are never mutated or retargeted. Subject identity, scope,
    /// queue and policy routing are copied from the source case; only the monotonic subject
    /// revision and the newly reviewed decision/effect/policy snapshot come from this request.
    async fn create_moderation_rereview(
        &self,
        ctx: &Context<'_>,
        idempotency_key: Uuid,
        source_decision_id: Uuid,
        fresh_subject_revision: i64,
        rereview_reason: String,
        decision_kind: String,
        reason_code: String,
        effect: Json<JsonValue>,
        policy_snapshot: Json<JsonValue>,
    ) -> Result<ModerationRereviewPayload> {
        let auth = require_recovery_authority(ctx)?;
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let service = moderation_service(ctx)?;
        let base_context = recovery_port_context(ctx, auth, idempotency_key)?;

        let rereview_reason = normalize_rereview_reason(rereview_reason)?;
        let decision_kind = parse_decision_kind(decision_kind)?;
        let reason_code = parse_reason_code(reason_code)?;
        let effect = parse_decision_effect(effect.0, decision_kind)?;
        let policy_snapshot = policy_snapshot.0;
        if !policy_snapshot.is_object() {
            return Err(<FieldError as GraphQLError>::bad_user_input(
                "moderation rereview policy snapshot must be a JSON object",
            ));
        }
        let request_hash = rereview_request_hash(
            source_decision_id,
            fresh_subject_revision,
            rereview_reason.as_str(),
            decision_kind,
            reason_code,
            &effect,
            &policy_snapshot,
        )?;

        let source_decision =
            ModerationReadPort::read_decision(&service, base_context.clone(), source_decision_id)
                .await
                .map_err(map_port_error)?
                .ok_or_else(|| {
                    <FieldError as GraphQLError>::not_found(
                        "source moderation decision was not found",
                    )
                })?;
        let source_case =
            ModerationReadPort::read_case(&service, base_context.clone(), source_decision.case_id)
                .await
                .map_err(map_port_error)?
                .ok_or_else(|| {
                    <FieldError as GraphQLError>::internal_error(
                        "source moderation decision references a missing case",
                    )
                })?;

        validate_rereview_source(
            &source_case,
            source_decision.case_id,
            source_decision.subject_revision,
            fresh_subject_revision,
        )?;

        let metadata = rereview_metadata(
            idempotency_key,
            &source_case,
            source_decision_id,
            fresh_subject_revision,
            rereview_reason.as_str(),
            request_hash.as_str(),
        );
        let mut fresh_subject = source_case.subject.clone();
        fresh_subject.revision = fresh_subject_revision;

        let opened = ModerationCommandPort::open_case(
            &service,
            rereview_step_context(&base_context, idempotency_key, "open"),
            OpenModerationCaseCommand {
                scope: source_case.scope.clone(),
                subject: fresh_subject,
                queue_key: source_case.queue_key.clone(),
                priority: source_case.priority,
                policy_id: source_case.policy_id,
                policy_version: source_case.policy_version,
                report_ids: Vec::new(),
                metadata,
            },
        )
        .await
        .map_err(map_port_error)?;

        require_owned_rereview_case(
            &opened,
            idempotency_key,
            source_case.id,
            source_decision_id,
            fresh_subject_revision,
            request_hash.as_str(),
        )?;

        let assigned = ModerationCommandPort::assign_case(
            &service,
            rereview_step_context(&base_context, idempotency_key, "assign"),
            AssignModerationCaseCommand {
                case_id: opened.id,
                expected_revision: opened.revision,
                moderator_id: auth.user_id,
            },
        )
        .await
        .map_err(map_port_error)?;

        let decision = ModerationCommandPort::decide_case(
            &service,
            rereview_step_context(&base_context, idempotency_key, "decide"),
            DecideModerationCaseCommand {
                case_id: assigned.id,
                expected_revision: assigned.revision,
                decision_kind,
                reason_code,
                effect,
                policy_snapshot,
            },
        )
        .await
        .map_err(map_port_error)?;

        Ok(ModerationRereviewPayload {
            source_case_id: source_case.id,
            source_decision_id,
            case_id: assigned.id,
            decision_id: decision.id,
            subject_revision: fresh_subject_revision,
            decision_kind: decision.decision_kind.as_str().to_string(),
            decision_hash: decision.decision_hash,
        })
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

#[derive(SimpleObject)]
pub struct ModerationRereviewPayload {
    pub source_case_id: Uuid,
    pub source_decision_id: Uuid,
    pub case_id: Uuid,
    pub decision_id: Uuid,
    pub subject_revision: i64,
    pub decision_kind: String,
    pub decision_hash: String,
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
        <FieldError as GraphQLError>::internal_error("Moderation tenant context is not registered")
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
        <FieldError as GraphQLError>::internal_error("Moderation tenant context is not registered")
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
        format!("graphql-moderation-recovery-{idempotency_key}"),
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

fn rereview_step_context(
    base: &PortContext,
    root_idempotency_key: Uuid,
    step: &str,
) -> PortContext {
    let mut context = base.clone();
    context.idempotency_key = Some(format!("{root_idempotency_key}:rereview:{step}"));
    context.correlation_id = format!("graphql-moderation-rereview-{root_idempotency_key}");
    context
}

fn normalize_rereview_reason(reason: String) -> Result<String> {
    let reason = reason.trim().to_string();
    if reason.is_empty() || reason.len() > MAX_REREVIEW_REASON_BYTES {
        return Err(<FieldError as GraphQLError>::bad_user_input(format!(
            "moderation rereview reason must contain 1 to {MAX_REREVIEW_REASON_BYTES} bytes",
        )));
    }
    Ok(reason)
}

fn parse_decision_kind(value: String) -> Result<ModerationDecisionKind> {
    let value = value.trim().to_ascii_lowercase();
    ModerationDecisionKind::parse(value.as_str()).ok_or_else(|| {
        <FieldError as GraphQLError>::bad_user_input("unknown moderation rereview decision kind")
    })
}

fn parse_reason_code(value: String) -> Result<ModerationReasonCode> {
    let value = value.trim().to_ascii_lowercase();
    ModerationReasonCode::parse(value.as_str()).ok_or_else(|| {
        <FieldError as GraphQLError>::bad_user_input("unknown moderation rereview reason code")
    })
}

fn parse_decision_effect(
    value: JsonValue,
    decision_kind: ModerationDecisionKind,
) -> Result<ModerationDecisionEffect> {
    let effect = serde_json::from_value::<ModerationDecisionEffect>(value).map_err(|_| {
        <FieldError as GraphQLError>::bad_user_input("invalid moderation rereview decision effect")
    })?;
    effect
        .validate_for_decision_kind(decision_kind)
        .map_err(|error| <FieldError as GraphQLError>::bad_user_input(&error.to_string()))?;
    Ok(effect)
}

fn rereview_request_hash(
    source_decision_id: Uuid,
    fresh_subject_revision: i64,
    rereview_reason: &str,
    decision_kind: ModerationDecisionKind,
    reason_code: ModerationReasonCode,
    effect: &ModerationDecisionEffect,
    policy_snapshot: &JsonValue,
) -> Result<String> {
    let payload = json!({
        "version": 1,
        "source_decision_id": source_decision_id,
        "fresh_subject_revision": fresh_subject_revision,
        "rereview_reason": rereview_reason,
        "decision_kind": decision_kind,
        "reason_code": reason_code,
        "effect": effect,
        "policy_snapshot": policy_snapshot,
    });
    let encoded = serde_json::to_vec(&payload).map_err(|_| {
        <FieldError as GraphQLError>::internal_error(
            "Moderation rereview request could not be normalized",
        )
    })?;
    Ok(hex::encode(Sha256::digest(encoded)))
}

fn validate_rereview_source(
    source_case: &ModerationCaseRecord,
    decision_case_id: Uuid,
    decision_subject_revision: i64,
    fresh_subject_revision: i64,
) -> Result<()> {
    if source_case.id != decision_case_id
        || source_case.subject.revision != decision_subject_revision
    {
        return Err(<FieldError as GraphQLError>::internal_error(
            "source moderation decision/case identity is inconsistent",
        ));
    }
    if source_case.status != ModerationCaseStatus::Escalated {
        return Err(<FieldError as GraphQLError>::bad_user_input(
            "moderation rereview requires an escalated source case",
        ));
    }
    if fresh_subject_revision <= source_case.subject.revision {
        return Err(<FieldError as GraphQLError>::bad_user_input(
            "moderation rereview requires a subject revision newer than the historical review",
        ));
    }
    Ok(())
}

fn rereview_metadata(
    root_idempotency_key: Uuid,
    source_case: &ModerationCaseRecord,
    source_decision_id: Uuid,
    fresh_subject_revision: i64,
    reason: &str,
    request_hash: &str,
) -> JsonValue {
    json!({
        "operator_rereview": {
            "root_idempotency_key": root_idempotency_key,
            "request_hash": request_hash,
            "source_case_id": source_case.id,
            "source_decision_id": source_decision_id,
            "source_subject_revision": source_case.subject.revision,
            "fresh_subject_revision": fresh_subject_revision,
            "reason": reason,
        }
    })
}

fn require_owned_rereview_case(
    case: &ModerationCaseRecord,
    root_idempotency_key: Uuid,
    source_case_id: Uuid,
    source_decision_id: Uuid,
    fresh_subject_revision: i64,
    request_hash: &str,
) -> Result<()> {
    let marker = case
        .metadata
        .get(REREVIEW_METADATA_KEY)
        .and_then(JsonValue::as_object)
        .ok_or_else(|| {
            <FieldError as GraphQLError>::bad_user_input(
                "an existing active moderation case already owns this fresh subject revision",
            )
        })?;

    let expected = [
        ("root_idempotency_key", root_idempotency_key.to_string()),
        ("request_hash", request_hash.to_string()),
        ("source_case_id", source_case_id.to_string()),
        ("source_decision_id", source_decision_id.to_string()),
        ("fresh_subject_revision", fresh_subject_revision.to_string()),
    ];
    let matches = expected.iter().all(|(field, expected)| {
        marker
            .get(*field)
            .map(|value| match value {
                JsonValue::String(value) => value == expected,
                JsonValue::Number(value) => value.to_string() == *expected,
                _ => false,
            })
            .unwrap_or(false)
    });
    if !matches {
        return Err(<FieldError as GraphQLError>::bad_user_input(
            "an existing active moderation case already owns this fresh subject revision",
        ));
    }
    Ok(())
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
    use rustok_moderation::{
        ModerationCasePriority, ModerationScopeRef, ModerationSubjectKind, ModerationSubjectRef,
    };

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

    #[test]
    fn rereview_step_keys_are_stable_and_distinct() {
        let root = Uuid::from_u128(1);
        let base = PortContext::new(
            "tenant",
            rustok_api::PortActor::user(Uuid::from_u128(2).to_string()),
            "en",
            "corr",
        )
        .with_deadline(RECOVERY_PORT_DEADLINE)
        .with_idempotency_key(root.to_string());
        assert_eq!(
            rereview_step_context(&base, root, "open")
                .idempotency_key
                .as_deref(),
            Some("00000000-0000-0000-0000-000000000001:rereview:open")
        );
        assert_ne!(
            rereview_step_context(&base, root, "assign").idempotency_key,
            rereview_step_context(&base, root, "decide").idempotency_key,
        );
    }

    #[test]
    fn rereview_request_hash_changes_with_decision_payload() {
        let source_decision_id = Uuid::from_u128(3);
        let effect = ModerationDecisionEffect::v1(
            rustok_moderation::ModerationDecisionEffectAction::NoDomainMutation,
        )
        .unwrap();
        let first = rereview_request_hash(
            source_decision_id,
            11,
            "fresh review",
            ModerationDecisionKind::Warning,
            ModerationReasonCode::Other,
            &effect,
            &json!({"policy": 1}),
        )
        .unwrap();
        let second = rereview_request_hash(
            source_decision_id,
            11,
            "fresh review",
            ModerationDecisionKind::Warning,
            ModerationReasonCode::Other,
            &effect,
            &json!({"policy": 2}),
        )
        .unwrap();
        assert_ne!(first, second);
    }

    #[test]
    fn rereview_metadata_proves_case_ownership() {
        let source_case_id = Uuid::from_u128(3);
        let source_decision_id = Uuid::from_u128(4);
        let root = Uuid::from_u128(5);
        let source_case = ModerationCaseRecord {
            id: source_case_id,
            tenant_id: Uuid::from_u128(6),
            scope: ModerationScopeRef::platform(),
            subject: ModerationSubjectRef {
                module: "forum".to_string(),
                kind: ModerationSubjectKind::ForumPost,
                id: Uuid::from_u128(7),
                revision: 10,
            },
            queue_key: "content".to_string(),
            policy_id: None,
            policy_version: 1,
            priority: ModerationCasePriority::Normal,
            status: ModerationCaseStatus::Escalated,
            assigned_moderator_id: None,
            revision: 4,
            metadata: json!({}),
            opened_at: chrono::Utc::now(),
            started_at: None,
            decided_at: None,
            closed_at: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        let request_hash = "request-hash";
        let case = ModerationCaseRecord {
            subject: ModerationSubjectRef {
                revision: 11,
                ..source_case.subject.clone()
            },
            metadata: rereview_metadata(
                root,
                &source_case,
                source_decision_id,
                11,
                "fresh review",
                request_hash,
            ),
            ..source_case.clone()
        };
        assert!(
            require_owned_rereview_case(
                &case,
                root,
                source_case_id,
                source_decision_id,
                11,
                request_hash,
            )
            .is_ok()
        );
        assert!(
            require_owned_rereview_case(
                &case,
                Uuid::from_u128(8),
                source_case_id,
                source_decision_id,
                11,
                request_hash,
            )
            .is_err()
        );
    }
}
