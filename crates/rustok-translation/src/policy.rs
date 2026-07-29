use std::{collections::BTreeSet, sync::Arc};

use chrono::Utc;
use rustok_api::{
    Action, PortActorKind, PortCallPolicy, PortContext, Resource, TenantLocale,
    manifest_hash::hash_manifest,
};
use rustok_core::{PermissionScope, SecurityContext, generate_id};
use rustok_tenant::{TenantLocalePolicyPort, TenantLocalePolicyProjection};
use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set, TransactionTrait,
    sea_query::{Expr, OnConflict},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    TranslationError, TranslationResult,
    entities::{policy, policy_receipt},
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplaceRequiredTargetLocalesInput {
    pub expected_revision: i64,
    pub required_target_locales: Vec<TenantLocale>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TranslationPolicyRecord {
    pub tenant_id: Uuid,
    pub required_target_locales: Vec<TenantLocale>,
    pub tenant_locale_policy_revision: i64,
    pub revision: i64,
    pub freshness: TranslationPolicyFreshness,
    pub disabled_required_target_locales: Vec<TenantLocale>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TranslationPolicyFreshness {
    Current,
    Stale,
}

pub struct TranslationPolicyService {
    database: DatabaseConnection,
    tenant_locale_policies: Arc<dyn TenantLocalePolicyPort>,
}

impl TranslationPolicyService {
    pub fn new(
        database: DatabaseConnection,
        tenant_locale_policies: Arc<dyn TenantLocalePolicyPort>,
    ) -> Self {
        Self {
            database,
            tenant_locale_policies,
        }
    }

    pub async fn read_policy(
        &self,
        context: PortContext,
    ) -> TranslationResult<TranslationPolicyRecord> {
        let tenant_id = authorize(&context, Action::Read, PortCallPolicy::read())?;
        let tenant_policy = read_validated_tenant_locale_policy(
            self.tenant_locale_policies.as_ref(),
            context,
            tenant_id,
        )
        .await?;
        let Some(model) = policy::Entity::find_by_id(tenant_id)
            .one(&self.database)
            .await?
        else {
            return Ok(TranslationPolicyRecord {
                tenant_id,
                required_target_locales: Vec::new(),
                tenant_locale_policy_revision: tenant_policy.revision,
                revision: 0,
                freshness: TranslationPolicyFreshness::Current,
                disabled_required_target_locales: Vec::new(),
            });
        };
        policy_record(model, &tenant_policy)
    }

    pub async fn replace_required_target_locales(
        &self,
        context: PortContext,
        mut input: ReplaceRequiredTargetLocalesInput,
    ) -> TranslationResult<TranslationPolicyRecord> {
        let tenant_id = authorize(&context, Action::Manage, PortCallPolicy::write())?;
        if input.expected_revision < 0 {
            return Err(TranslationError::TranslationPolicyConflict {
                expected: input.expected_revision,
                actual: 0,
            });
        }
        normalize_required_target_locales(&mut input.required_target_locales)?;
        let request_hash = hash_manifest(&input)?;
        let idempotency_key = context.idempotency_key.clone().unwrap_or_default();
        if let Some(receipt) =
            find_policy_receipt(&self.database, tenant_id, &idempotency_key).await?
        {
            return replay_policy_receipt(receipt, &context, &request_hash);
        }

        let tenant_policy = read_validated_tenant_locale_policy(
            self.tenant_locale_policies.as_ref(),
            context.clone(),
            tenant_id,
        )
        .await?;
        validate_required_target_locales(&input.required_target_locales, &tenant_policy)?;

        let existing = policy::Entity::find_by_id(tenant_id)
            .one(&self.database)
            .await?;
        let actual_revision = existing.as_ref().map_or(0, |model| model.revision);
        if actual_revision != input.expected_revision {
            return Err(TranslationError::TranslationPolicyConflict {
                expected: input.expected_revision,
                actual: actual_revision,
            });
        }
        let next_revision =
            actual_revision
                .checked_add(1)
                .ok_or(TranslationError::TranslationPolicyInvariant(
                    "translation policy revision overflow".to_string(),
                ))?;
        let now = Utc::now().fixed_offset();
        let required_target_locales = serde_json::to_value(&input.required_target_locales)?;
        let record = TranslationPolicyRecord {
            tenant_id,
            required_target_locales: input.required_target_locales,
            tenant_locale_policy_revision: tenant_policy.revision,
            revision: next_revision,
            freshness: TranslationPolicyFreshness::Current,
            disabled_required_target_locales: Vec::new(),
        };
        let response = serde_json::to_value(&record)?;
        let receipt_id = generate_id();
        let transaction = self.database.begin().await?;

        match existing {
            Some(model) => {
                let update = policy::Entity::update_many()
                    .col_expr(
                        policy::Column::RequiredTargetLocales,
                        Expr::value(required_target_locales),
                    )
                    .col_expr(
                        policy::Column::TenantLocalePolicyRevision,
                        Expr::value(tenant_policy.revision),
                    )
                    .col_expr(policy::Column::Revision, Expr::value(next_revision))
                    .col_expr(
                        policy::Column::LastIdempotencyKey,
                        Expr::value(idempotency_key.clone()),
                    )
                    .col_expr(
                        policy::Column::LastRequestHash,
                        Expr::value(request_hash.clone()),
                    )
                    .col_expr(
                        policy::Column::UpdatedByActorKind,
                        Expr::value(actor_kind(&context)),
                    )
                    .col_expr(
                        policy::Column::UpdatedByActorId,
                        Expr::value(context.actor.id.clone()),
                    )
                    .col_expr(policy::Column::UpdatedAt, Expr::value(now))
                    .filter(policy::Column::TenantId.eq(tenant_id))
                    .filter(policy::Column::Revision.eq(model.revision))
                    .exec(&transaction)
                    .await?;
                if update.rows_affected != 1 {
                    transaction.rollback().await?;
                    return Err(TranslationError::TranslationPolicyConflict {
                        expected: input.expected_revision,
                        actual: actual_revision.saturating_add(1),
                    });
                }
            }
            None => {
                policy::Entity::insert(policy::ActiveModel {
                    tenant_id: Set(tenant_id),
                    required_target_locales: Set(required_target_locales),
                    tenant_locale_policy_revision: Set(tenant_policy.revision),
                    revision: Set(next_revision),
                    last_idempotency_key: Set(idempotency_key.clone()),
                    last_request_hash: Set(request_hash.clone()),
                    updated_by_actor_kind: Set(actor_kind(&context).to_string()),
                    updated_by_actor_id: Set(context.actor.id.clone()),
                    updated_at: Set(now),
                })
                .on_conflict(
                    OnConflict::column(policy::Column::TenantId)
                        .do_nothing()
                        .to_owned(),
                )
                .exec_without_returning(&transaction)
                .await?;
                let persisted = policy::Entity::find_by_id(tenant_id)
                    .one(&transaction)
                    .await?
                    .ok_or_else(|| {
                        TranslationError::TranslationPolicyInvariant(
                            "translation policy insert did not persist".to_string(),
                        )
                    })?;
                if persisted.revision != next_revision
                    || persisted.required_target_locales
                        != serde_json::to_value(&record.required_target_locales)?
                    || persisted.last_idempotency_key != idempotency_key
                    || persisted.last_request_hash != request_hash
                {
                    transaction.rollback().await?;
                    return Err(TranslationError::TranslationPolicyConflict {
                        expected: input.expected_revision,
                        actual: persisted.revision,
                    });
                }
            }
        }

        policy_receipt::Entity::insert(policy_receipt::ActiveModel {
            id: Set(receipt_id),
            tenant_id: Set(tenant_id),
            idempotency_key: Set(idempotency_key.clone()),
            request_hash: Set(request_hash.clone()),
            requested_by_actor_kind: Set(actor_kind(&context).to_string()),
            requested_by_actor_id: Set(context.actor.id.clone()),
            resulting_policy_revision: Set(next_revision),
            response: Set(response),
            created_at: Set(now),
        })
        .on_conflict(
            OnConflict::columns([
                policy_receipt::Column::TenantId,
                policy_receipt::Column::IdempotencyKey,
            ])
            .do_nothing()
            .to_owned(),
        )
        .exec_without_returning(&transaction)
        .await?;
        let persisted_receipt = find_policy_receipt(&transaction, tenant_id, &idempotency_key)
            .await?
            .ok_or_else(|| {
                TranslationError::TranslationPolicyInvariant(
                    "translation policy receipt did not persist".to_string(),
                )
            })?;
        if persisted_receipt.id != receipt_id {
            transaction.rollback().await?;
            return replay_policy_receipt(persisted_receipt, &context, &request_hash);
        }
        transaction.commit().await?;
        Ok(record)
    }
}

pub(crate) async fn read_validated_tenant_locale_policy(
    port: &dyn TenantLocalePolicyPort,
    context: PortContext,
    tenant_id: Uuid,
) -> TranslationResult<TenantLocalePolicyProjection> {
    let projection = port.read_locale_policy(context).await?;
    validate_tenant_locale_policy_projection(&projection, tenant_id)?;
    Ok(projection)
}

pub(crate) fn validate_job_locales(
    projection: &TenantLocalePolicyProjection,
    source_locale: &TenantLocale,
    target_locale: &TenantLocale,
) -> TranslationResult<()> {
    let enabled = projection
        .locales
        .iter()
        .filter(|entry| entry.is_enabled)
        .map(|entry| entry.locale.as_str())
        .collect::<BTreeSet<_>>();
    if !enabled.contains(source_locale.as_str()) {
        return Err(TranslationError::DisabledJobLocale {
            role: "source",
            locale: source_locale.as_str().to_string(),
        });
    }
    if !enabled.contains(target_locale.as_str()) {
        return Err(TranslationError::DisabledJobLocale {
            role: "target",
            locale: target_locale.as_str().to_string(),
        });
    }
    Ok(())
}

fn normalize_required_target_locales(locales: &mut Vec<TenantLocale>) -> TranslationResult<()> {
    locales.sort();
    if locales.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(TranslationError::DuplicateRequiredTargetLocale);
    }
    Ok(())
}

fn validate_required_target_locales(
    required: &[TenantLocale],
    projection: &TenantLocalePolicyProjection,
) -> TranslationResult<()> {
    let enabled = projection
        .locales
        .iter()
        .filter(|entry| entry.is_enabled)
        .map(|entry| entry.locale.as_str())
        .collect::<BTreeSet<_>>();
    for locale in required {
        if !enabled.contains(locale.as_str()) {
            return Err(TranslationError::RequiredTargetLocaleDisabled(
                locale.as_str().to_string(),
            ));
        }
    }
    Ok(())
}

fn validate_tenant_locale_policy_projection(
    projection: &TenantLocalePolicyProjection,
    tenant_id: Uuid,
) -> TranslationResult<()> {
    if projection.tenant_id != tenant_id {
        return Err(TranslationError::TranslationPolicyInvariant(
            "tenant locale policy returned another tenant".to_string(),
        ));
    }
    if projection.revision < 0 {
        return Err(TranslationError::TranslationPolicyInvariant(
            "tenant locale policy revision must be non-negative".to_string(),
        ));
    }
    let mut locales = BTreeSet::new();
    let mut default_is_enabled = false;
    for entry in &projection.locales {
        if !locales.insert(entry.locale.as_str()) {
            return Err(TranslationError::TranslationPolicyInvariant(
                "tenant locale policy contains duplicate locales".to_string(),
            ));
        }
        if entry.locale == projection.default_locale {
            default_is_enabled = entry.is_enabled && entry.is_default;
        }
    }
    if !default_is_enabled {
        return Err(TranslationError::TranslationPolicyInvariant(
            "tenant locale policy default locale is not enabled and marked default".to_string(),
        ));
    }
    Ok(())
}

fn policy_record(
    model: policy::Model,
    tenant_policy: &TenantLocalePolicyProjection,
) -> TranslationResult<TranslationPolicyRecord> {
    let required_target_locales: Vec<TenantLocale> =
        serde_json::from_value(model.required_target_locales)?;
    let disabled_required_target_locales =
        disabled_required_target_locales(&required_target_locales, tenant_policy);
    let freshness = if model.tenant_locale_policy_revision == tenant_policy.revision
        && disabled_required_target_locales.is_empty()
    {
        TranslationPolicyFreshness::Current
    } else {
        TranslationPolicyFreshness::Stale
    };
    Ok(TranslationPolicyRecord {
        tenant_id: model.tenant_id,
        required_target_locales,
        tenant_locale_policy_revision: model.tenant_locale_policy_revision,
        revision: model.revision,
        freshness,
        disabled_required_target_locales,
    })
}

fn disabled_required_target_locales(
    required: &[TenantLocale],
    projection: &TenantLocalePolicyProjection,
) -> Vec<TenantLocale> {
    let enabled = projection
        .locales
        .iter()
        .filter(|entry| entry.is_enabled)
        .map(|entry| entry.locale.as_str())
        .collect::<BTreeSet<_>>();
    required
        .iter()
        .filter(|locale| !enabled.contains(locale.as_str()))
        .cloned()
        .collect()
}

async fn find_policy_receipt<C>(
    database: &C,
    tenant_id: Uuid,
    idempotency_key: &str,
) -> TranslationResult<Option<policy_receipt::Model>>
where
    C: sea_orm::ConnectionTrait,
{
    Ok(policy_receipt::Entity::find()
        .filter(policy_receipt::Column::TenantId.eq(tenant_id))
        .filter(policy_receipt::Column::IdempotencyKey.eq(idempotency_key))
        .one(database)
        .await?)
}

fn replay_policy_receipt(
    receipt: policy_receipt::Model,
    context: &PortContext,
    request_hash: &str,
) -> TranslationResult<TranslationPolicyRecord> {
    if receipt.requested_by_actor_kind != actor_kind(context)
        || receipt.requested_by_actor_id != context.actor.id
    {
        return Err(TranslationError::IdempotencyActorMismatch);
    }
    if receipt.request_hash != request_hash {
        return Err(TranslationError::IdempotencyConflict);
    }
    let record: TranslationPolicyRecord = serde_json::from_value(receipt.response)?;
    if record.revision != receipt.resulting_policy_revision {
        return Err(TranslationError::TranslationPolicyInvariant(
            "translation policy receipt revision does not match its response".to_string(),
        ));
    }
    Ok(record)
}

fn authorize(
    context: &PortContext,
    action: Action,
    call_policy: PortCallPolicy,
) -> TranslationResult<Uuid> {
    context.require_policy(call_policy)?;
    let security = SecurityContext::try_from_port_context(context)?;
    if security.get_scope(Resource::Translations, action) == PermissionScope::None {
        return Err(TranslationError::Forbidden);
    }
    Uuid::parse_str(&context.tenant_id).map_err(|_| TranslationError::InvalidTenantId)
}

fn actor_kind(context: &PortContext) -> &'static str {
    match context.actor.kind {
        PortActorKind::User => "user",
        PortActorKind::Service => "service",
        PortActorKind::System => "system",
    }
}
