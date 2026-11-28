use chrono::{DateTime, Duration, Utc};
use rustok_events::DomainEvent;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DbBackend, EntityTrait, QueryFilter,
    QuerySelect, Set, Statement, TransactionTrait, sea_query::Expr,
};
use sha2::{Digest, Sha256};

use crate::auth::{AuthConfig, decode_invite_token, normalize_email};
use crate::error::Error;
use crate::models::{auth_invite_consumptions, auth_invites, tenants, users};
use crate::services::auth_lifecycle::{AuthLifecycleError, AuthLifecycleService};
use crate::services::event_bus::transactional_event_bus_from_context;
use crate::services::rbac_service::RbacService;
use crate::services::server_runtime_context::ServerRuntimeContext;

/// Shared active-tenant fence for authenticated administration and invite
/// acceptance. Acquire before app/user/ledger locks; KEY SHARE would not
/// prevent an is_active update. SQLite must acquire its writer first.
pub(crate) async fn lock_active_auth_tenant(
    tx: &sea_orm::DatabaseTransaction,
    tenant_id: uuid::Uuid,
) -> Result<bool, sea_orm::DbErr> {
    match tx.get_database_backend() {
        DbBackend::Postgres => Ok(tx.query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT id FROM tenants WHERE id = $1 AND is_active = TRUE FOR SHARE",
            [tenant_id.into()],
        )).await?.is_some()),
        DbBackend::Sqlite => Ok(tenants::Entity::update_many()
            .col_expr(tenants::Column::UpdatedAt, Expr::col(tenants::Column::UpdatedAt))
            .filter(tenants::Column::Id.eq(tenant_id))
            .filter(tenants::Column::IsActive.eq(true))
            .exec(tx).await?.rows_affected == 1),
        _ => Err(sea_orm::DbErr::Custom("unsupported auth tenant fence backend".into())),
    }
}

/// Identifies an issuance-ledger row whose lifecycle is being changed.
#[derive(Clone, Copy)]
pub(crate) enum InviteLookup<'a> {
    Id(uuid::Uuid),
    TokenHash(&'a str),
}

/// Acquire the invite row's mutation fence and return the current persisted
/// value. Acceptance and revocation MUST use the same fence so a revocation
/// that commits first cannot race an acceptance based on an earlier read.
///
/// PostgreSQL and MySQL use a row-level locking read. SQLite has no
/// `SELECT .. FOR UPDATE`; a no-op update acquires its write lock before
/// reading the row, avoiding a deferred read-to-write upgrade race.
pub(crate) async fn lock_invite_for_mutation<C>(
    db: &C,
    tenant_id: uuid::Uuid,
    lookup: InviteLookup<'_>,
) -> Result<Option<auth_invites::Model>, sea_orm::DbErr>
where
    C: ConnectionTrait,
{
    let select = || {
        let query =
            auth_invites::Entity::find().filter(auth_invites::Column::TenantId.eq(tenant_id));
        match lookup {
            InviteLookup::Id(id) => query.filter(auth_invites::Column::Id.eq(id)),
            InviteLookup::TokenHash(token_hash) => {
                query.filter(auth_invites::Column::TokenHash.eq(token_hash))
            }
        }
    };

    match db.get_database_backend() {
        DbBackend::Postgres | DbBackend::MySql => select().lock_exclusive().one(db).await,
        DbBackend::Sqlite => {
            let update = auth_invites::Entity::update_many()
                .col_expr(
                    auth_invites::Column::TokenHash,
                    Expr::col(auth_invites::Column::TokenHash),
                )
                .filter(auth_invites::Column::TenantId.eq(tenant_id));
            let update = match lookup {
                InviteLookup::Id(id) => update.filter(auth_invites::Column::Id.eq(id)),
                InviteLookup::TokenHash(token_hash) => {
                    update.filter(auth_invites::Column::TokenHash.eq(token_hash))
                }
            };
            let result = update.exec(db).await?;
            if result.rows_affected != 1 {
                return Ok(None);
            }
            select().one(db).await
        }
        _ => Err(sea_orm::DbErr::Custom(
            "unsupported database backend for invite mutation locking".to_string(),
        )),
    }
}

/// The unlocked observation is only a locator. Lock the issuer before the
/// invite: deleting an issuer locks its user row before the FK SET NULL
/// update of its invitations. Never follow a changed issuer while holding
/// the invite fence. SQLite also acquires its writer before any tx read.
async fn lock_invite_for_acceptance(
    tx: &sea_orm::DatabaseTransaction,
    tenant_id: uuid::Uuid,
    observed: &auth_invites::Model,
) -> Result<Option<auth_invites::Model>, sea_orm::DbErr> {
    if !lock_active_auth_tenant(tx, tenant_id).await? {
        return Ok(None);
    }
    let Some(issuer_id) = observed.issued_by else {
        return Ok(None);
    };
    let Some(issuer) =
        AuthLifecycleService::lock_user_for_mutation(tx, tenant_id, issuer_id).await?
    else {
        return Ok(None);
    };
    let current = lock_invite_for_mutation(tx, tenant_id, InviteLookup::Id(observed.id)).await?;
    Ok(current.filter(|invite| {
        invite.token_hash == observed.token_hash
            && invite.revoked_at.is_none()
            && invite_issuer_matches_account(invite, &issuer)
    }))
}

#[derive(Debug)]
pub enum InviteAcceptanceError {
    InvalidToken,
    EmailAlreadyExists,
    Validation(String),
    Internal(Error),
}

impl From<InviteAcceptanceError> for Error {
    fn from(value: InviteAcceptanceError) -> Self {
        match value {
            InviteAcceptanceError::InvalidToken => {
                Error::Unauthorized("Invalid or already consumed invite token".to_string())
            }
            InviteAcceptanceError::EmailAlreadyExists => {
                Error::BadRequest("A user with this email already exists".to_string())
            }
            InviteAcceptanceError::Validation(message) => Error::Validation(message),
            InviteAcceptanceError::Internal(error) => error,
        }
    }
}

#[derive(Debug)]
pub struct AcceptedInvite {
    pub user: users::Model,
    pub email: String,
    pub role: rustok_core::UserRole,
}

pub(crate) fn invite_token_hash(token: &str) -> String {
    hex::encode(Sha256::digest(token.trim().as_bytes()))
}

fn invite_ledger_matches_claims(
    issued: &auth_invites::Model,
    email: &str,
    token_expires_at: DateTime<Utc>,
    now: DateTime<Utc>,
) -> bool {
    let Ok(issued_email) = normalize_email(&issued.email) else {
        return false;
    };
    let ledger_expires_at = issued.expires_at.with_timezone(&Utc);

    issued_email == email
        && issued.created_at.with_timezone(&Utc) <= now
        && ledger_expires_at > now
        && ledger_expires_at <= token_expires_at
}

/// Nonwaiting late receipt admission; token_hash is globally unique.
pub(crate) async fn fence_invite_receipt(
    tx: &sea_orm::DatabaseTransaction,
    token_hash: &str,
) -> Result<(), sea_orm::DbErr> {
    match tx.get_database_backend() {
        DbBackend::Postgres => {
            tx.execute_unprepared("LOCK TABLE auth_invite_consumptions IN ROW EXCLUSIVE MODE NOWAIT").await?;
            tx.execute_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                "SELECT auth_fence_invite_consumption_digest($1)",
                [token_hash.to_owned().into()],
            )).await?;
            Ok(())
        }
        DbBackend::Sqlite => Ok(()), // caller already reserved the writer
        _ => Err(sea_orm::DbErr::Custom("unsupported invite receipt backend".into())),
    }
}

async fn invite_already_consumed(
    db: &impl ConnectionTrait,
    tenant_id: uuid::Uuid,
    token_hash: &str,
) -> Result<bool, sea_orm::DbErr> {
    Ok(auth_invite_consumptions::Entity::find()
        .filter(auth_invite_consumptions::Column::TenantId.eq(tenant_id))
        .filter(auth_invite_consumptions::Column::TokenHash.eq(token_hash))
        .one(db)
        .await?
        .is_some())
}

impl AuthLifecycleService {
    /// Validate and consume an invitation exactly once while creating the user
    /// in the same database transaction.
    ///
    /// The SHA-256 token digest remains persisted after the created account is
    /// deleted, so an old stateless invite cannot be replayed to recreate the
    /// account before the JWT expires.
    pub async fn accept_invite_once_runtime(
        ctx: &ServerRuntimeContext,
        config: &AuthConfig,
        tenant_id: uuid::Uuid,
        token: &str,
        password: &str,
        name: Option<String>,
    ) -> Result<AcceptedInvite, InviteAcceptanceError> {
        // Canonical feature gate for every transport (REST handler and
        // GraphQL/native provider). Keep the rejection deliberately
        // indistinguishable from an invalid token so disabled invites do not
        // become an oracle for token validity.
        if !ctx.settings().features.invites_enabled {
            return Err(InviteAcceptanceError::InvalidToken);
        }

        // The JWT decoder accepts surrounding whitespace. Hash the same
        // canonical token representation so copy/pasted tokens remain
        // consistent with the issuance ledger lookup.
        let admitted_at = rustok_auth::application_clock::admit_now(ctx.db())
            .await
            .map_err(|error| InviteAcceptanceError::Internal(error.into()))?;
        let token = token.trim();
        let claims =
            decode_invite_token(config, token).map_err(|_| InviteAcceptanceError::InvalidToken)?;
        // INVARIANT: In-memory verification ensuring the decoded invite token belongs to the target tenant.
        if claims.tenant_id != tenant_id {
            return Err(InviteAcceptanceError::InvalidToken);
        }

        let expires_at = i64::try_from(claims.exp)
            .ok()
            .and_then(|seconds| DateTime::<Utc>::from_timestamp(seconds, 0))
            .ok_or(InviteAcceptanceError::InvalidToken)?;
        if expires_at <= admitted_at {
            return Err(InviteAcceptanceError::InvalidToken);
        }

        // A signed invite carrying a malformed address is an invalid token,
        // not caller input worth a distinct validation response. Validate
        // and hash the credentials *before* opening the transaction and
        // reserving the token digest, so a policy failure never burns a
        // single-use invite and Argon2 never runs inside the transaction.
        let email =
            normalize_email(&claims.sub).map_err(|_| InviteAcceptanceError::InvalidToken)?;
        let role = claims.role;
        let custom_role_slug = claims.custom_role_slug.clone();
        let ledger_role = custom_role_slug.clone().unwrap_or_else(|| role.to_string());
        let token_hash = invite_token_hash(token);
        // Read outside the transaction only to locate the issuer and exact
        // ledger row. Reject clearly invalid invitations before Argon2;
        // neither this snapshot nor its authority is trusted in the tx.
        let observed = auth_invites::Entity::find()
            .filter(auth_invites::Column::TenantId.eq(tenant_id))
            .filter(auth_invites::Column::TokenHash.eq(&token_hash))
            .one(ctx.db())
            .await
            .map_err(|error| InviteAcceptanceError::Internal(error.into()))?
            .filter(|invite| {
                invite.issued_by.is_some()
                    && invite.revoked_at.is_none()
                    && invite.role == ledger_role
                    && invite_ledger_matches_claims(invite, &email, expires_at, admitted_at)
            })
            .ok_or(InviteAcceptanceError::InvalidToken)?;

        // A committed receipt is terminal even when the account was deleted.
        // Reject ordinary replays before expensive password preparation. The
        // transaction still repeats the lookup under the invitation fence.
        if invite_already_consumed(ctx.db(), tenant_id, &token_hash)
            .await
            .map_err(|error| InviteAcceptanceError::Internal(error.into()))?
        {
            return Err(InviteAcceptanceError::InvalidToken);
        }

        let prepared = AuthLifecycleService::prepare_new_user(
            &email,
            password,
            name,
            config.min_password_length,
        )
        .await
        .map_err(map_lifecycle_error)?;
        let consumption_id = uuid::Uuid::new_v4();

        let registry = ctx.shared_get::<rustok_core::ModuleRegistry>();
        let mut attempt = 1;
        loop {
            // Re-admit the same signed token and pinned ledger identity;
            // never extend expiry or reuse transaction-local authority.
            let admitted = rustok_auth::application_clock::admit_now(ctx.db())
                .await
                .map_err(|error| InviteAcceptanceError::Internal(error.into()))?;
            if expires_at <= admitted || decode_invite_token(config, token).is_err() {
                return Err(InviteAcceptanceError::InvalidToken);
            }
            crate::services::rbac_module_availability::prepare_invite_module_policy(
                ctx.db(),
                registry.as_ref(),
            )
            .await
            .map_err(InviteAcceptanceError::Internal)?;

            let tx = ctx
                .db()
                .begin()
                .await
                .map_err(|error| InviteAcceptanceError::Internal(error.into()))?;

            // Tenant -> issuer -> invite -> module policy -> ordered role grants.
            // Revocation still uses the same invite-row fence.
            let issued = lock_invite_for_acceptance(&tx, tenant_id, &observed)
                .await
                .map_err(|error| InviteAcceptanceError::Internal(error.into()))?
                .ok_or(InviteAcceptanceError::InvalidToken)?;
            // Argon2 preparation and lock acquisition may take time. Recheck
            // expiration after the invite row is locked; it is checked once more
            // immediately before the unique consumption reservation below.
            let now = rustok_auth::application_clock::verified_now(&tx)
                .await
                .map_err(|error| InviteAcceptanceError::Internal(error.into()))?;
            if expires_at <= now
                || issued.role != ledger_role
                || !invite_ledger_matches_claims(&issued, &email, expires_at.clone(), now)
            {
                return Err(InviteAcceptanceError::InvalidToken);
            }
            let invite_id = issued.id;

            // Under the ledger fence a committed receipt is terminal. Avoid
            // taking module/RBAC locks for a replay. Keep the unique insert as
            // the final authority even if another writer bypasses this fence.
            if tx.get_database_backend() == DbBackend::Postgres {
                // Only a relation probe here; the digest gate follows RBAC.
                if let Err(error) = tx.execute_unprepared(
                    "LOCK TABLE auth_invite_consumptions IN ACCESS SHARE MODE NOWAIT",
                ).await {
                    if rustok_core::resilience::retry::rollback_for_transaction_retry(
                        tx, &error, attempt,
                    ).await.map_err(|error| InviteAcceptanceError::Internal(error.into()))? {
                        attempt += 1;
                        continue;
                    }
                    return Err(InviteAcceptanceError::Internal(error.into()));
                }
            }
            if invite_already_consumed(&tx, tenant_id, &token_hash)
                .await
                .map_err(|error| InviteAcceptanceError::Internal(error.into()))?
            {
                return Err(InviteAcceptanceError::InvalidToken);
            }

            // Role grants, module availability, and the issuer's authority can
            // change while an invite is outstanding. Re-check the issuer's active
            // invite permission, hierarchy, and current grant ceiling before
            // accepting it, so a later role edit cannot widen the invitation.
            // This happens before reservation, leaving policy-rejected invites
            // unconsumed.
            let issuer_id = issued
                .issued_by
                .ok_or(InviteAcceptanceError::InvalidToken)?;
            // The issuer mutation fence and account epoch were validated by
            // lock_invite_for_acceptance and remain held through commit.
            let (module_policy_binding, availability) =
                crate::services::rbac_module_availability::lock_invite_module_policy(
                    ctx.db(),
                    &tx,
                    registry.as_ref(),
                    tenant_id,
                )
                .await
                .map_err(InviteAcceptanceError::Internal)?;
            if issued.module_policy_binding.as_deref() != Some(module_policy_binding.as_str()) {
                return Err(InviteAcceptanceError::InvalidToken);
            }
            let membership_revision =
                rustok_rbac::read_user_membership_revision_on(&tx, tenant_id, issuer_id)
                    .await
                    .map_err(|error| InviteAcceptanceError::Internal(error.into()))?;
            if issued.issuer_membership_revision != Some(membership_revision) {
                return Err(InviteAcceptanceError::InvalidToken);
            }
            let role_binding =
                rustok_rbac::lock_invite_role_grant_binding_on(&tx, tenant_id, issuer_id, &ledger_role)
                    .await
                    .map_err(|error| InviteAcceptanceError::Internal(error.into()))?;
            if role_binding.is_none() || issued.role_grant_binding != role_binding {
                return Err(InviteAcceptanceError::InvalidToken);
            }
            let mut issuer_permissions =
                RbacService::get_user_permissions_authoritative(&tx, &tenant_id, &issuer_id)
                    .await
                    .map_err(InviteAcceptanceError::Internal)?;
            // Permission grants owned by disabled modules are persisted but are
            // not current request authority. Apply the same effective-permission
            // boundary used at invite issuance and regular authentication before
            // checking whether the issuer may delegate this role.
            if let Some(availability) = availability {
                issuer_permissions = availability.active_permissions(&issuer_permissions);
            }
            let issuer_role = RbacService::get_user_role_authoritative(&tx, &tenant_id, &issuer_id)
                .await
                .map_err(InviteAcceptanceError::Internal)?;
            let may_issue_invite =
                issuer_may_issue_invite(&issuer_permissions, &role, custom_role_slug.is_some());
            if !may_issue_invite {
                tracing::warn!(
                    tenant_id = %tenant_id,
                    issuer_id = %issuer_id,
                    role_slug = %ledger_role,
                    "invite issuer no longer has authority to issue this invite"
                );
                return Err(InviteAcceptanceError::InvalidToken);
            }
            let authority_check = validate_invite_role_grant_on(
                &tx,
                tenant_id,
                &issuer_role,
                &issuer_permissions,
                &role,
                custom_role_slug.as_deref(),
            )
            .await;
            if let Err(error) = authority_check {
                tracing::warn!(
                    %error,
                    tenant_id = %tenant_id,
                    issuer_id = %issuer_id,
                    role_slug = %ledger_role,
                    "invite role no longer satisfies issuer authority"
                );
                return Err(map_invite_role_authority_error(error));
            }

            // Recheck at the actual consumption boundary after issuer/RBAC reads.
            // The signed JWT and issuance ledger must both still be live when the
            // single-use reservation is written.
            let consumed_at = rustok_auth::application_clock::verified_now(&tx)
                .await
                .map_err(|error| InviteAcceptanceError::Internal(error.into()))?;
            if expires_at <= consumed_at
                || decode_invite_token(config, token).is_err()
                || !invite_ledger_matches_claims(&issued, &email, expires_at.clone(), consumed_at)
            {
                return Err(InviteAcceptanceError::InvalidToken);
            }

            // Reserve the digest first. The unique token_hash constraint is the
            // serialization point, so every concurrent replay is rejected before
            // user creation regardless of email uniqueness or transaction timing.
            let reservation = auth_invite_consumptions::ActiveModel {
                id: Set(consumption_id),
                tenant_id: Set(tenant_id),
                token_hash: Set(token_hash.clone()),
                email: Set(email.clone()),
                role: Set(ledger_role.clone()),
                user_id: Set(None),
                expires_at: Set(expires_at.into()),
                consumed_at: Set(consumed_at.into()),
            };
            let reservation_result = match fence_invite_receipt(&tx, &token_hash).await {
                Ok(()) => reservation.insert(&tx).await.map(|_| ()),
                Err(error) => Err(error),
            };
            if let Err(insert_error) = reservation_result {
                if rustok_core::resilience::retry::rollback_for_transaction_retry(
                    tx, &insert_error, attempt,
                )
                .await
                .map_err(|error| InviteAcceptanceError::Internal(error.into()))?
                {
                    attempt += 1;
                    continue;
                }

                let already_consumed = invite_already_consumed(ctx.db(), tenant_id, &token_hash)
                    .await
                    .map_err(|error| InviteAcceptanceError::Internal(error.into()))?;
                return if already_consumed {
                    Err(InviteAcceptanceError::InvalidToken)
                } else {
                    Err(InviteAcceptanceError::Internal(insert_error.into()))
                };
            }

            // The target grant is already authorized and locked. Do not
            // temporarily assign Customer: that unrelated role may require an
            // additional lock or bootstrap write and creates spurious membership
            // history. Both branches share validated account insertion.
            let created = if let Some(slug) = custom_role_slug.as_deref() {
                AuthLifecycleService::create_user_with_custom_role_in_tx(
                    &tx,
                    tenant_id,
                    prepared.clone(),
                    slug,
                    Some(rustok_core::UserStatus::Active),
                )
                .await
            } else {
                AuthLifecycleService::create_user_in_tx(
                    &tx,
                    tenant_id,
                    prepared.clone(),
                    role.clone(),
                    Some(rustok_core::UserStatus::Active),
                )
                .await
            };
            let user = match created {
                Ok(user) => user,
                Err(AuthLifecycleError::Internal(Error::Database(error))) => {
                    if rustok_core::resilience::retry::rollback_for_transaction_retry(
                        tx, &error, attempt,
                    )
                    .await
                    .map_err(|error| InviteAcceptanceError::Internal(error.into()))?
                    {
                        attempt += 1;
                        continue;
                    }
                    return Err(InviteAcceptanceError::Internal(error.into()));
                }
                Err(error) => {
                    tx.rollback()
                        .await
                        .map_err(|rollback| InviteAcceptanceError::Internal(rollback.into()))?;
                    return Err(map_lifecycle_error(error));
                }
            };

            // Project the final custom-role membership, never the JWT's
            // built-in placeholder, before committing the acceptance.
            let role = if custom_role_slug.is_some() {
                RbacService::get_user_role_authoritative(&tx, &tenant_id, &user.id)
                    .await
                    .map_err(InviteAcceptanceError::Internal)?
            } else {
                role.clone()
            };

            let reservation = auth_invite_consumptions::Entity::find_by_id(consumption_id)
                .filter(auth_invite_consumptions::Column::TenantId.eq(tenant_id))
                .one(&tx)
                .await
                .map_err(|error| InviteAcceptanceError::Internal(error.into()))?
                .ok_or_else(|| {
                    InviteAcceptanceError::Internal(Error::Message(
                        "invite consumption reservation disappeared".to_string(),
                    ))
                })?;
            let mut active: auth_invite_consumptions::ActiveModel = reservation.into();
            active.user_id = Set(Some(user.id));
            active
                .update(&tx)
                .await
                .map_err(|error| InviteAcceptanceError::Internal(error.into()))?;

            let event_bus = transactional_event_bus_from_context(ctx);
            event_bus
                .publish_in_tx(
                    &tx,
                    tenant_id,
                    Some(user.id),
                    DomainEvent::UserAccountRegistered { user_id: user.id },
                )
                .await
                .map_err(|error| InviteAcceptanceError::Internal(error.into()))?;

            event_bus
                .publish_in_tx(
                    &tx,
                    tenant_id,
                    Some(user.id),
                    DomainEvent::UserInviteAccepted {
                        user_id: user.id,
                        invite_id,
                    },
                )
                .await
                .map_err(|error| InviteAcceptanceError::Internal(error.into()))?;

            // User creation, role assignment and outbox inserts can wait on
            // other transactions. A late invite must not leave a user or receipt.
            let final_now = rustok_auth::application_clock::verified_now(&tx)
                .await
                .map_err(|error| InviteAcceptanceError::Internal(error.into()))?;
            if expires_at <= final_now
                || decode_invite_token(config, token).is_err()
                || !invite_ledger_matches_claims(&issued, &email, expires_at, final_now)
            {
                tx.rollback()
                    .await
                    .map_err(|error| InviteAcceptanceError::Internal(error.into()))?;
                return Err(InviteAcceptanceError::InvalidToken);
            }
            tx.commit()
                .await
                .map_err(|error| InviteAcceptanceError::Internal(error.into()))?;

            return Ok(AcceptedInvite { user, email, role });
        }
    }
}

/// Issuance binds the durable account epoch, not merely the current active
/// status. Restoring an earlier email/password/status cannot revive an invite.
fn invite_issuer_matches_account(issued: &auth_invites::Model, issuer: &users::Model) -> bool {
    issued.tenant_id == issuer.tenant_id
        && issued.issued_by == Some(issuer.id)
        && issuer.is_active()
        && issuer.recovery_revision >= 0
        && issued.issuer_recovery_revision == Some(issuer.recovery_revision)
}

pub(crate) fn issuer_may_issue_invite(
    permissions: &[rustok_api::Permission],
    role: &rustok_core::UserRole,
    is_custom_role: bool,
) -> bool {
    use rustok_api::Permission;

    if is_custom_role || role != &rustok_core::UserRole::Customer {
        rustok_rbac::has_effective_permission_in_set(permissions, &Permission::USERS_MANAGE)
    } else {
        rustok_rbac::has_effective_permission_in_set(permissions, &Permission::USERS_CREATE)
            || rustok_rbac::has_effective_permission_in_set(permissions, &Permission::USERS_MANAGE)
    }
}

/// Validate delegation against one authority view. Issuance uses both the
/// request-bound ceiling and freshly read persisted authority; acceptance
/// uses freshly read persisted authority. Existing target roles stay locked
/// until the caller's transaction ends.
pub(crate) async fn validate_invite_role_grant_on(
    tx: &sea_orm::DatabaseTransaction,
    tenant_id: uuid::Uuid,
    actor_role: &rustok_core::UserRole,
    actor_permissions: &[rustok_api::Permission],
    role: &rustok_core::UserRole,
    custom_role_slug: Option<&str>,
) -> Result<String, rustok_rbac::RbacRolePersistenceError> {
    if let Some(slug) = custom_role_slug {
        rustok_rbac::validate_custom_role_grant_on(
            tx,
            tenant_id,
            actor_role,
            actor_permissions,
            slug,
        )
        .await
    } else {
        if role != &rustok_core::UserRole::Customer {
            rustok_rbac::require_role_assignment(actor_role, role)?;
        }
        rustok_rbac::validate_persisted_role_grant_on(
            tx,
            tenant_id,
            actor_permissions,
            &role.to_string(),
        )
        .await
    }
}

fn map_invite_role_authority_error(
    error: rustok_rbac::RbacRolePersistenceError,
) -> InviteAcceptanceError {
    use rustok_rbac::RbacRolePersistenceError;

    match error {
        RbacRolePersistenceError::Policy(_)
        | RbacRolePersistenceError::RoleNotFound(_)
        | RbacRolePersistenceError::TargetNotFound => InviteAcceptanceError::InvalidToken,
        RbacRolePersistenceError::Database(error) => InviteAcceptanceError::Internal(error.into()),
        _ => InviteAcceptanceError::Internal(Error::InternalServerError),
    }
}

fn map_lifecycle_error(error: AuthLifecycleError) -> InviteAcceptanceError {
    match error {
        AuthLifecycleError::EmailAlreadyExists => InviteAcceptanceError::EmailAlreadyExists,
        AuthLifecycleError::Validation(message) => InviteAcceptanceError::Validation(message),
        other => InviteAcceptanceError::Internal(Error::from(other)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::RustokSettings;
    use sha2::{Digest, Sha256};

    #[test]
    fn invite_authority_storage_failures_are_not_invalid_tokens() {
        use rustok_rbac::{RbacRoleMutationPolicyError, RbacRolePersistenceError};

        assert!(matches!(
            map_invite_role_authority_error(RbacRolePersistenceError::Database(
                sea_orm::DbErr::Custom("storage unavailable".to_string())
            )),
            InviteAcceptanceError::Internal(_)
        ));
        assert!(matches!(
            map_invite_role_authority_error(RbacRolePersistenceError::RoleNotFound(
                "deleted-role".to_string()
            )),
            InviteAcceptanceError::InvalidToken
        ));
        assert!(matches!(
            map_invite_role_authority_error(
                RbacRoleMutationPolicyError::CannotAssignPeerOrHigherRole.into()
            ),
            InviteAcceptanceError::InvalidToken
        ));
    }

    #[test]
    fn pending_invites_require_the_issuer_to_retain_invitation_authority() {
        use rustok_api::Permission;
        use rustok_core::UserRole;

        assert!(issuer_may_issue_invite(
            &[Permission::USERS_CREATE],
            &UserRole::Customer,
            false,
        ));
        assert!(!issuer_may_issue_invite(&[], &UserRole::Customer, false,));
        assert!(!issuer_may_issue_invite(
            &[Permission::USERS_CREATE],
            &UserRole::Admin,
            false,
        ));
        assert!(issuer_may_issue_invite(
            &[Permission::USERS_MANAGE],
            &UserRole::Manager,
            true,
        ));
        assert!(!issuer_may_issue_invite(
            &[Permission::USERS_CREATE],
            &UserRole::Manager,
            true,
        ));

        let disabled_create = rustok_rbac::PermissionAvailability::new(
            std::collections::BTreeMap::from([("users:create".to_string(), "auth".to_string())]),
            std::collections::BTreeSet::new(),
        );
        let current_permissions = disabled_create.active_permissions(&[Permission::USERS_CREATE]);
        assert!(!issuer_may_issue_invite(
            &current_permissions,
            &UserRole::Customer,
            false,
        ));
    }

    #[tokio::test]
    async fn invite_mutation_lock_reads_the_current_row_for_both_lookup_keys() {
        use sea_orm::{Database, TransactionTrait};

        let db = Database::connect("sqlite::memory:")
            .await
            .expect("sqlite test database");
        db.execute_unprepared(
            "CREATE TABLE auth_invites (\
                id TEXT PRIMARY KEY NOT NULL, \
                tenant_id TEXT NOT NULL, \
                token_hash VARCHAR(64) NOT NULL UNIQUE, \
                email VARCHAR(320) NOT NULL, \
                role VARCHAR(64) NOT NULL, \
                issued_by TEXT NULL, \
                issuer_recovery_revision BIGINT NULL, \
                issuer_membership_revision BIGINT NULL, \
                role_grant_binding TEXT NULL, \
                module_policy_binding TEXT NULL, \
                expires_at TEXT NOT NULL, \
                revoked_at TEXT NULL, \
                created_at TEXT NOT NULL\
            )",
        )
        .await
        .expect("create invite ledger");

        let tenant_id = uuid::Uuid::new_v4();
        let invite_id = uuid::Uuid::new_v4();
        let now = Utc::now();
        auth_invites::ActiveModel {
            id: Set(invite_id),
            tenant_id: Set(tenant_id),
            token_hash: Set("test-invite-digest".to_string()),
            email: Set("invite@example.test".to_string()),
            role: Set("customer".to_string()),
            issued_by: Set(None),
            issuer_recovery_revision: Set(None),
            issuer_membership_revision: Set(None),
            role_grant_binding: Set(None),
            module_policy_binding: Set(None),
            expires_at: Set((now + chrono::Duration::days(7)).into()),
            revoked_at: Set(None),
            created_at: Set(now.into()),
        }
        .insert(&db)
        .await
        .expect("insert invite");

        let tx = db.begin().await.expect("begin first transaction");
        let issued = lock_invite_for_mutation(
            &tx,
            tenant_id,
            InviteLookup::TokenHash("test-invite-digest"),
        )
        .await
        .expect("lock invite by digest")
        .expect("invite exists");
        assert!(issued.revoked_at.is_none());

        let mut active: auth_invites::ActiveModel = issued.into();
        active.revoked_at = Set(Some(Utc::now().into()));
        active.update(&tx).await.expect("revoke invite");
        tx.commit().await.expect("commit revocation");

        let tx = db.begin().await.expect("begin second transaction");
        let current = lock_invite_for_mutation(&tx, tenant_id, InviteLookup::Id(invite_id))
            .await
            .expect("lock invite by id")
            .expect("invite still exists");
        assert!(current.revoked_at.is_some());
        tx.rollback().await.expect("release invite lock");
    }

    #[tokio::test]
    #[ignore = "requires RUSTOK_AUTH_TEST_DATABASE_URL and PostgreSQL schema creation privileges"]
    async fn postgres_invite_tenant_fence_serializes_deactivation() {
        use sea_orm::{Database, Statement};
        use std::time::Duration;

        let db = Database::connect(
            std::env::var("RUSTOK_AUTH_TEST_DATABASE_URL").unwrap(),
        ).await.unwrap();
        assert_eq!(db.get_database_backend(), DbBackend::Postgres);
        let schema = format!("auth_invite_tenant_{}", uuid::Uuid::new_v4().simple());
        let tenant_id = uuid::Uuid::new_v4();
        db.execute_unprepared(&format!(
            "CREATE SCHEMA {schema};
             CREATE TABLE {schema}.tenants (id UUID PRIMARY KEY, is_active BOOLEAN NOT NULL);
             INSERT INTO {schema}.tenants VALUES ('{tenant_id}', TRUE)"
        )).await.unwrap();

        let admission = db.begin().await.unwrap();
        admission.execute_unprepared(&format!("SET LOCAL search_path TO {schema}"))
            .await.unwrap();
        assert!(lock_active_auth_tenant(&admission, tenant_id).await.unwrap());

        let deactivation = db.begin().await.unwrap();
        deactivation.execute_unprepared("SET LOCAL lock_timeout = '100ms'").await.unwrap();
        let error = tokio::time::timeout(
            Duration::from_secs(3),
            deactivation.execute_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                format!("UPDATE {schema}.tenants SET is_active = FALSE WHERE id = $1"),
                [tenant_id.into()],
            )),
        ).await.expect("bounded lock timeout").unwrap_err();
        assert_eq!(
            rustok_core::resilience::retry::PostgresTransactionConflict::classify(
                DbBackend::Postgres, &error,
            ),
            Some(rustok_core::resilience::retry::PostgresTransactionConflict::LockUnavailable),
        );
        deactivation.rollback().await.unwrap();
        admission.rollback().await.unwrap();

        db.execute_unprepared(&format!("UPDATE {schema}.tenants SET is_active = FALSE"))
            .await.unwrap();
        let retry = db.begin().await.unwrap();
        retry.execute_unprepared(&format!("SET LOCAL search_path TO {schema}"))
            .await.unwrap();
        assert!(!lock_active_auth_tenant(&retry, tenant_id).await.unwrap());
        retry.rollback().await.unwrap();
        db.execute_unprepared(&format!("DROP SCHEMA {schema} CASCADE")).await.unwrap();
    }

    #[tokio::test]
    async fn acceptance_rereads_invite_after_locking_observed_issuer() {
        use crate::models::tenants;
        use rustok_migrations::SqliteTestMigrator;
        use rustok_test_utils::db::setup_test_db_with_migrations;

        let db = setup_test_db_with_migrations::<SqliteTestMigrator>().await;
        let tenant = tenants::ActiveModel::new("Tenant", "invite-lock-order")
            .insert(&db)
            .await
            .unwrap();
        let issuer = users::ActiveModel::new(tenant.id, "issuer@example.test", "hash")
            .insert(&db)
            .await
            .unwrap();
        let other = users::ActiveModel::new(tenant.id, "other@example.test", "hash")
            .insert(&db)
            .await
            .unwrap();
        let now = Utc::now();
        let observed = auth_invites::ActiveModel {
            id: Set(uuid::Uuid::new_v4()),
            tenant_id: Set(tenant.id),
            token_hash: Set("a".repeat(64)),
            email: Set("invitee@example.test".to_string()),
            role: Set("customer".to_string()),
            issued_by: Set(Some(issuer.id)),
            issuer_recovery_revision: Set(Some(issuer.recovery_revision)),
            issuer_membership_revision: Set(None),
            role_grant_binding: Set(None),
            module_policy_binding: Set(None),
            expires_at: Set((now + Duration::hours(1)).into()),
            revoked_at: Set(None),
            created_at: Set(now.into()),
        }
        .insert(&db)
        .await
        .unwrap();

        let tx = db.begin().await.unwrap();
        assert!(
            lock_invite_for_acceptance(&tx, tenant.id, &observed)
                .await
                .unwrap()
                .is_some()
        );
        tx.rollback().await.unwrap();

        // A disabled tenant must reject even a previously valid locator.
        tenants::ActiveModel {
            id: Set(tenant.id),
            is_active: Set(false),
            ..Default::default()
        }.update(&db).await.unwrap();
        let tx = db.begin().await.unwrap();
        assert!(lock_invite_for_acceptance(&tx, tenant.id, &observed)
            .await.unwrap().is_none());
        tx.rollback().await.unwrap();
        tenants::ActiveModel {
            id: Set(tenant.id),
            is_active: Set(true),
            ..Default::default()
        }.update(&db).await.unwrap();

        // The unlocked locator must not authorize a different issuer.
        auth_invites::ActiveModel {
            id: Set(observed.id),
            issued_by: Set(Some(other.id)),
            ..Default::default()
        }
        .update(&db)
        .await
        .unwrap();
        let tx = db.begin().await.unwrap();
        assert!(
            lock_invite_for_acceptance(&tx, tenant.id, &observed)
                .await
                .unwrap()
                .is_none()
        );
        tx.rollback().await.unwrap();

        // A revocation committed after the preflight remains terminal.
        auth_invites::ActiveModel {
            id: Set(observed.id),
            issued_by: Set(Some(issuer.id)),
            revoked_at: Set(Some(now.into())),
            ..Default::default()
        }
        .update(&db)
        .await
        .unwrap();
        let tx = db.begin().await.unwrap();
        assert!(
            lock_invite_for_acceptance(&tx, tenant.id, &observed)
                .await
                .unwrap()
                .is_none()
        );
        tx.rollback().await.unwrap();
    }

    #[tokio::test]
    async fn disabled_invites_reject_before_token_decode() {
        let db = sea_orm::Database::connect("sqlite::memory:")
            .await
            .expect("sqlite test database");
        let mut settings = RustokSettings::default();
        settings.features.invites_enabled = false;
        let ctx = ServerRuntimeContext::new(db, settings);
        let config =
            crate::auth::test_auth_config("invite-feature-gate-secret-for-tests".to_string());

        let error = AuthLifecycleService::accept_invite_once_runtime(
            &ctx,
            &config,
            uuid::Uuid::new_v4(),
            "not-a-jwt",
            "Password123!",
            None,
        )
        .await
        .expect_err("disabled invites must reject");

        assert!(matches!(error, InviteAcceptanceError::InvalidToken));
    }

    fn invite_ledger(email: &str, expires_at: DateTime<Utc>) -> auth_invites::Model {
        let now = Utc::now();
        auth_invites::Model {
            id: uuid::Uuid::new_v4(),
            tenant_id: uuid::Uuid::new_v4(),
            token_hash: "invite-digest".to_string(),
            email: email.to_string(),
            role: "customer".to_string(),
            issued_by: None,
            issuer_recovery_revision: None,
            issuer_membership_revision: None,
            role_grant_binding: None,
            module_policy_binding: None,
            expires_at: expires_at.into(),
            revoked_at: None,
            created_at: now.into(),
        }
    }

    #[tokio::test]
    async fn invite_account_revision_rejects_status_aba_and_legacy_binding() {
        use crate::models::{tenants, users};
        use rustok_migrations::SqliteTestMigrator;
        use rustok_test_utils::db::setup_test_db_with_migrations;

        let db = setup_test_db_with_migrations::<SqliteTestMigrator>().await;
        let tenant = tenants::ActiveModel::new("Tenant", "invite-account-epoch")
            .insert(&db)
            .await
            .expect("tenant");
        let issuer = users::ActiveModel::new(tenant.id, "issuer@example.test", "hash")
            .insert(&db)
            .await
            .expect("issuer");
        let mut invite = invite_ledger("invitee@example.test", Utc::now() + Duration::hours(1));
        invite.tenant_id = tenant.id;
        invite.issued_by = Some(issuer.id);
        assert!(!invite_issuer_matches_account(&invite, &issuer));
        invite.issuer_recovery_revision = Some(issuer.recovery_revision);
        assert!(invite_issuer_matches_account(&invite, &issuer));

        // Unrelated updates must not invalidate the account epoch.
        users::ActiveModel {
            id: Set(issuer.id),
            name: Set(Some("Updated name".to_string())),
            ..Default::default()
        }
        .update(&db)
        .await
        .expect("profile update");
        let current = users::Entity::find_by_id(issuer.id)
            .one(&db)
            .await
            .unwrap()
            .unwrap();
        assert!(invite_issuer_matches_account(&invite, &current));

        for status in [
            rustok_core::UserStatus::Inactive,
            rustok_core::UserStatus::Active,
        ] {
            users::ActiveModel {
                id: Set(issuer.id),
                status: Set(status),
                ..Default::default()
            }
            .update(&db)
            .await
            .expect("status change");
            let current = users::Entity::find_by_id(issuer.id)
                .one(&db)
                .await
                .unwrap()
                .unwrap();
            assert!(!invite_issuer_matches_account(&invite, &current));
        }
        let current = users::Entity::find_by_id(issuer.id)
            .one(&db)
            .await
            .unwrap()
            .unwrap();
        assert!(current.is_active());
        assert!(current.recovery_revision > issuer.recovery_revision);
        // Only a freshly issued invite can bind the new account state.
        invite.issuer_recovery_revision = Some(current.recovery_revision);
        assert!(invite_issuer_matches_account(&invite, &current));
        invite.tenant_id = uuid::Uuid::new_v4();
        assert!(!invite_issuer_matches_account(&invite, &current));
    }

    #[test]
    fn invite_acceptance_rechecks_ledger_email_and_expiry() {
        let now = Utc::now();
        let token_expiry = now + Duration::hours(1);

        let mut matching = invite_ledger("INVITEE@example.test", token_expiry.clone());
        matching.created_at = now.into();
        assert!(invite_ledger_matches_claims(
            &matching,
            "invitee@example.test",
            token_expiry.clone(),
            now.clone(),
        ));

        let mut future = matching.clone();
        future.created_at = (now + Duration::seconds(1)).into();
        assert!(!invite_ledger_matches_claims(
            &future,
            "invitee@example.test",
            token_expiry,
            now,
        ));

        let wrong_email = invite_ledger("other@example.test", token_expiry.clone());
        assert!(!invite_ledger_matches_claims(
            &wrong_email,
            "invitee@example.test",
            token_expiry.clone(),
            now.clone(),
        ));

        let expired_ledger =
            invite_ledger("invitee@example.test", now.clone() - Duration::seconds(1));
        assert!(!invite_ledger_matches_claims(
            &expired_ledger,
            "invitee@example.test",
            token_expiry.clone(),
            now.clone(),
        ));

        let ledger_extends_past_signed_token = invite_ledger(
            "invitee@example.test",
            token_expiry.clone() + Duration::seconds(1),
        );
        assert!(!invite_ledger_matches_claims(
            &ledger_extends_past_signed_token,
            "invitee@example.test",
            token_expiry.clone(),
            now.clone(),
        ));

        let shortened_but_live_ledger =
            invite_ledger("invitee@example.test", now + Duration::minutes(5));
        assert!(invite_ledger_matches_claims(
            &shortened_but_live_ledger,
            "invitee@example.test",
            token_expiry,
            Utc::now(),
        ));
    }

    #[tokio::test]
    async fn consumed_invite_rejects_before_password_policy() {
        use crate::models::tenants;
        use rustok_migrations::SqliteTestMigrator;
        use rustok_test_utils::db::setup_test_db_with_migrations;

        let db = setup_test_db_with_migrations::<SqliteTestMigrator>().await;
        let tenant = tenants::ActiveModel::new("Tenant", "invite-consumed-preflight")
            .insert(&db)
            .await
            .unwrap();
        let issuer = users::ActiveModel::new(tenant.id, "issuer@example.test", "hash")
            .insert(&db)
            .await
            .unwrap();
        let config =
            crate::auth::test_auth_config("invite-preflight-test-secret-key-32b".to_string());
        let email = "invitee@example.test";
        let token = crate::auth::encode_invite_token(
            &config,
            tenant.id,
            email,
            rustok_core::UserRole::Customer,
            None,
            3600,
        )
        .unwrap();
        let claims = decode_invite_token(&config, &token).unwrap();
        let expiry =
            DateTime::<Utc>::from_timestamp(i64::try_from(claims.exp).unwrap(), 0).unwrap();
        let now = Utc::now();
        let token_hash = invite_token_hash(&token);
        auth_invites::ActiveModel {
            id: Set(uuid::Uuid::new_v4()),
            tenant_id: Set(tenant.id),
            token_hash: Set(token_hash.clone()),
            email: Set(email.to_string()),
            role: Set("customer".to_string()),
            issued_by: Set(Some(issuer.id)),
            issuer_recovery_revision: Set(Some(issuer.recovery_revision)),
            issuer_membership_revision: Set(None),
            role_grant_binding: Set(None),
            module_policy_binding: Set(None),
            expires_at: Set(expiry.into()),
            revoked_at: Set(None),
            created_at: Set(now.into()),
        }
        .insert(&db)
        .await
        .unwrap();
        auth_invite_consumptions::ActiveModel {
            id: Set(uuid::Uuid::new_v4()),
            tenant_id: Set(tenant.id),
            token_hash: Set(token_hash.clone()),
            email: Set(email.to_string()),
            role: Set("customer".to_string()),
            user_id: Set(None),
            expires_at: Set(expiry.into()),
            consumed_at: Set(now.into()),
        }
        .insert(&db)
        .await
        .unwrap();
        let mut settings = RustokSettings::default();
        settings.features.invites_enabled = true;
        let ctx = ServerRuntimeContext::new(db.clone(), settings);
        assert!(matches!(
            AuthLifecycleService::accept_invite_once_runtime(
                &ctx, &config, tenant.id, &token, "", None,
            )
            .await,
            Err(InviteAcceptanceError::InvalidToken)
        ));
        assert!(
            invite_already_consumed(&db, tenant.id, &token_hash)
                .await
                .unwrap()
        );
    }

    #[test]
    fn invite_digest_is_stable_and_does_not_store_plaintext() {
        let token = "signed.invite.token";
        let digest = hex::encode(Sha256::digest(token.as_bytes()));

        assert_eq!(digest.len(), 64);
        assert_ne!(digest, token);
        assert_eq!(digest, hex::encode(Sha256::digest(token.as_bytes())));
        assert_eq!(invite_token_hash(&format!("  {token}  ")), digest);
    }
}
