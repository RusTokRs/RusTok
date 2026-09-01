use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, DatabaseBackend,
    DatabaseConnection, DatabaseTransaction, EntityTrait, QueryFilter, QueryOrder, QuerySelect,
    Statement, TransactionTrait,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use rustok_api::{Action, Resource};
use rustok_core::SecurityContext;

use crate::entities::{
    forum_user_trust_revision::{self, ForumUserTrustChangeKind},
    forum_user_trust_state,
};
use crate::error::{ForumError, ForumResult};

use super::rbac::enforce_scope;

pub const MAX_FORUM_USER_TRUST_LEVEL: u8 = crate::audience::MAX_FORUM_AUDIENCE_TRUST_LEVEL;
pub const MAX_FORUM_USER_TRUST_HISTORY_PAGE: u16 = 100;
const MAX_REASON_CODE_LENGTH: usize = 64;
const MAX_REASON_SUMMARY_LENGTH: usize = 256;
const MAX_IDEMPOTENCY_KEY_LENGTH: usize = 128;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ForumUserTrustState {
    pub user_id: Uuid,
    pub configured: bool,
    pub trust_level: u8,
    pub revision: u64,
    pub updated_at: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ForumUserTrustRevision {
    pub user_id: Uuid,
    pub revision: u64,
    pub previous_trust_level: Option<u8>,
    pub trust_level: u8,
    pub change_kind: ForumUserTrustChangeKind,
    pub reason_code: String,
    pub reason_summary: String,
    pub changed_by_user_id: Option<Uuid>,
    pub idempotency_key: String,
    pub created_at: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ForumUserTrustChange {
    pub state: ForumUserTrustState,
    pub revision: ForumUserTrustRevision,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ForumUserTrustRevisionPage {
    pub items: Vec<ForumUserTrustRevision>,
    pub next_before_revision: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct SetForumUserTrustInput {
    pub trust_level: u8,
    pub reason_code: String,
    pub reason_summary: String,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct NormalizedSetForumUserTrustInput {
    trust_level: u8,
    reason_code: String,
    reason_summary: String,
    idempotency_key: String,
}

/// Forum-owned authoritative trust state.
///
/// This owner is deliberately independent from `forum_user_stats`. The activity
/// projection can become one typed input to a later policy evaluator, but it is
/// never interpreted as trust state and is not read by this service.
pub struct ForumUserTrustService {
    db: DatabaseConnection,
}

impl ForumUserTrustService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn get(
        &self,
        tenant_id: Uuid,
        user_id: Uuid,
        security: SecurityContext,
    ) -> ForumResult<ForumUserTrustState> {
        enforce_manage_scope(&security)?;
        validate_identity(tenant_id, user_id)?;

        let txn = self.db.begin().await?;
        lock_user_trust_in_tx(&txn, tenant_id, user_id).await?;
        let state = load_state(&txn, tenant_id, user_id).await?;
        txn.commit().await?;
        state_from_model(user_id, state)
    }

    pub async fn set(
        &self,
        tenant_id: Uuid,
        user_id: Uuid,
        security: SecurityContext,
        input: SetForumUserTrustInput,
    ) -> ForumResult<ForumUserTrustChange> {
        enforce_manage_scope(&security)?;
        validate_identity(tenant_id, user_id)?;
        let changed_by_user_id = security.user_id.ok_or_else(|| {
            ForumError::forbidden("Forum trust changes require an authenticated user actor")
        })?;
        if changed_by_user_id.is_nil() {
            return Err(ForumError::Validation(
                "Forum trust change actor cannot be nil".to_string(),
            ));
        }
        let input = normalize_input(input)?;

        let txn = self.db.begin().await?;
        lock_user_trust_in_tx(&txn, tenant_id, user_id).await?;

        if let Some(replayed) = forum_user_trust_revision::Entity::find()
            .filter(forum_user_trust_revision::Column::TenantId.eq(tenant_id))
            .filter(
                forum_user_trust_revision::Column::IdempotencyKey.eq(input.idempotency_key.clone()),
            )
            .one(&txn)
            .await?
        {
            ensure_replay_matches(&replayed, user_id, changed_by_user_id, &input)?;
            let change = change_from_revision_model(replayed)?;
            txn.commit().await?;
            return Ok(change);
        }

        let current = load_state(&txn, tenant_id, user_id).await?;
        let previous_trust_level = current
            .as_ref()
            .map(|state| trust_level_from_storage(state.trust_level))
            .transpose()?;
        let revision = current
            .as_ref()
            .map(|state| {
                state.revision.checked_add(1).ok_or_else(|| {
                    ForumError::Validation(
                        "Forum user trust revision has reached its storage limit".to_string(),
                    )
                })
            })
            .transpose()?
            .unwrap_or(1);
        let now = Utc::now();

        let inserted = forum_user_trust_revision::ActiveModel {
            tenant_id: Set(tenant_id),
            user_id: Set(user_id),
            revision: Set(revision),
            previous_trust_level: Set(previous_trust_level.map(i16::from)),
            trust_level: Set(i16::from(input.trust_level)),
            change_kind: Set(ForumUserTrustChangeKind::ManualOverride),
            reason_code: Set(input.reason_code.clone()),
            reason_summary: Set(input.reason_summary.clone()),
            changed_by_user_id: Set(Some(changed_by_user_id)),
            idempotency_key: Set(input.idempotency_key.clone()),
            created_at: Set(now.into()),
        }
        .insert(&txn)
        .await?;

        let materialized = load_state(&txn, tenant_id, user_id).await?.ok_or_else(|| {
            ForumError::Validation(
                "Forum trust revision did not materialize a current state".to_string(),
            )
        })?;
        if materialized.revision != revision
            || materialized.trust_level != i16::from(input.trust_level)
        {
            return Err(ForumError::Validation(
                "Forum trust materialized state does not match its immutable revision".to_string(),
            ));
        }

        let change = change_from_revision_model(inserted)?;
        txn.commit().await?;
        Ok(change)
    }

    pub async fn history(
        &self,
        tenant_id: Uuid,
        user_id: Uuid,
        security: SecurityContext,
        before_revision: Option<u64>,
        limit: u16,
    ) -> ForumResult<ForumUserTrustRevisionPage> {
        enforce_manage_scope(&security)?;
        validate_identity(tenant_id, user_id)?;
        if limit == 0 || limit > MAX_FORUM_USER_TRUST_HISTORY_PAGE {
            return Err(ForumError::Validation(format!(
                "Forum trust history limit must be between 1 and {MAX_FORUM_USER_TRUST_HISTORY_PAGE}"
            )));
        }
        let before_revision = before_revision
            .map(|revision| {
                if revision == 0 {
                    return Err(ForumError::Validation(
                        "Forum trust history cursor must be greater than zero".to_string(),
                    ));
                }
                i64::try_from(revision).map_err(|_| {
                    ForumError::Validation(
                        "Forum trust history cursor exceeds the supported range".to_string(),
                    )
                })
            })
            .transpose()?;

        let txn = self.db.begin().await?;
        lock_user_trust_in_tx(&txn, tenant_id, user_id).await?;
        let mut query = forum_user_trust_revision::Entity::find()
            .filter(forum_user_trust_revision::Column::TenantId.eq(tenant_id))
            .filter(forum_user_trust_revision::Column::UserId.eq(user_id))
            .order_by_desc(forum_user_trust_revision::Column::Revision)
            .limit(u64::from(limit) + 1);
        if let Some(before_revision) = before_revision {
            query = query.filter(forum_user_trust_revision::Column::Revision.lt(before_revision));
        }
        let mut rows = query.all(&txn).await?;
        let has_more = rows.len() > usize::from(limit);
        if has_more {
            rows.truncate(usize::from(limit));
        }
        let items = rows
            .into_iter()
            .map(revision_from_model)
            .collect::<ForumResult<Vec<_>>>()?;
        let next_before_revision = has_more
            .then(|| items.last().map(|item| item.revision))
            .flatten();
        txn.commit().await?;

        Ok(ForumUserTrustRevisionPage {
            items,
            next_before_revision,
        })
    }
}

fn enforce_manage_scope(security: &SecurityContext) -> ForumResult<()> {
    enforce_scope(security, Resource::ForumTopics, Action::Manage)
}

fn validate_identity(tenant_id: Uuid, user_id: Uuid) -> ForumResult<()> {
    if tenant_id.is_nil() || user_id.is_nil() {
        return Err(ForumError::Validation(
            "Forum trust tenant and user identities must be non-nil".to_string(),
        ));
    }
    Ok(())
}

fn normalize_input(input: SetForumUserTrustInput) -> ForumResult<NormalizedSetForumUserTrustInput> {
    if input.trust_level > MAX_FORUM_USER_TRUST_LEVEL {
        return Err(ForumError::Validation(format!(
            "Forum trust level must be between 0 and {MAX_FORUM_USER_TRUST_LEVEL}"
        )));
    }

    let reason_code = input.reason_code.trim().to_ascii_lowercase();
    if reason_code.is_empty()
        || reason_code.len() > MAX_REASON_CODE_LENGTH
        || !reason_code.chars().enumerate().all(|(index, character)| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || (index > 0 && matches!(character, '_' | '.' | '-'))
        })
    {
        return Err(ForumError::Validation(
            "Forum trust reason code must be a bounded lowercase token".to_string(),
        ));
    }

    let reason_summary = input.reason_summary.trim().to_string();
    if reason_summary.is_empty()
        || reason_summary.len() > MAX_REASON_SUMMARY_LENGTH
        || reason_summary.chars().any(char::is_control)
    {
        return Err(ForumError::Validation(
            "Forum trust reason summary must be bounded single-line text".to_string(),
        ));
    }

    let idempotency_key = input.idempotency_key.trim().to_string();
    if idempotency_key.is_empty()
        || idempotency_key.len() > MAX_IDEMPOTENCY_KEY_LENGTH
        || idempotency_key.chars().any(char::is_control)
    {
        return Err(ForumError::Validation(
            "Forum trust idempotency key must be bounded single-line text".to_string(),
        ));
    }

    Ok(NormalizedSetForumUserTrustInput {
        trust_level: input.trust_level,
        reason_code,
        reason_summary,
        idempotency_key,
    })
}

async fn load_state<C>(
    db: &C,
    tenant_id: Uuid,
    user_id: Uuid,
) -> ForumResult<Option<forum_user_trust_state::Model>>
where
    C: ConnectionTrait,
{
    Ok(
        forum_user_trust_state::Entity::find_by_id((tenant_id, user_id))
            .one(db)
            .await?,
    )
}

fn state_from_model(
    user_id: Uuid,
    state: Option<forum_user_trust_state::Model>,
) -> ForumResult<ForumUserTrustState> {
    match state {
        Some(state) => Ok(ForumUserTrustState {
            user_id,
            configured: true,
            trust_level: trust_level_from_storage(state.trust_level)?,
            revision: revision_from_storage(state.revision)?,
            updated_at: Some(state.updated_at.to_rfc3339()),
        }),
        None => Ok(ForumUserTrustState {
            user_id,
            configured: false,
            trust_level: 0,
            revision: 0,
            updated_at: None,
        }),
    }
}

fn revision_from_model(
    model: forum_user_trust_revision::Model,
) -> ForumResult<ForumUserTrustRevision> {
    Ok(ForumUserTrustRevision {
        user_id: model.user_id,
        revision: revision_from_storage(model.revision)?,
        previous_trust_level: model
            .previous_trust_level
            .map(trust_level_from_storage)
            .transpose()?,
        trust_level: trust_level_from_storage(model.trust_level)?,
        change_kind: model.change_kind,
        reason_code: model.reason_code,
        reason_summary: model.reason_summary,
        changed_by_user_id: model.changed_by_user_id,
        idempotency_key: model.idempotency_key,
        created_at: model.created_at.to_rfc3339(),
    })
}

fn change_from_revision_model(
    model: forum_user_trust_revision::Model,
) -> ForumResult<ForumUserTrustChange> {
    let revision = revision_from_model(model)?;
    Ok(ForumUserTrustChange {
        state: ForumUserTrustState {
            user_id: revision.user_id,
            configured: true,
            trust_level: revision.trust_level,
            revision: revision.revision,
            updated_at: Some(revision.created_at.clone()),
        },
        revision,
    })
}

fn ensure_replay_matches(
    model: &forum_user_trust_revision::Model,
    user_id: Uuid,
    changed_by_user_id: Uuid,
    input: &NormalizedSetForumUserTrustInput,
) -> ForumResult<()> {
    let matches = model.user_id == user_id
        && model.trust_level == i16::from(input.trust_level)
        && model.change_kind == ForumUserTrustChangeKind::ManualOverride
        && model.reason_code == input.reason_code
        && model.reason_summary == input.reason_summary
        && model.changed_by_user_id == Some(changed_by_user_id);
    if !matches {
        return Err(ForumError::Validation(
            "Forum trust idempotency key was already used for another change".to_string(),
        ));
    }
    Ok(())
}

fn trust_level_from_storage(level: i16) -> ForumResult<u8> {
    let level = u8::try_from(level).map_err(|_| {
        ForumError::Validation("Forum trust storage contains an invalid level".to_string())
    })?;
    if level > MAX_FORUM_USER_TRUST_LEVEL {
        return Err(ForumError::Validation(
            "Forum trust storage contains an invalid level".to_string(),
        ));
    }
    Ok(level)
}

fn revision_from_storage(revision: i64) -> ForumResult<u64> {
    u64::try_from(revision).map_err(|_| {
        ForumError::Validation("Forum trust storage contains an invalid revision".to_string())
    })
}

pub(crate) async fn lock_user_trust_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    user_id: Uuid,
) -> ForumResult<()> {
    match txn.get_database_backend() {
        DatabaseBackend::Postgres => {
            txn.execute_raw(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                "SELECT pg_advisory_xact_lock(hashtextextended($1, 26))",
                [format!("{tenant_id}:{user_id}:trust").into()],
            ))
            .await?;
        }
        DatabaseBackend::Sqlite => {
            txn.execute_raw(Statement::from_string(
                DatabaseBackend::Sqlite,
                "SELECT 1".to_string(),
            ))
            .await?;
        }
        backend => {
            return Err(ForumError::Validation(format!(
                "Forum trust state does not support database backend {backend:?}"
            )));
        }
    }
    Ok(())
}
