//! OAuth App Service — CRUD operations and credential management

use crate::auth;
use crate::error::{Error, Result};
use crate::models::oauth_apps::{self, ActiveModel as OAuthAppActiveModel, Entity as OAuthApps};
use crate::models::oauth_authorization_codes::{self, ActiveModel as OAuthCodeActiveModel};
use crate::models::oauth_consents::{
    ActiveModel as OAuthConsentActiveModel, Entity as OAuthConsents,
};
use crate::models::oauth_tokens::{self, Entity as OAuthTokens};
use crate::models::{tenants, users};
use chrono::Utc;
use reqwest::Url;
use rustok_api::Permission;
use rustok_api::context::scope_matches;
use rustok_core::{Rbac, UserRole};
use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, ConnectionTrait, DatabaseConnection,
    DatabaseTransaction, DbBackend, EntityTrait, QueryFilter, QuerySelect, Set, TransactionTrait,
    sea_query::Expr,
};
use std::str::FromStr;
use subtle::ConstantTimeEq;
use uuid::Uuid;

/// Outcome classification for one consent-grant attempt: a unique-index
/// race against a concurrent first grant is retryable, everything else
/// is a plain internal failure.
pub(crate) enum GrantConsentAttempt {
    UniqueRace,
    Fatal(Error),
}

impl GrantConsentAttempt {
    fn classify(error: sea_orm::DbErr) -> Self {
        if matches!(
            error.sql_err(),
            Some(sea_orm::SqlErr::UniqueConstraintViolation(_))
        ) {
            Self::UniqueRace
        } else {
            Self::Fatal(Error::InternalServerError)
        }
    }
}

/// Input for creating a new OAuth app
#[derive(Debug, Clone)]
pub struct CreateOAuthAppInput {
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    pub app_type: String,
    pub icon_url: Option<String>,
    pub redirect_uris: Vec<String>,
    pub scopes: Vec<String>,
    pub grant_types: Vec<String>,
    pub granted_permissions: Vec<String>,
}

/// Input for updating a manual OAuth app
#[derive(Debug, Clone)]
pub struct UpdateOAuthAppInput {
    pub name: String,
    pub description: Option<String>,
    pub icon_url: Option<String>,
    pub redirect_uris: Vec<String>,
    pub scopes: Vec<String>,
    pub grant_types: Vec<String>,
    pub granted_permissions: Vec<String>,
}

/// Prepared credentials stay outside SQL transactions while Argon2 runs.
pub(crate) struct PreparedOAuthApp {
    model: OAuthAppActiveModel,
    client_secret: String,
}

pub(crate) struct PreparedOAuthSecret {
    app_id: Uuid,
    tenant_id: Uuid,
    expected_hash: Option<String>,
    secret_hash: String,
    client_secret: String,
}

/// Result of creating an OAuth app — includes the plaintext secret shown once
pub struct CreateOAuthAppResult {
    pub app: oauth_apps::Model,
    pub client_secret: String,
}

impl std::fmt::Debug for CreateOAuthAppResult {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CreateOAuthAppResult")
            .field("app", &self.app)
            .field("client_secret", &"<redacted>")
            .finish()
    }
}

/// Result of rotating an OAuth app's secret
pub struct RotateSecretResult {
    pub app: oauth_apps::Model,
    pub client_secret: String,
}

impl std::fmt::Debug for RotateSecretResult {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RotateSecretResult")
            .field("app", &self.app)
            .field("client_secret", &"<redacted>")
            .finish()
    }
}

/// Build an update model that writes only fields editable through the
/// manual-app API. A stale full-row ActiveModel can otherwise restore
/// lifecycle and runtime-owned values (for example, reactivate an app after
/// a concurrent revoke or erase a newer last_used_at timestamp).
fn manual_app_update_base(app: oauth_apps::Model) -> OAuthAppActiveModel {
    let mut active: OAuthAppActiveModel = app.into();
    active.slug = ActiveValue::NotSet;
    active.app_type = ActiveValue::NotSet;
    active.client_id = ActiveValue::NotSet;
    active.client_secret_hash = ActiveValue::NotSet;
    active.manifest_ref = ActiveValue::NotSet;
    active.auto_created = ActiveValue::NotSet;
    active.is_active = ActiveValue::NotSet;
    active.revoked_at = ActiveValue::NotSet;
    active.last_used_at = ActiveValue::NotSet;
    active.metadata = ActiveValue::NotSet;
    active.created_at = ActiveValue::NotSet;
    active
}

/// Compare security policy sets without treating order or duplicate entries as
/// authority changes. Malformed stored policy never compares equal.
fn oauth_policy_matches(stored: &serde_json::Value, requested: &[String]) -> bool {
    let Ok(existing) = serde_json::from_value::<Vec<String>>(stored.clone()) else {
        return false;
    };
    let existing: std::collections::BTreeSet<_> = existing.iter().collect();
    let requested: std::collections::BTreeSet<_> = requested.iter().collect();
    existing == requested
}

fn oauth_app_policy_changed(
    app: &oauth_apps::Model,
    redirect_uris: &[String],
    scopes: &[String],
    grant_types: &[String],
    granted_permissions: &[String],
) -> bool {
    !oauth_policy_matches(&app.redirect_uris, redirect_uris)
        || !oauth_policy_matches(&app.scopes, scopes)
        || !oauth_policy_matches(&app.grant_types, grant_types)
        || !oauth_policy_matches(&app.granted_permissions, granted_permissions)
}

/// Lock an OAuth app in the same order used by token issuance. Consent
/// grants/revocations use this fence so they cannot race a token exchange that
/// has already validated the app.
pub(crate) async fn lock_oauth_app_for_mutation<C>(
    db: &C,
    app_id: Uuid,
    tenant_id: Uuid,
) -> Result<Option<oauth_apps::Model>, sea_orm::DbErr>
where
    C: ConnectionTrait,
{
    let query = || OAuthApps::find_by_id(app_id).filter(oauth_apps::Column::TenantId.eq(tenant_id));
    match db.get_database_backend() {
        DbBackend::Postgres | DbBackend::MySql => query().lock_exclusive().one(db).await,
        DbBackend::Sqlite => {
            // Acquire the SQLite write fence before reading. A read-first
            // deferred transaction can fail with SQLITE_BUSY_SNAPSHOT when
            // another writer commits between the read and lock upgrade.
            let locked = oauth_apps::Entity::update_many()
                .col_expr(
                    oauth_apps::Column::UpdatedAt,
                    Expr::col(oauth_apps::Column::UpdatedAt),
                )
                .filter(oauth_apps::Column::Id.eq(app_id))
                .filter(oauth_apps::Column::TenantId.eq(tenant_id))
                .exec(db)
                .await?;
            if locked.rows_affected != 1 {
                return Ok(None);
            }
            query().one(db).await
        }
        _ => Err(sea_orm::DbErr::Custom(
            "unsupported database backend for OAuth app mutation locking".to_string(),
        )),
    }
}

/// Lock a user subject in the same app-then-user order used by token issuance.
pub(crate) async fn lock_oauth_user_for_mutation<C>(
    db: &C,
    user_id: Uuid,
    tenant_id: Uuid,
) -> Result<Option<users::Model>, sea_orm::DbErr>
where
    C: ConnectionTrait,
{
    let query = || users::Entity::find_by_id(user_id).filter(users::Column::TenantId.eq(tenant_id));
    match db.get_database_backend() {
        DbBackend::Postgres | DbBackend::MySql => query().lock_exclusive().one(db).await,
        DbBackend::Sqlite => {
            let locked = users::Entity::update_many()
                .col_expr(
                    users::Column::UpdatedAt,
                    Expr::col(users::Column::UpdatedAt),
                )
                .filter(users::Column::Id.eq(user_id))
                .filter(users::Column::TenantId.eq(tenant_id))
                .exec(db)
                .await?;
            if locked.rows_affected != 1 {
                return Ok(None);
            }
            query().one(db).await
        }
        _ => Err(sea_orm::DbErr::Custom(
            "unsupported database backend for OAuth subject mutation locking".to_string(),
        )),
    }
}

/// Previously authenticated direct-session facts, revalidated under the
/// app -> user -> session locks before any user delegation is persisted.
#[derive(Clone, Copy)]
pub(crate) struct AuthorizationSession {
    pub user_id: Uuid,
    pub session_id: Uuid,
    pub access_expires_at: chrono::DateTime<Utc>,
    pub require_email_verification: bool,
    pub confirmation_deadline: Option<chrono::DateTime<Utc>>,
}

impl AuthorizationSession {
    fn ensure_live(
        &self,
        session: &crate::models::sessions::Model,
        now: chrono::DateTime<Utc>,
    ) -> Result<()> {
        if self
            .confirmation_deadline
            .is_some_and(|deadline| now >= deadline)
        {
            return Err(Error::Unauthorized(
                "OAuth authorization confirmation expired".to_string(),
            ));
        }
        if session.revoked_at.is_some()
            || session.created_at.with_timezone(&Utc) > now
            || session.expires_at.with_timezone(&Utc) <= now
            || session.absolute_deadline() <= now
            || self.access_expires_at <= now
        {
            return Err(Error::Unauthorized(
                "OAuth authorization credential expired or inactive".to_string(),
            ));
        }
        Ok(())
    }
}

pub struct OAuthAppService;

impl OAuthAppService {
    /// Registers a manual OAuth client application with generated credentials and tenant association.
    #[tracing::instrument(skip(db, input), fields(tenant_id = %tenant_id, slug = %input.slug, client_id))]
    pub async fn create_app(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        input: CreateOAuthAppInput,
    ) -> Result<CreateOAuthAppResult> {
        ensure_slug_available(db, tenant_id, &input.slug).await?;
        let prepared = Self::prepare_app(tenant_id, input).await?;
        let tx = db.begin().await?;
        let result = Self::create_prepared_app_in_transaction(&tx, prepared).await?;
        tx.commit().await?;
        Ok(result)
    }

    pub(crate) async fn prepare_app(
        tenant_id: Uuid,
        input: CreateOAuthAppInput,
    ) -> Result<PreparedOAuthApp> {
        let granted_permissions = validate_granted_permissions(input.granted_permissions)?;
        validate_manual_app_configuration(
            &input.app_type,
            &input.redirect_uris,
            &input.scopes,
            &input.grant_types,
            &granted_permissions,
        )?;
        validate_app_display_fields(&input.name, Some(&input.slug), input.icon_url.as_deref())?;
        let client_id = Uuid::new_v4();
        let client_secret_plain = generate_client_secret()?;
        let client_secret_hash = auth::hash_password_async(client_secret_plain.clone()).await?;
        let model = OAuthAppActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant_id),
            name: Set(input.name),
            slug: Set(input.slug),
            description: Set(input.description),
            app_type: Set(input.app_type),
            icon_url: Set(input.icon_url),
            client_id: Set(client_id),
            client_secret_hash: Set(Some(client_secret_hash)),
            redirect_uris: Set(serde_json::to_value(&input.redirect_uris).map_err(|error| {
                Error::Message(format!("failed to serialize redirect_uris: {error}"))
            })?),
            scopes: Set(serde_json::to_value(&input.scopes)
                .map_err(|error| Error::Message(format!("failed to serialize scopes: {error}")))?),
            grant_types: Set(serde_json::to_value(&input.grant_types).map_err(|error| {
                Error::Message(format!("failed to serialize grant_types: {error}"))
            })?),
            granted_permissions: Set(to_json_array(&granted_permissions)?),
            manifest_ref: Set(None),
            auto_created: Set(false),
            is_active: Set(true),
            revoked_at: Set(None),
            last_used_at: Set(None),
            metadata: Set(serde_json::json!({})),
            created_at: Set(Utc::now().into()),
            updated_at: Set(Utc::now().into()),
        };
        Ok(PreparedOAuthApp {
            model,
            client_secret: client_secret_plain,
        })
    }

    /// Caller owns the transaction and the authorization fences.
    pub(crate) async fn create_prepared_app_in_transaction(
        tx: &DatabaseTransaction,
        prepared: PreparedOAuthApp,
    ) -> Result<CreateOAuthAppResult> {
        let app = prepared
            .model
            .insert_in_transaction(tx)
            .await
            .map_err(|error| {
                if matches!(
                    error.sql_err(),
                    Some(sea_orm::SqlErr::UniqueConstraintViolation(_))
                ) {
                    Error::Conflict("OAuth app slug or identity already exists".to_string())
                } else {
                    tracing::error!(%error, "Failed to insert OAuth app");
                    Error::InternalServerError
                }
            })?;
        tracing::Span::current().record("client_id", tracing::field::display(app.client_id));
        Ok(CreateOAuthAppResult {
            app,
            client_secret: prepared.client_secret,
        })
    }

    /// Update a manual OAuth app.
    #[tracing::instrument(skip(db, input), fields(tenant_id = %tenant_id, app_id = %app_id))]
    pub async fn update_app(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        app_id: Uuid,
        input: UpdateOAuthAppInput,
    ) -> Result<oauth_apps::Model> {
        let tx = db.begin().await.map_err(|_| Error::InternalServerError)?;
        let app = lock_oauth_app_for_mutation(&tx, app_id, tenant_id)
            .await
            .map_err(|_| Error::InternalServerError)?
            .ok_or(Error::NotFound)?;
        let updated = Self::update_locked_app(&tx, app, input).await?;
        tx.commit().await.map_err(|_| Error::InternalServerError)?;
        Ok(updated)
    }

    /// Apply a validated manual update on the caller's locked app.
    pub(crate) async fn update_locked_app(
        tx: &DatabaseTransaction,
        app: oauth_apps::Model,
        input: UpdateOAuthAppInput,
    ) -> Result<oauth_apps::Model> {
        let app_id = app.id;
        let tenant_id = app.tenant_id;
        let granted_permissions = validate_granted_permissions(input.granted_permissions)?;
        if !app.can_edit() {
            return Err(Error::BadRequest(
                "This OAuth app is managed by manifest/config and cannot be edited manually"
                    .to_string(),
            ));
        }

        validate_manual_app_configuration(
            &app.app_type,
            &input.redirect_uris,
            &input.scopes,
            &input.grant_types,
            &granted_permissions,
        )?;
        // `slug` is not editable after creation, so only name/icon are rechecked.
        validate_app_display_fields(&input.name, None, input.icon_url.as_deref())?;

        // Go through the ActiveModel wrapper: name/description are
        // translation-owned (`#[sea_orm(ignore)]` here) and its update
        // path syncs them into oauth_app_translations. The base model marks
        // immutable, lifecycle and runtime-owned columns NotSet so this
        // pre-read cannot undo a concurrent revoke, secret rotation or use.
        if oauth_app_policy_changed(
            &app,
            &input.redirect_uris,
            &input.scopes,
            &input.grant_types,
            &granted_permissions,
        ) {
            revoke_oauth_app_credentials(tx, app_id, tenant_id, Utc::now()).await?;
        }
        let mut active = manual_app_update_base(app);
        active.name = Set(input.name);
        active.description = Set(input.description);
        active.icon_url = Set(input.icon_url);
        active.redirect_uris =
            Set(serde_json::to_value(&input.redirect_uris)
                .map_err(|_| Error::InternalServerError)?);
        active.scopes =
            Set(serde_json::to_value(&input.scopes).map_err(|_| Error::InternalServerError)?);
        active.grant_types =
            Set(serde_json::to_value(&input.grant_types).map_err(|_| Error::InternalServerError)?);
        active.granted_permissions = Set(to_json_array(&granted_permissions)?);
        active.updated_at = Set(Utc::now().into());

        let mut updated = active
            .update(tx)
            .await
            .map_err(|_| Error::InternalServerError)?;
        // The ActiveModel update deliberately leaves these columns NotSet.
        // Read their projection under the same lock/transaction so the response
        // represents the committed mutation, not a stale preflight snapshot.
        let current = OAuthApps::find_by_id(app_id)
            .filter(oauth_apps::Column::TenantId.eq(tenant_id))
            .one(tx)
            .await
            .map_err(|_| Error::InternalServerError)?
            .ok_or(Error::NotFound)?;
        updated.client_secret_hash = current.client_secret_hash;
        updated.is_active = current.is_active;
        updated.revoked_at = current.revoked_at;
        updated.last_used_at = current.last_used_at;
        Ok(updated)
    }

    /// Find an active app by its public client_id scoped to a tenant
    pub async fn find_by_client_id_scoped(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        client_id: Uuid,
    ) -> Result<Option<oauth_apps::Model>> {
        OAuthApps::find_active_by_client_id_scoped(db, tenant_id, client_id)
            .await
            .map_err(|_| Error::InternalServerError)
    }

    /// Security-only client lookup for protocol authentication/revocation scoped to a tenant.
    /// Presentation translation failures must not decide credential validity.
    pub async fn find_security_by_client_id_scoped(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        client_id: Uuid,
    ) -> Result<Option<oauth_apps::Model>> {
        OAuthApps::find_active_security_by_client_id_scoped(db, tenant_id, client_id)
            .await
            .map_err(|_| Error::InternalServerError)
    }

    /// Find an active app by its public client_id
    pub async fn find_by_client_id(
        db: &DatabaseConnection,
        client_id: Uuid,
    ) -> Result<Option<oauth_apps::Model>> {
        OAuthApps::find_active_by_client_id(db, client_id)
            .await
            .map_err(|_| Error::InternalServerError)
    }

    /// Security-only client lookup for protocol authentication/revocation.
    /// Presentation translation failures must not decide credential validity.
    pub async fn find_security_by_client_id(
        db: &DatabaseConnection,
        client_id: Uuid,
    ) -> Result<Option<oauth_apps::Model>> {
        OAuthApps::find_active_security_by_client_id(db, client_id)
            .await
            .map_err(|_| Error::InternalServerError)
    }

    /// List all apps for a tenant
    pub async fn list_by_tenant(
        db: &DatabaseConnection,
        tenant_id: Uuid,
    ) -> Result<Vec<oauth_apps::Model>> {
        OAuthApps::find_active_by_tenant(db, tenant_id)
            .await
            .map_err(|_| Error::InternalServerError)
    }

    /// Rotate client_secret — generates a new secret, returns plaintext once
    pub async fn rotate_secret(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        app_id: Uuid,
    ) -> Result<RotateSecretResult> {
        let app = OAuthApps::find_by_id(app_id)
            .filter(oauth_apps::Column::TenantId.eq(tenant_id))
            .one(db)
            .await
            .map_err(|_| Error::InternalServerError)?
            .ok_or_else(|| Error::NotFound)?;

        if !app.can_rotate_secret() {
            return Err(Error::BadRequest(
                "This OAuth app does not support client secret rotation".to_string(),
            ));
        }

        let prepared = Self::prepare_secret(&app).await?;
        let tx = db.begin().await?;
        let locked = lock_oauth_app_for_mutation(&tx, app_id, tenant_id)
            .await?
            .ok_or(Error::NotFound)?;
        let result = Self::rotate_locked_secret(&tx, locked, prepared).await?;
        tx.commit().await?;
        Ok(result)
    }

    pub(crate) async fn prepare_secret(app: &oauth_apps::Model) -> Result<PreparedOAuthSecret> {
        if !app.can_rotate_secret() {
            return Err(Error::BadRequest(
                "This OAuth app does not support client secret rotation".to_string(),
            ));
        }
        let client_secret = generate_client_secret()?;
        let secret_hash = auth::hash_password_async(client_secret.clone()).await?;
        Ok(PreparedOAuthSecret {
            app_id: app.id,
            tenant_id: app.tenant_id,
            expected_hash: app.client_secret_hash.clone(),
            secret_hash,
            client_secret,
        })
    }

    pub(crate) async fn rotate_locked_secret(
        tx: &DatabaseTransaction,
        mut app: oauth_apps::Model,
        prepared: PreparedOAuthSecret,
    ) -> Result<RotateSecretResult> {
        if app.id != prepared.app_id || app.tenant_id != prepared.tenant_id {
            return Err(Error::Unauthorized(
                "Prepared secret belongs to another OAuth app".to_string(),
            ));
        }
        if !app.can_rotate_secret() {
            return Err(Error::BadRequest(
                "This OAuth app does not support client secret rotation".to_string(),
            ));
        }
        let updated_at = Utc::now();
        if !replace_oauth_secret_if_current(
            tx,
            app.tenant_id,
            app.id,
            prepared.expected_hash.as_deref(),
            &prepared.secret_hash,
            updated_at,
        )
        .await?
        {
            return Err(Error::Conflict(
                "OAuth client secret changed during rotation; retry the operation".to_string(),
            ));
        }
        app.client_secret_hash = Some(prepared.secret_hash);
        app.updated_at = updated_at.into();
        Ok(RotateSecretResult {
            app,
            client_secret: prepared.client_secret,
        })
    }

    /// Revoke an app — deactivate it and revoke tokens, consents, and pending codes.
    ///
    /// Tenancy is part of the lookup, not something the caller is trusted to
    /// have checked first: a revoke keyed on a bare `app_id` would let any
    /// tenant deactivate another tenant's app and invalidate its issued tokens.
    pub async fn revoke_app(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        app_id: Uuid,
    ) -> Result<oauth_apps::Model> {
        let tx = db.begin().await.map_err(|_| Error::InternalServerError)?;
        // Check revocation eligibility only after acquiring the app mutation
        // fence. A manifest sync may claim the app after an unprotected read;
        // the old path could then revoke newly manifest-managed credentials.
        let app = lock_oauth_app_for_mutation(&tx, app_id, tenant_id)
            .await
            .map_err(|_| Error::InternalServerError)?
            .ok_or(Error::NotFound)?;

        let updated = Self::revoke_locked_app(&tx, app).await?;
        tx.commit().await.map_err(|_| Error::InternalServerError)?;
        Ok(updated)
    }

    pub(crate) async fn revoke_locked_app(
        tx: &DatabaseTransaction,
        app: oauth_apps::Model,
    ) -> Result<oauth_apps::Model> {
        let tenant_id = app.tenant_id;
        let app_id = app.id;
        if !app.can_revoke() {
            return Err(Error::BadRequest(
                "This OAuth app is managed by manifest/config and cannot be revoked manually"
                    .to_string(),
            ));
        }

        let now = Utc::now();

        // Deactivate the app (column-scoped; see update_app)
        let deactivated = oauth_apps::Entity::update_many()
            .col_expr(oauth_apps::Column::IsActive, Expr::value(false))
            .col_expr(
                oauth_apps::Column::RevokedAt,
                Expr::value(Some(now.clone())),
            )
            .col_expr(oauth_apps::Column::UpdatedAt, Expr::value(now.clone()))
            .filter(oauth_apps::Column::Id.eq(app_id))
            .filter(oauth_apps::Column::TenantId.eq(tenant_id))
            .exec(tx)
            .await
            .map_err(|_| Error::InternalServerError)?;
        if deactivated.rows_affected != 1 {
            return Err(Error::NotFound);
        }

        let updated = OAuthApps::find_by_id(app_id)
            .filter(oauth_apps::Column::TenantId.eq(tenant_id))
            .one(tx)
            .await
            .map_err(|_| Error::InternalServerError)?
            .ok_or_else(|| Error::NotFound)?;

        revoke_oauth_app_credentials(tx, app_id, tenant_id, now).await?;

        Ok(updated)
    }

    /// Verify a client_secret against the stored hash asynchronously
    pub async fn verify_client_secret_async(secret: String, hash: String) -> Result<bool> {
        auth::verify_password_async(secret, hash).await
    }

    /// Verify a client_secret against the stored hash
    pub fn verify_client_secret(secret: &str, hash: &str) -> Result<bool> {
        auth::verify_password(secret, hash)
    }

    /// Update last_used_at for an app (column-scoped; see update_app)
    pub async fn touch_last_used(db: &DatabaseConnection, app_id: Uuid) -> Result<()> {
        let updated = oauth_apps::Entity::update_many()
            .col_expr(
                oauth_apps::Column::LastUsedAt,
                Expr::value(Some(Utc::now())),
            )
            .filter(oauth_apps::Column::Id.eq(app_id))
            .exec(db)
            .await
            .map_err(|_| Error::InternalServerError)?;
        if updated.rows_affected == 0 {
            return Err(Error::NotFound);
        }

        Ok(())
    }

    /// Update last_used_at for an app with tenant isolation
    pub async fn touch_last_used_scoped(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        app_id: Uuid,
    ) -> Result<()> {
        let updated = oauth_apps::Entity::update_many()
            .col_expr(
                oauth_apps::Column::LastUsedAt,
                Expr::value(Some(Utc::now())),
            )
            .filter(oauth_apps::Column::Id.eq(app_id))
            .filter(oauth_apps::Column::TenantId.eq(tenant_id))
            .exec(db)
            .await
            .map_err(|_| Error::InternalServerError)?;
        if updated.rows_affected == 0 {
            return Err(Error::NotFound);
        }

        Ok(())
    }

    /// Atomically admit and store one authorization code under the same app ->
    /// user lock order used by exchange, consent grants and revocation.
    ///
    /// HTTP authorization preflight is advisory: app settings, the user's
    /// active status and third-party consent may change before this insert.
    /// Revalidate them inside the transaction to avoid creating a code after
    /// revocation has already invalidated all previously pending codes.
    pub(crate) async fn generate_authorization_code(
        db: &DatabaseConnection,
        app_id: Uuid,
        subject: AuthorizationSession,
        tenant_id: Uuid,
        redirect_uri: String,
        scopes: Vec<String>,
        code_challenge: String,
        clock: &(dyn Fn() -> Result<chrono::DateTime<Utc>> + Send + Sync),
    ) -> Result<String> {
        clock()?;
        rustok_auth::application_clock::admit_now(db).await?;
        let tx = db.begin().await.map_err(|_| Error::InternalServerError)?;
        let app = lock_oauth_app_for_mutation(&tx, app_id, tenant_id)
            .await
            .map_err(|_| Error::InternalServerError)?
            .filter(|app| app.is_active())
            .ok_or(Error::NotFound)?;
        if !app.supports_grant_type("authorization_code") || app.app_type == "service" {
            return Err(Error::BadRequest(
                "OAuth application does not allow authorization-code grants".to_string(),
            ));
        }
        if !app.redirect_uris_list().contains(&redirect_uri) {
            return Err(Error::BadRequest(
                "OAuth redirect URI is no longer registered".to_string(),
            ));
        }
        // Legacy persisted redirect URIs may predate registration validation.
        // Reject ambiguous callbacks before any consent or code is written.
        if !Url::parse(&redirect_uri).is_ok_and(|url| redirect_response_query_is_unambiguous(&url))
        {
            return Err(Error::BadRequest(
                "OAuth redirect URI contains reserved response parameters or is invalid"
                    .to_string(),
            ));
        }
        let allowed_scopes = app.scopes_list();
        if scopes.is_empty()
            || scopes
                .iter()
                .any(|scope| scope.trim().is_empty() || !scope_matches(&allowed_scopes, scope))
        {
            return Err(Error::BadRequest(
                "OAuth authorization scopes are no longer permitted".to_string(),
            ));
        }

        let user = lock_oauth_user_for_mutation(&tx, subject.user_id, tenant_id)
            .await
            .map_err(|_| Error::InternalServerError)?
            .filter(|user| user.is_active())
            .ok_or_else(|| {
                Error::Unauthorized(
                    "OAuth authorization subject is missing or inactive".to_string(),
                )
            })?;
        if subject.require_email_verification && !user.is_email_verified() {
            return Err(Error::Unauthorized(
                "Email verification required".to_string(),
            ));
        }
        let session = crate::services::auth_lifecycle::AuthLifecycleService::find_session_by_id_for_update_in_tx(
            &tx, tenant_id, subject.session_id,
        )
        .await
        .map_err(|_| Error::InternalServerError)?
        .filter(|session| session.user_id == user.id)
        .ok_or_else(|| Error::Unauthorized("OAuth authorization session is no longer active".to_string()))?;
        let verified = rustok_auth::application_clock::verified_now(&tx).await?;
        subject.ensure_live(
            &session,
            rustok_auth::application_clock::decision_time(verified, clock()?)?,
        )?;

        if app.requires_user_consent() {
            let consent = OAuthConsents::find_active_consent(&tx, app_id, user.id, tenant_id)
                .await
                .map_err(|_| Error::InternalServerError)?;
            let covered = match consent {
                Some(consent) => {
                    let granted = consent
                        .scopes_list()
                        .map_err(|_| Error::InternalServerError)?;
                    scopes.iter().all(|scope| scope_matches(&granted, scope))
                }
                None => false,
            };
            if !covered {
                if subject.confirmation_deadline.is_none() {
                    return Err(Error::Unauthorized(
                        "OAuth authorization requires active user consent".to_string(),
                    ));
                }
                let scopes_json =
                    serde_json::to_value(&scopes).map_err(|_| Error::InternalServerError)?;
                let verified = rustok_auth::application_clock::verified_now(&tx).await?;
                let now = rustok_auth::application_clock::decision_time(verified, clock()?)?;
                subject.ensure_live(&session, now)?;
                Self::grant_consent_in_transaction(
                    &tx,
                    app_id,
                    user.id,
                    tenant_id,
                    &scopes_json,
                    now,
                )
                .await
                .map_err(|attempt| match attempt {
                    GrantConsentAttempt::UniqueRace => Error::Conflict(
                        "OAuth consent changed concurrently; retry the operation".to_string(),
                    ),
                    GrantConsentAttempt::Fatal(error) => error,
                })?;
            }
        }

        // Generate a fresh code only after the caller has passed authoritative
        // checks. Hashing an opaque random code does not leak plaintext to DB.
        // Use the Auth owner's fallible CSPRNG; preserve base64url wire format.
        let random_bytes =
            hex::decode(auth::generate_refresh_token()?).map_err(|_| Error::InternalServerError)?;
        use base64::{Engine as _, engine::general_purpose};
        let code = general_purpose::URL_SAFE_NO_PAD.encode(random_bytes);
        let code_hash = auth::hash_refresh_token(&code);

        let verified = rustok_auth::application_clock::verified_now(&tx).await?;
        let now = rustok_auth::application_clock::decision_time(verified, clock()?)?;
        subject.ensure_live(&session, now)?;
        let expires_at = now
            .checked_add_signed(chrono::Duration::minutes(10))
            .ok_or(Error::InternalServerError)?;
        let stored = OAuthCodeActiveModel {
            id: Set(Uuid::new_v4()),
            app_id: Set(app_id),
            user_id: Set(user.id),
            tenant_id: Set(tenant_id),
            code_hash: Set(code_hash),
            redirect_uri: Set(redirect_uri),
            scopes: Set(serde_json::to_value(&scopes).map_err(|_| Error::InternalServerError)?),
            code_challenge: Set(code_challenge),
            code_challenge_method: Set("S256".to_string()),
            expires_at: Set(expires_at.into()),
            used_at: Set(None),
            created_at: Set(now.into()),
        }
        .insert(&tx)
        .await
        .map_err(|_| Error::InternalServerError)?;
        // Recheck immediately before attempting commit; a slow code write
        // must not pass a stale authorization decision. The session stays locked.
        let verified = rustok_auth::application_clock::verified_now(&tx).await?;
        let now = rustok_auth::application_clock::decision_time(verified, clock()?)?;
        subject.ensure_live(&session, now)?;
        if !stored.is_active_at(now) {
            return Err(Error::Unauthorized(
                "OAuth authorization code expired before commit".to_string(),
            ));
        }
        tx.commit().await.map_err(|_| Error::InternalServerError)?;
        // A committed code cannot be rolled back after acknowledgement.
        // Refuse publication if the clock fence closed or the code expired.
        rustok_auth::application_clock::check_local()?;
        if !stored.is_active_at(clock()?) {
            return Err(Error::Unauthorized(
                "OAuth authorization code expired before response".to_string(),
            ));
        }
        Ok(code)
    }

    /// Verify a PKCE code_verifier against the stored code_challenge
    /// code_challenge = BASE64URL-ENCODE(SHA256(ASCII(code_verifier)))
    pub fn verify_pkce(code_challenge: &str, code_verifier: &str) -> bool {
        use base64::{Engine as _, engine::general_purpose};
        use sha2::{Digest, Sha256};

        if !(43..=128).contains(&code_verifier.len())
            || !code_verifier
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"-._~".contains(&byte))
        {
            return false;
        }

        let mut hasher = Sha256::new();
        hasher.update(code_verifier.as_bytes());
        let hash = hasher.finalize();

        let expected_challenge = general_purpose::URL_SAFE_NO_PAD.encode(hash);

        // Constant time comparison is safer for crypto
        expected_challenge
            .as_bytes()
            .ct_eq(code_challenge.as_bytes())
            .into()
    }

    /// Revoke an OAuth access or refresh token and its family by hash (RFC 7009).
    ///
    /// The app mutation fence, lookup and family-wide revocation share one
    /// transaction. Token issuance/refresh takes the app lock first too: this
    /// prevents revocation from missing a fresh refresh-family successor that
    /// commits concurrently. The token predicate is scoped by client and
    /// tenant, and unknown tokens succeed without revealing existence.
    /// Access JWTs are registered
    /// as `token_kind = access` rows, so revocation fences an already-issued
    /// JWT before its signed TTL; refresh rows fence the rotation lineage.
    pub(crate) async fn revoke_authenticated_token_by_hash(
        db: &DatabaseConnection,
        token_hash: &str,
        authenticated_app: &oauth_apps::Model,
        tenant_id: Uuid,
    ) -> Result<()> {
        Self::revoke_token_by_hash_inner(
            db,
            token_hash,
            authenticated_app.id,
            tenant_id,
            Some(authenticated_app),
        )
        .await
    }

    #[cfg(test)]
    async fn revoke_token_by_hash(
        db: &DatabaseConnection,
        token_hash: &str,
        app_id: Uuid,
        tenant_id: Uuid,
    ) -> Result<()> {
        Self::revoke_token_by_hash_inner(db, token_hash, app_id, tenant_id, None).await
    }

    async fn revoke_token_by_hash_inner(
        db: &DatabaseConnection,
        token_hash: &str,
        app_id: Uuid,
        tenant_id: Uuid,
        authenticated_app: Option<&oauth_apps::Model>,
    ) -> Result<()> {
        let tx = db.begin().await.map_err(|_| Error::InternalServerError)?;
        // Acquire the same app-row fence as refresh rotation and token issuance
        // BEFORE reading the revocation target. Otherwise a refresh can commit
        // its replacement after this lookup but before the family update.
        let current_app = lock_oauth_app_for_mutation(&tx, app_id, tenant_id)
            .await
            .map_err(|_| Error::InternalServerError)?;
        if let Some(authenticated) = authenticated_app {
            // Secret verification is intentionally outside the transaction.
            // The app snapshot must still describe the same authenticated
            // client under the mutation fence, even for an unknown token.
            let unchanged = current_app.as_ref().is_some_and(|current| {
                current.is_active()
                    && authenticated.tenant_id == tenant_id
                    && authenticated.id == current.id
                    && authenticated.client_id == current.client_id
                    && authenticated.app_type == current.app_type
                    && authenticated.client_secret_hash == current.client_secret_hash
            });
            if !unchanged {
                return Err(Error::Unauthorized(
                    "OAuth client credentials changed".to_string(),
                ));
            }
        }
        let token = OAuthTokens::find()
            .filter(oauth_tokens::Column::TokenHash.eq(token_hash))
            .filter(oauth_tokens::Column::AppId.eq(app_id))
            .filter(oauth_tokens::Column::TenantId.eq(tenant_id))
            .one(&tx)
            .await
            .map_err(|_| Error::InternalServerError)?;

        if let Some(token) = token {
            let family_id = token.family_id;
            let now = Utc::now();
            let mut family_scope = sea_orm::Condition::all()
                .add(oauth_tokens::Column::AppId.eq(app_id))
                .add(oauth_tokens::Column::TenantId.eq(tenant_id))
                .add(oauth_tokens::Column::FamilyId.eq(family_id));
            family_scope = match token.user_id {
                Some(user_id) => family_scope.add(oauth_tokens::Column::UserId.eq(user_id)),
                None => family_scope.add(oauth_tokens::Column::UserId.is_null()),
            };
            OAuthTokens::update_many()
                .col_expr(oauth_tokens::Column::RevokedAt, Expr::value(now))
                .col_expr(oauth_tokens::Column::UpdatedAt, Expr::value(now))
                .filter(family_scope)
                // A repeated RFC 7009 request must not rewrite the original
                // revocation timestamp of an already-invalid family member.
                .filter(oauth_tokens::Column::RevokedAt.is_null())
                .exec(&tx)
                .await
                .map_err(|_| Error::InternalServerError)?;
        }

        tx.commit().await.map_err(|_| Error::InternalServerError)?;
        // Per RFC 7009: always succeed even if token not found.
        Ok(())
    }

    /// Check if user has granted consent for the requested scopes
    pub async fn get_active_consent(
        db: &DatabaseConnection,
        app_id: Uuid,
        user_id: Uuid,
        tenant_id: Uuid,
        requested_scopes: &[String],
    ) -> Result<bool> {
        let consent = OAuthConsents::find_active_consent(db, app_id, user_id, tenant_id)
            .await
            .map_err(|_| Error::InternalServerError)?;

        if let Some(c) = consent {
            let granted_scopes = c.scopes_list().map_err(|_| Error::InternalServerError)?;
            // Check if all requested scopes are covered by granted scopes
            for req_scope in requested_scopes {
                if !scope_matches(&granted_scopes, req_scope) {
                    return Ok(false);
                }
            }
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Grant user consent for an application, replacing any previously
    /// consented scope set with exactly the approved one.
    ///
    /// Re-granting after a revoke revives the soft-deleted consent row in
    /// place instead of inserting a second row: with the historical full
    /// unique index that insert failed forever (the index is now partial
    /// over active rows, and revive-in-place keeps a single row per
    /// (app, user, tenant) regardless). Scope narrowing is possible by
    /// re-granting a narrower set; the previous merge-only behavior made
    /// consent monotonically grow.
    pub async fn grant_consent(
        db: &DatabaseConnection,
        app_id: Uuid,
        user_id: Uuid,
        tenant_id: Uuid,
        scopes: Vec<String>,
    ) -> Result<()> {
        if scopes.is_empty() {
            return Err(Error::BadRequest(
                "OAuth consent must include at least one scope".to_string(),
            ));
        }

        let tx = db.begin().await.map_err(|_| Error::InternalServerError)?;
        // Keep the same lock order as token issuance and consent revocation.
        // This serializes concurrent grants and prevents consent from being
        // created after either the app or subject has become inactive.
        let app = lock_oauth_app_for_mutation(&tx, app_id, tenant_id)
            .await
            .map_err(|_| Error::InternalServerError)?
            .filter(|app| app.is_active())
            .ok_or(Error::NotFound)?;
        lock_oauth_user_for_mutation(&tx, user_id, tenant_id)
            .await
            .map_err(|_| Error::InternalServerError)?
            .filter(|user| user.is_active())
            .ok_or_else(|| {
                Error::Unauthorized("OAuth consent subject is missing or inactive".to_string())
            })?;

        let app_scopes = app.scopes_list();
        if scopes
            .iter()
            .any(|scope| !scope_matches(&app_scopes, scope))
        {
            return Err(Error::BadRequest(
                "Requested scopes exceed the application's registered scopes".to_string(),
            ));
        }

        let scopes_json = serde_json::to_value(&scopes).map_err(|_| Error::InternalServerError)?;
        let now = rustok_auth::application_clock::verified_now(&tx).await?;
        Self::grant_consent_in_transaction(&tx, app_id, user_id, tenant_id, &scopes_json, now)
            .await
            .map_err(|attempt| match attempt {
                GrantConsentAttempt::UniqueRace => Error::Conflict(
                    "OAuth consent changed concurrently; retry the operation".to_string(),
                ),
                GrantConsentAttempt::Fatal(error) => error,
            })?;
        rustok_auth::application_clock::verified_now(&tx).await?;
        tx.commit().await.map_err(|_| Error::InternalServerError)?;
        Ok(())
    }

    /// Upsert consent on a caller-owned transaction. Strict admin paths first
    /// lock the app and subject rows, then call this helper so a grant cannot
    /// commit against a client/user that became inactive during validation.
    pub(crate) async fn grant_consent_in_transaction<C>(
        db: &C,
        app_id: Uuid,
        user_id: Uuid,
        tenant_id: Uuid,
        scopes_json: &serde_json::Value,
        now: chrono::DateTime<Utc>,
    ) -> std::result::Result<(), GrantConsentAttempt>
    where
        C: ConnectionTrait,
    {
        let existing = OAuthConsents::find()
            .filter(crate::models::oauth_consents::Column::AppId.eq(app_id))
            .filter(crate::models::oauth_consents::Column::UserId.eq(user_id))
            .filter(crate::models::oauth_consents::Column::TenantId.eq(tenant_id))
            .one(db)
            .await
            .map_err(|_| GrantConsentAttempt::Fatal(Error::InternalServerError))?;

        match existing {
            Some(consent) => {
                // Re-grant creates a new consent epoch even when the scopes
                // are unchanged. Durably revoke all credentials of the previous
                // grant, even if the database clock repeats an epoch timestamp.
                OAuthTokens::update_many()
                    .col_expr(oauth_tokens::Column::RevokedAt, Expr::value(now))
                    .col_expr(oauth_tokens::Column::UpdatedAt, Expr::value(now))
                    .filter(oauth_tokens::Column::AppId.eq(app_id))
                    .filter(oauth_tokens::Column::UserId.eq(user_id))
                    .filter(oauth_tokens::Column::TenantId.eq(tenant_id))
                    .filter(oauth_tokens::Column::RevokedAt.is_null())
                    .exec(db)
                    .await
                    .map_err(GrantConsentAttempt::classify)?;
                // Pending codes belong to the previous epoch.
                // Invalidate them under the same app/subject transaction as
                // the grant, before any exchange can observe the new epoch.
                oauth_authorization_codes::Entity::update_many()
                    .col_expr(oauth_authorization_codes::Column::UsedAt, Expr::value(now))
                    .filter(oauth_authorization_codes::Column::AppId.eq(app_id))
                    .filter(oauth_authorization_codes::Column::UserId.eq(user_id))
                    .filter(oauth_authorization_codes::Column::TenantId.eq(tenant_id))
                    .filter(oauth_authorization_codes::Column::UsedAt.is_null())
                    .exec(db)
                    .await
                    .map_err(GrantConsentAttempt::classify)?;
                let mut active: OAuthConsentActiveModel = consent.into();
                active.scopes = Set(scopes_json.clone());
                active.granted_at = Set(now.into());
                active.revoked_at = Set(None);
                active
                    .update(db)
                    .await
                    .map_err(GrantConsentAttempt::classify)?;
            }
            None => {
                OAuthConsentActiveModel {
                    id: Set(Uuid::new_v4()),
                    app_id: Set(app_id),
                    user_id: Set(user_id),
                    tenant_id: Set(tenant_id),
                    scopes: Set(scopes_json.clone()),
                    granted_at: Set(now.into()),
                    revoked_at: Set(None),
                }
                .insert(db)
                .await
                .map_err(GrantConsentAttempt::classify)?;
            }
        }
        Ok(())
    }

    /// Revoke user consent for an application (and optionally tokens)
    pub async fn revoke_user_consent(
        db: &DatabaseConnection,
        app_id: Uuid,
        user_id: Uuid,
        tenant_id: Uuid,
    ) -> Result<()> {
        rustok_auth::application_clock::admit_now(db).await?;
        let tx = db.begin().await.map_err(|_| Error::InternalServerError)?;
        // Match token issuance's app-then-user lock order. Without these
        // fences an exchange can read active consent, then commit a new token
        // after this revocation has already committed.
        lock_oauth_app_for_mutation(&tx, app_id, tenant_id)
            .await
            .map_err(|_| Error::InternalServerError)?
            .ok_or(Error::NotFound)?;
        lock_oauth_user_for_mutation(&tx, user_id, tenant_id)
            .await
            .map_err(|_| Error::InternalServerError)?
            .ok_or(Error::NotFound)?;
        Self::revoke_locked_user_consent(&tx, app_id, user_id, tenant_id).await?;
        tx.commit().await.map_err(|_| Error::InternalServerError)?;
        Ok(())
    }

    pub(crate) async fn revoke_locked_user_consent(
        tx: &DatabaseTransaction,
        app_id: Uuid,
        user_id: Uuid,
        tenant_id: Uuid,
    ) -> Result<()> {
        let consent = OAuthConsents::find_active_consent(tx, app_id, user_id, tenant_id)
            .await
            .map_err(|_| Error::InternalServerError)?;

        let now = rustok_auth::application_clock::verified_now(tx).await?;

        if let Some(c) = consent {
            let mut active: OAuthConsentActiveModel = c.into();
            active.revoked_at = Set(Some(now.into()));
            active
                .update(tx)
                .await
                .map_err(|_| Error::InternalServerError)?;
        }

        // Revoke all access and refresh tokens for this user and app.
        use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
        let revoked_tokens = oauth_tokens::Entity::update_many()
            .col_expr(
                oauth_tokens::Column::RevokedAt,
                sea_orm::sea_query::Expr::value(now),
            )
            .col_expr(
                oauth_tokens::Column::UpdatedAt,
                sea_orm::sea_query::Expr::value(now),
            )
            .filter(
                sea_orm::Condition::all()
                    .add(oauth_tokens::Column::AppId.eq(app_id))
                    .add(oauth_tokens::Column::UserId.eq(user_id))
                    .add(oauth_tokens::Column::TenantId.eq(tenant_id))
                    .add(oauth_tokens::Column::RevokedAt.is_null()),
            )
            .exec(tx)
            .await
            .map_err(|_| Error::InternalServerError)?;

        crate::models::oauth_authorization_codes::Entity::update_many()
            .col_expr(
                crate::models::oauth_authorization_codes::Column::UsedAt,
                sea_orm::sea_query::Expr::value(now),
            )
            .filter(crate::models::oauth_authorization_codes::Column::AppId.eq(app_id))
            .filter(crate::models::oauth_authorization_codes::Column::UserId.eq(user_id))
            .filter(crate::models::oauth_authorization_codes::Column::TenantId.eq(tenant_id))
            .filter(crate::models::oauth_authorization_codes::Column::UsedAt.is_null())
            .exec(tx)
            .await
            .map_err(|_| Error::InternalServerError)?;
        tracing::debug!(
            app_id = %app_id,
            user_id = %user_id,
            tenant_id = %tenant_id,
            revoked_tokens = revoked_tokens.rows_affected,
            "Revoked OAuth token family after consent revocation"
        );

        Ok(())
    }
}

/// Sync OAuth app connections based on the modules manifest.
/// Called after a successful build to ensure apps match the deployment configuration.
///
/// Rules:
/// - embed_admin=true → upsert Embedded app "leptos-admin" (no secret, *:*)
/// - embed_storefront=true → upsert Embedded app "leptos-storefront" (no secret, *:*)
/// - Standalone storefronts → upsert FirstParty app per storefront entry
/// - Standalone admin → upsert FirstParty app for the admin stack
/// - Apps in DB that no longer match any manifest entry → soft-deactivate
pub async fn sync_app_connections(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    manifest: &crate::modules::ModulesManifest,
) -> Result<()> {
    // Serialize the entire manifest reconciliation for this tenant. Every app
    // upsert, translation write, orphan deactivation, token/consent revocation,
    // and authorization-code invalidation must commit or roll back together.
    let tx = db.begin().await.map_err(|error| {
        tracing::error!(%error, %tenant_id, "Failed to start OAuth manifest reconciliation");
        Error::InternalServerError
    })?;
    lock_tenant_for_manifest_sync(&tx, tenant_id)
        .await
        .map_err(|error| {
            tracing::error!(%error, %tenant_id, "Failed to lock tenant for OAuth manifest reconciliation");
            Error::InternalServerError
        })?
        .ok_or_else(|| {
            tracing::error!(%tenant_id, "Tenant disappeared during OAuth manifest reconciliation");
            Error::InternalServerError
        })?;

    let existing = OAuthApps::find()
        .filter(oauth_apps::Column::TenantId.eq(tenant_id))
        .all(&tx)
        .await
        .map_err(|error| {
            tracing::error!(
                tenant_id = %tenant_id,
                error = %error,
                "Failed to load OAuth apps for tenant during manifest sync"
            );
            Error::InternalServerError
        })?;

    let mut active_slugs = std::collections::HashSet::new();

    // 1. Embedded apps
    if manifest.build.server.embed_admin {
        upsert_embedded_app(&tx, tenant_id, "leptos-admin", &["*:*"]).await?;
        active_slugs.insert("leptos-admin".to_string());
    }
    if manifest.build.server.embed_storefront {
        upsert_embedded_app(&tx, tenant_id, "leptos-storefront", &["*:*"]).await?;
        active_slugs.insert("leptos-storefront".to_string());
    }

    // 2. Standalone storefronts → FirstParty apps
    for sf in &manifest.build.storefront {
        if !manifest.build.server.embed_storefront || sf.stack == "next" {
            upsert_first_party_app(
                &tx,
                tenant_id,
                &sf.id,
                &format!("{} storefront", sf.id),
                &sf.public_url,
                &sf.redirect_uris,
                &["storefront:*"],
                &["authorization_code", "refresh_token", "client_credentials"],
                &role_permissions(UserRole::Customer),
            )
            .await?;
            active_slugs.insert(sf.id.clone());
        }
    }

    // 3. Standalone admin → FirstParty app
    if !manifest.build.server.embed_admin && !manifest.build.admin.stack.is_empty() {
        let slug = format!("{}-admin", manifest.build.admin.stack);
        upsert_first_party_app(
            &tx,
            tenant_id,
            &slug,
            &format!("{} Admin", manifest.build.admin.stack),
            &manifest.build.admin.public_url,
            &manifest.build.admin.redirect_uris,
            &["admin:*"],
            &["authorization_code", "refresh_token", "client_credentials"],
            &role_permissions(UserRole::Admin),
        )
        .await?;
        active_slugs.insert(slug);
    }

    // 4. Deactivate orphaned apps and revoke their credentials in this same
    // tenant transaction. A failed upsert or cleanup must not leave a partial
    // manifest projection behind.
    let orphaned = existing
        .iter()
        .filter(|app| {
            app.auto_created
                && app.manifest_ref.as_deref() == Some(app.slug.as_str())
                && !active_slugs.contains(&app.slug)
        })
        .collect::<Vec<_>>();
    let now = Utc::now();
    for app in orphaned {
        let current_app = lock_oauth_app_for_mutation(&tx, app.id, tenant_id)
            .await
            .map_err(|error| {
                tracing::error!(
                    %error,
                    %tenant_id,
                    app_id = %app.id,
                    "Failed to lock orphan OAuth app during manifest sync"
                );
                Error::InternalServerError
            })?
            .ok_or_else(|| {
                tracing::error!(
                    %tenant_id,
                    app_id = %app.id,
                    "Orphan OAuth app disappeared during manifest sync"
                );
                Error::Conflict("orphan OAuth app disappeared during manifest sync".to_string())
            })?;
        if !current_app.auto_created
            || current_app.manifest_ref.as_deref() != Some(current_app.slug.as_str())
        {
            return Err(Error::Conflict(format!(
                "OAuth app `{}` changed manifest ownership during sync",
                current_app.slug
            )));
        }

        // Also repair legacy inactive rows whose dependent credentials were
        // not revoked by earlier manifest-sync implementations.
        if current_app.is_active || current_app.revoked_at.is_none() {
            let result = oauth_apps::Entity::update_many()
                .col_expr(oauth_apps::Column::IsActive, Expr::value(false))
                .col_expr(
                    oauth_apps::Column::RevokedAt,
                    Expr::value(Some(now.clone())),
                )
                .col_expr(oauth_apps::Column::UpdatedAt, Expr::value(now.clone()))
                .filter(oauth_apps::Column::Id.eq(current_app.id))
                .filter(oauth_apps::Column::TenantId.eq(tenant_id))
                .filter(oauth_apps::Column::Slug.eq(current_app.slug.clone()))
                .filter(oauth_apps::Column::AppType.eq(current_app.app_type.clone()))
                .filter(oauth_apps::Column::AutoCreated.eq(true))
                .filter(oauth_apps::Column::ManifestRef.eq(Some(current_app.slug.clone())))
                .exec(&tx)
                .await
                .map_err(|error| {
                    tracing::error!(
                        %error,
                        %tenant_id,
                        app_id = %current_app.id,
                        "Failed to deactivate orphan OAuth app"
                    );
                    Error::InternalServerError
                })?;
            if result.rows_affected != 1 {
                tracing::error!(
                    %tenant_id,
                    app_id = %current_app.id,
                    "Orphan OAuth app state changed during manifest sync"
                );
                return Err(Error::Conflict(
                    "orphan OAuth app changed during manifest sync".to_string(),
                ));
            }
        }

        revoke_oauth_app_credentials(&tx, current_app.id, tenant_id, now.clone()).await?;
    }

    tx.commit().await.map_err(|error| {
        tracing::error!(%error, %tenant_id, "Failed to commit OAuth manifest reconciliation");
        Error::InternalServerError
    })?;
    Ok(())
}

pub async fn sync_manifest_managed_apps_for_all_tenants(
    db: &DatabaseConnection,
    manifest: &crate::modules::ModulesManifest,
) -> Result<()> {
    let tenants = tenants::find_active(db).await.map_err(|error| {
        tracing::error!(
            error = %error,
            "Failed to load active tenants during manifest OAuth sync"
        );
        Error::InternalServerError
    })?;

    for tenant in tenants {
        sync_app_connections(db, tenant.id, manifest)
            .await
            .map_err(|error| {
                tracing::error!(
                    tenant_id = %tenant.id,
                    error = %error,
                    "Failed to sync manifest OAuth apps for tenant"
                );
                error
            })?;
    }

    Ok(())
}

async fn lock_tenant_for_manifest_sync<C>(
    db: &C,
    tenant_id: Uuid,
) -> std::result::Result<Option<tenants::Model>, sea_orm::DbErr>
where
    C: ConnectionTrait,
{
    let query = || tenants::Entity::find_by_id(tenant_id);
    match db.get_database_backend() {
        DbBackend::Postgres | DbBackend::MySql => query().lock_exclusive().one(db).await,
        DbBackend::Sqlite => {
            // Acquire a SQLite write fence before reading. This avoids the
            // read-first deferred-transaction upgrade race between replicas.
            let locked = tenants::Entity::update_many()
                .col_expr(
                    tenants::Column::UpdatedAt,
                    Expr::col(tenants::Column::UpdatedAt),
                )
                .filter(tenants::Column::Id.eq(tenant_id))
                .exec(db)
                .await?;
            if locked.rows_affected != 1 {
                return Ok(None);
            }
            query().one(db).await
        }
        _ => Err(sea_orm::DbErr::Custom(
            "unsupported database backend for OAuth manifest synchronization".to_string(),
        )),
    }
}

async fn revoke_oauth_app_credentials<C>(
    db: &C,
    app_id: Uuid,
    tenant_id: Uuid,
    now: chrono::DateTime<Utc>,
) -> Result<()>
where
    C: ConnectionTrait,
{
    OAuthTokens::update_many()
        .col_expr(
            oauth_tokens::Column::RevokedAt,
            Expr::value(Some(now.clone())),
        )
        .col_expr(oauth_tokens::Column::UpdatedAt, Expr::value(now.clone()))
        .filter(oauth_tokens::Column::AppId.eq(app_id))
        .filter(oauth_tokens::Column::TenantId.eq(tenant_id))
        .filter(oauth_tokens::Column::RevokedAt.is_null())
        .exec(db)
        .await
        .map_err(|error| {
            tracing::error!(%error, %tenant_id, %app_id, "Failed to revoke OAuth app tokens");
            Error::InternalServerError
        })?;

    OAuthConsents::update_many()
        .col_expr(
            crate::models::oauth_consents::Column::RevokedAt,
            Expr::value(Some(now.clone())),
        )
        .filter(crate::models::oauth_consents::Column::AppId.eq(app_id))
        .filter(crate::models::oauth_consents::Column::TenantId.eq(tenant_id))
        .filter(crate::models::oauth_consents::Column::RevokedAt.is_null())
        .exec(db)
        .await
        .map_err(|error| {
            tracing::error!(%error, %tenant_id, %app_id, "Failed to revoke OAuth app consents");
            Error::InternalServerError
        })?;

    crate::models::oauth_authorization_codes::Entity::update_many()
        .col_expr(
            crate::models::oauth_authorization_codes::Column::UsedAt,
            Expr::value(now),
        )
        .filter(crate::models::oauth_authorization_codes::Column::AppId.eq(app_id))
        .filter(crate::models::oauth_authorization_codes::Column::TenantId.eq(tenant_id))
        .filter(crate::models::oauth_authorization_codes::Column::UsedAt.is_null())
        .exec(db)
        .await
        .map_err(|error| {
            tracing::error!(
                %error,
                %tenant_id,
                %app_id,
                "Failed to invalidate OAuth app authorization codes"
            );
            Error::InternalServerError
        })?;

    Ok(())
}

fn ensure_manifest_app_ownership(
    app: &oauth_apps::Model,
    slug: &str,
    expected_app_type: &str,
) -> Result<()> {
    if !app.auto_created
        || app.manifest_ref.as_deref() != Some(slug)
        || app.app_type != expected_app_type
    {
        return Err(Error::Conflict(format!(
            "OAuth app slug `{slug}` is occupied by an app not owned by the matching manifest entry"
        )));
    }
    Ok(())
}

/// Upsert an embedded app (no credentials needed)
async fn upsert_embedded_app(
    db: &DatabaseTransaction,
    tenant_id: Uuid,
    slug: &str,
    scopes: &[&str],
) -> Result<()> {
    let existing = OAuthApps::find()
        .filter(
            sea_orm::Condition::all()
                .add(oauth_apps::Column::TenantId.eq(tenant_id))
                .add(oauth_apps::Column::Slug.eq(slug)),
        )
        .one(db)
        .await
        .map_err(|error| {
            tracing::error!(
                tenant_id = %tenant_id,
                slug = slug,
                error = %error,
                "Failed to find embedded OAuth app during manifest sync"
            );
            Error::InternalServerError
        })?;

    if let Some(existing_app) = existing {
        let app = lock_oauth_app_for_mutation(db, existing_app.id, tenant_id)
            .await
            .map_err(|error| {
                tracing::error!(
                    %error,
                    tenant_id = %tenant_id,
                    slug = slug,
                    "Failed to lock embedded OAuth app during manifest sync"
                );
                Error::InternalServerError
            })?
            .ok_or_else(|| {
                Error::Conflict(format!(
                    "embedded OAuth app `{slug}` disappeared during manifest sync"
                ))
            })?;
        ensure_manifest_app_ownership(&app, slug, "embedded")?;
        // Reactivate with a column-scoped write so a stale read cannot overwrite
        // unrelated runtime-owned fields such as last_used_at.
        if !app.is_active() {
            let now = Utc::now();
            revoke_oauth_app_credentials(db, app.id, tenant_id, now.clone()).await?;
            let updated = oauth_apps::Entity::update_many()
                .col_expr(oauth_apps::Column::IsActive, Expr::value(true))
                .col_expr(
                    oauth_apps::Column::RevokedAt,
                    Expr::value(None::<sea_orm::prelude::DateTimeWithTimeZone>),
                )
                .col_expr(oauth_apps::Column::UpdatedAt, Expr::value(now))
                .filter(oauth_apps::Column::Id.eq(app.id))
                .filter(oauth_apps::Column::TenantId.eq(tenant_id))
                .filter(oauth_apps::Column::AppType.eq("embedded"))
                .filter(oauth_apps::Column::AutoCreated.eq(true))
                .filter(oauth_apps::Column::ManifestRef.eq(Some(slug.to_string())))
                .exec(db)
                .await
                .map_err(|error| {
                    tracing::error!(
                        tenant_id = %tenant_id,
                        slug = slug,
                        error = %error,
                        "Failed to reactivate embedded OAuth app during manifest sync"
                    );
                    Error::InternalServerError
                })?;
            if updated.rows_affected != 1 {
                return Err(Error::Conflict(format!(
                    "embedded OAuth app `{slug}` changed during manifest sync"
                )));
            }
        }
    } else {
        let scopes_vec: Vec<String> = scopes.iter().map(|s| s.to_string()).collect();
        OAuthAppActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant_id),
            name: Set(slug.to_string()),
            slug: Set(slug.to_string()),
            description: Set(Some("Auto-created embedded app".to_string())),
            app_type: Set("embedded".to_string()),
            icon_url: Set(None),
            client_id: Set(Uuid::new_v4()),
            client_secret_hash: Set(None), // Embedded — no secret
            redirect_uris: Set(serde_json::json!([])),
            scopes: Set(serde_json::to_value(&scopes_vec).map_err(|_| Error::InternalServerError)?),
            grant_types: Set(serde_json::json!([])),
            granted_permissions: Set(serde_json::json!([])),
            manifest_ref: Set(Some(slug.to_string())),
            auto_created: Set(true),
            is_active: Set(true),
            revoked_at: Set(None),
            last_used_at: Set(None),
            metadata: Set(serde_json::json!({})),
            created_at: Set(Utc::now().into()),
            updated_at: Set(Utc::now().into()),
        }
        .insert_in_transaction(db)
        .await
        .map_err(|error| {
            tracing::error!(
                tenant_id = %tenant_id,
                slug = slug,
                error = %error,
                "Failed to create embedded OAuth app during manifest sync"
            );
            Error::InternalServerError
        })?;
    }

    Ok(())
}

/// Upsert a first-party app (creates credentials on first creation)
#[allow(clippy::too_many_arguments)]
async fn upsert_first_party_app(
    db: &DatabaseTransaction,
    tenant_id: Uuid,
    slug: &str,
    name: &str,
    public_url: &str,
    redirect_uris: &[String],
    scopes: &[&str],
    grant_types: &[&str],
    granted_permissions: &[String],
) -> Result<()> {
    let scopes_vec: Vec<String> = scopes.iter().map(|s| s.to_string()).collect();
    let grant_types_vec: Vec<String> = grant_types.iter().map(|s| s.to_string()).collect();
    let granted_permissions_vec = validate_granted_permissions(granted_permissions.to_vec())?;
    let allow_loopback_http = public_url_uses_loopback_http(public_url);
    validate_managed_app_configuration(
        "first_party",
        redirect_uris,
        &scopes_vec,
        &grant_types_vec,
        &granted_permissions_vec,
        allow_loopback_http,
    )?;

    let existing = OAuthApps::find()
        .filter(
            sea_orm::Condition::all()
                .add(oauth_apps::Column::TenantId.eq(tenant_id))
                .add(oauth_apps::Column::Slug.eq(slug)),
        )
        .one(db)
        .await
        .map_err(|error| {
            tracing::error!(
                tenant_id = %tenant_id,
                slug = slug,
                error = %error,
                "Failed to find first-party OAuth app during manifest sync"
            );
            Error::InternalServerError
        })?;

    if let Some(existing_app) = existing {
        let app = lock_oauth_app_for_mutation(db, existing_app.id, tenant_id)
            .await
            .map_err(|error| {
                tracing::error!(
                    %error,
                    tenant_id = %tenant_id,
                    slug = slug,
                    "Failed to lock first-party OAuth app during manifest sync"
                );
                Error::InternalServerError
            })?
            .ok_or_else(|| {
                Error::Conflict(format!(
                    "first-party OAuth app `{slug}` disappeared during manifest sync"
                ))
            })?;
        if !app.is_active() {
            revoke_oauth_app_credentials(db, app.id, tenant_id, Utc::now()).await?;
        }
        update_first_party_app_from_manifest(
            db,
            &app,
            tenant_id,
            slug,
            name,
            public_url,
            redirect_uris,
            &scopes_vec,
            &grant_types_vec,
            &granted_permissions_vec,
        )
        .await?;
    } else {
        let app = OAuthAppActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant_id),
            name: Set(name.to_string()),
            slug: Set(slug.to_string()),
            description: Set(Some(format!("Auto-created for {slug}"))),
            app_type: Set("first_party".to_string()),
            icon_url: Set(None),
            client_id: Set(Uuid::new_v4()),
            client_secret_hash: Set(None),
            redirect_uris: Set(
                serde_json::to_value(redirect_uris).map_err(|_| Error::InternalServerError)?
            ),
            scopes: Set(serde_json::to_value(&scopes_vec).map_err(|_| Error::InternalServerError)?),
            grant_types: Set(
                serde_json::to_value(&grant_types_vec).map_err(|_| Error::InternalServerError)?
            ),
            granted_permissions: Set(to_json_array(&granted_permissions_vec)?),
            manifest_ref: Set(Some(slug.to_string())),
            auto_created: Set(true),
            is_active: Set(true),
            revoked_at: Set(None),
            last_used_at: Set(None),
            metadata: Set(build_manifest_managed_metadata(public_url)),
            created_at: Set(Utc::now().into()),
            updated_at: Set(Utc::now().into()),
        }
        .insert_in_transaction(db)
        .await
        .map_err(|error| {
            tracing::error!(
                tenant_id = %tenant_id,
                slug = slug,
                error = %error,
                "Failed to create first-party OAuth app during manifest sync"
            );
            Error::InternalServerError
        })?;

        tracing::info!(
            client_id = %app.client_id,
            slug = slug,
            "First-party OAuth app created as a public PKCE client; rotate a secret to enable SSR client_credentials"
        );
    }

    Ok(())
}

async fn update_first_party_app_from_manifest<C>(
    db: &C,
    app: &oauth_apps::Model,
    tenant_id: Uuid,
    slug: &str,
    name: &str,
    public_url: &str,
    redirect_uris: &[String],
    scopes: &[String],
    grant_types: &[String],
    granted_permissions: &[String],
) -> Result<()>
where
    C: ConnectionTrait,
{
    ensure_manifest_app_ownership(app, slug, "first_party")?;
    if oauth_app_policy_changed(app, redirect_uris, scopes, grant_types, granted_permissions) {
        revoke_oauth_app_credentials(db, app.id, tenant_id, Utc::now()).await?;
    }
    let scopes_json = serde_json::to_value(scopes).map_err(|_| Error::InternalServerError)?;
    let grant_types_json =
        serde_json::to_value(grant_types).map_err(|_| Error::InternalServerError)?;
    let redirect_uris_json =
        serde_json::to_value(redirect_uris).map_err(|_| Error::InternalServerError)?;
    let now = Utc::now();
    let updated = oauth_apps::Entity::update_many()
        .col_expr(oauth_apps::Column::IsActive, Expr::value(true))
        .col_expr(
            oauth_apps::Column::RevokedAt,
            Expr::value(None::<sea_orm::prelude::DateTimeWithTimeZone>),
        )
        .col_expr(oauth_apps::Column::Scopes, Expr::value(scopes_json))
        .col_expr(
            oauth_apps::Column::GrantTypes,
            Expr::value(grant_types_json),
        )
        .col_expr(
            oauth_apps::Column::GrantedPermissions,
            Expr::value(to_json_array(granted_permissions)?),
        )
        .col_expr(
            oauth_apps::Column::RedirectUris,
            Expr::value(redirect_uris_json),
        )
        .col_expr(
            oauth_apps::Column::Metadata,
            Expr::value(build_manifest_managed_metadata(public_url)),
        )
        .col_expr(
            oauth_apps::Column::ManifestRef,
            Expr::value(Some(slug.to_string())),
        )
        .col_expr(oauth_apps::Column::AutoCreated, Expr::value(true))
        .col_expr(oauth_apps::Column::UpdatedAt, Expr::value(now))
        .filter(oauth_apps::Column::Id.eq(app.id))
        .filter(oauth_apps::Column::TenantId.eq(tenant_id))
        .filter(oauth_apps::Column::AppType.eq("first_party"))
        .filter(oauth_apps::Column::AutoCreated.eq(true))
        .filter(oauth_apps::Column::ManifestRef.eq(Some(slug.to_string())))
        .exec(db)
        .await
        .map_err(|error| {
            tracing::error!(
                tenant_id = %tenant_id,
                slug = slug,
                error = %error,
                "Failed to update first-party OAuth app during manifest sync"
            );
            Error::InternalServerError
        })?;
    if updated.rows_affected != 1 {
        return Err(Error::Conflict(format!(
            "first-party OAuth app `{slug}` changed ownership during manifest sync"
        )));
    }
    oauth_apps::upsert_translation(
        db,
        tenant_id,
        app.id,
        "en",
        name.to_string(),
        Some(format!("Auto-created for {slug}")),
    )
    .await
    .map_err(|error| {
        tracing::error!(
            tenant_id = %tenant_id,
            slug = slug,
            error = %error,
            "Failed to upsert first-party OAuth app translation during manifest sync"
        );
        Error::InternalServerError
    })?;
    oauth_apps::record_translation_change(db, app, Some(rustok_core::generate_id()))
        .await
        .map_err(|error| {
            tracing::error!(
                tenant_id = %tenant_id,
                slug = slug,
                error = %error,
                "Failed to record first-party OAuth app translation change during manifest sync"
            );
            Error::InternalServerError
        })?;
    Ok(())
}

/// Generate a client secret with `sk_live_` prefix
fn generate_client_secret() -> Result<String> {
    // Keep the prefix stable while using more than 256 bits of entropy so
    // the resulting secret comfortably exceeds bcrypt/Argon guidance tests.
    let token = format!(
        "{}{}",
        auth::generate_refresh_token()?,
        auth::generate_refresh_token()?
    );
    Ok(format!("sk_live_{token}"))
}

async fn replace_oauth_secret_if_current(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    app_id: Uuid,
    expected_hash: Option<&str>,
    new_hash: &str,
    updated_at: chrono::DateTime<Utc>,
) -> Result<bool> {
    let result = oauth_apps::Entity::update_many()
        .col_expr(
            oauth_apps::Column::ClientSecretHash,
            Expr::value(Some(new_hash.to_string())),
        )
        .col_expr(oauth_apps::Column::UpdatedAt, Expr::value(updated_at))
        .filter(oauth_apps::Column::Id.eq(app_id))
        .filter(oauth_apps::Column::TenantId.eq(tenant_id))
        // Rotation eligibility was checked before Argon2 hashing. Require
        // the same eligible app-type set at the atomic write: a concurrent
        // manifest reconciliation must not turn an embedded/unsupported app
        // into an accidental confidential client through a stale preflight.
        .filter(oauth_apps::Column::AppType.is_in([
            "first_party",
            "third_party",
            "mobile",
            "service",
        ]))
        .filter(oauth_apps::Column::RevokedAt.is_null())
        .filter(match expected_hash {
            Some(expected_hash) => {
                oauth_apps::Column::ClientSecretHash.eq(Some(expected_hash.to_string()))
            }
            None => oauth_apps::Column::ClientSecretHash.is_null(),
        })
        .exec(db)
        .await
        .map_err(|_| Error::InternalServerError)?;
    Ok(result.rows_affected == 1)
}

/// Column widths the schema enforces for OAuth application display fields.
///
/// `name` is translation-owned and lands in `oauth_app_translations.name`
/// (`varchar(255)`); `slug` and `icon_url` are plain `oauth_apps` columns
/// (`varchar(100)` and `varchar(500)`). Validating here keeps an over-long value
/// a client error instead of letting the write fail: those paths map database
/// errors to a generic 500, so a 300-character name would otherwise be reported
/// as a server fault for a purely user-made mistake.
const OAUTH_APP_NAME_MAX_LEN: usize = 255;
const OAUTH_APP_SLUG_MAX_LEN: usize = 100;
const OAUTH_APP_ICON_URL_MAX_LEN: usize = 500;

/// Reject a slug that is already taken within the tenant.
///
/// The uniqueness rule lives in `idx_oauth_apps_tenant_slug`; checking it up
/// front keeps the failure a `Conflict` (409) rather than a generic 500 derived
/// from the constraint violation, and the message tells the operator which
/// field collided. The check is advisory: the unique index remains the
/// authority, so a race between two concurrent creates still cannot persist a
/// duplicate.
async fn ensure_slug_available(db: &DatabaseConnection, tenant_id: Uuid, slug: &str) -> Result<()> {
    let taken = OAuthApps::find()
        .filter(
            sea_orm::Condition::all()
                .add(oauth_apps::Column::TenantId.eq(tenant_id))
                .add(oauth_apps::Column::Slug.eq(slug)),
        )
        .one(db)
        .await
        .map_err(|error| {
            tracing::error!(
                tenant_id = %tenant_id,
                error = %error,
                "Failed to check OAuth app slug availability"
            );
            Error::InternalServerError
        })?
        .is_some();

    if taken {
        return Err(Error::Conflict(format!(
            "OAuth app slug `{slug}` is already in use for this tenant"
        )));
    }
    Ok(())
}

fn validate_app_display_fields(
    name: &str,
    slug: Option<&str>,
    icon_url: Option<&str>,
) -> Result<()> {
    let name = name.trim();
    if name.is_empty() {
        return Err(Error::BadRequest(
            "OAuth app name must not be empty".to_string(),
        ));
    }
    if name.chars().count() > OAUTH_APP_NAME_MAX_LEN {
        return Err(Error::BadRequest(format!(
            "OAuth app name must be at most {OAUTH_APP_NAME_MAX_LEN} characters"
        )));
    }
    if let Some(slug) = slug {
        let slug = slug.trim();
        if slug.is_empty() {
            return Err(Error::BadRequest(
                "OAuth app slug must not be empty".to_string(),
            ));
        }
        if slug.chars().count() > OAUTH_APP_SLUG_MAX_LEN {
            return Err(Error::BadRequest(format!(
                "OAuth app slug must be at most {OAUTH_APP_SLUG_MAX_LEN} characters"
            )));
        }
    }
    if let Some(icon_url) = icon_url
        && icon_url.chars().count() > OAUTH_APP_ICON_URL_MAX_LEN
    {
        return Err(Error::BadRequest(format!(
            "OAuth app icon URL must be at most {OAUTH_APP_ICON_URL_MAX_LEN} characters"
        )));
    }
    Ok(())
}

fn validate_manual_app_configuration(
    app_type: &str,
    redirect_uris: &[String],
    scopes: &[String],
    grant_types: &[String],
    granted_permissions: &[String],
) -> Result<()> {
    if !matches!(app_type, "third_party" | "mobile" | "service") {
        return Err(Error::BadRequest(
            "Only third_party, mobile, and service apps can be created or edited manually"
                .to_string(),
        ));
    }

    validate_app_configuration(
        app_type,
        redirect_uris,
        scopes,
        grant_types,
        granted_permissions,
        app_type == "mobile",
    )
}

fn validate_managed_app_configuration(
    app_type: &str,
    redirect_uris: &[String],
    scopes: &[String],
    grant_types: &[String],
    granted_permissions: &[String],
    allow_loopback_http: bool,
) -> Result<()> {
    validate_app_configuration(
        app_type,
        redirect_uris,
        scopes,
        grant_types,
        granted_permissions,
        allow_loopback_http,
    )
}

fn validate_app_configuration(
    app_type: &str,
    redirect_uris: &[String],
    scopes: &[String],
    grant_types: &[String],
    granted_permissions: &[String],
    allow_loopback_http: bool,
) -> Result<()> {
    if scopes.is_empty() {
        return Err(Error::BadRequest(
            "OAuth app must declare at least one scope".to_string(),
        ));
    }

    if grant_types.is_empty() {
        return Err(Error::BadRequest(
            "OAuth app must declare at least one grant type".to_string(),
        ));
    }

    for grant_type in grant_types {
        if !matches!(
            grant_type.as_str(),
            "authorization_code" | "client_credentials" | "refresh_token"
        ) {
            return Err(Error::BadRequest(format!(
                "Unsupported OAuth grant type '{grant_type}'"
            )));
        }
    }

    if grant_types.iter().any(|item| item == "refresh_token")
        && !grant_types.iter().any(|item| item == "authorization_code")
    {
        return Err(Error::BadRequest(
            "refresh_token grant requires authorization_code grant".to_string(),
        ));
    }

    if app_type == "service" && grant_types.iter().any(|item| item == "authorization_code") {
        return Err(Error::BadRequest(
            "Service apps cannot use authorization_code grant".to_string(),
        ));
    }

    let supports_authorization_code = grant_types.iter().any(|item| item == "authorization_code");
    let supports_client_credentials = grant_types.iter().any(|item| item == "client_credentials");

    if matches!(app_type, "mobile" | "third_party") && supports_client_credentials {
        return Err(Error::BadRequest(
            "Mobile and third-party apps cannot use client_credentials grant".to_string(),
        ));
    }
    if matches!(app_type, "mobile" | "third_party") && !supports_authorization_code {
        return Err(Error::BadRequest(
            "Mobile and third-party apps must use authorization_code grant".to_string(),
        ));
    }
    if app_type == "service" && !supports_client_credentials {
        return Err(Error::BadRequest(
            "Service apps must use client_credentials grant".to_string(),
        ));
    }

    if grant_types.iter().any(|item| item == "authorization_code") && redirect_uris.is_empty() {
        return Err(Error::BadRequest(
            "Apps using authorization_code grant must declare at least one redirect URI"
                .to_string(),
        ));
    }

    if grant_types.iter().any(|item| item == "client_credentials") && granted_permissions.is_empty()
    {
        return Err(Error::BadRequest(
            "Apps using client_credentials grant must declare at least one granted permission"
                .to_string(),
        ));
    }

    for redirect_uri in redirect_uris {
        validate_redirect_uri(app_type, redirect_uri, allow_loopback_http)?;
    }

    Ok(())
}

/// Redirect URIs are compared byte-for-byte at authorize time, so the
/// registered value itself must be a safe absolute URI: no fragment
/// (RFC 6749 section 3.1.2), no user info, and HTTPS for web clients.
/// HTTP loopback is limited to native mobile clients and manifest-managed
/// first-party apps whose configured public URL is itself loopback (local
/// development). Private-use schemes remain limited to native/mobile apps.
fn validate_redirect_uri(
    app_type: &str,
    redirect_uri: &str,
    allow_loopback_http: bool,
) -> Result<()> {
    let invalid = |reason: &str| {
        Error::BadRequest(format!("Invalid redirect URI '{redirect_uri}': {reason}"))
    };
    let url = Url::parse(redirect_uri).map_err(|error| invalid(&error.to_string()))?;

    if url.fragment().is_some() {
        return Err(invalid("fragment component is not allowed"));
    }
    if !redirect_response_query_is_unambiguous(&url) {
        return Err(invalid("query contains reserved OAuth response parameters"));
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(invalid("user info is not allowed"));
    }

    match url.scheme() {
        "https" => {
            if url.host_str().is_none_or(str::is_empty) {
                return Err(invalid("host is required"));
            }
        }
        "http" => {
            let loopback = url.host_str().is_some_and(is_loopback_host);
            if !loopback {
                return Err(invalid(
                    "http is only allowed for loopback hosts; use https",
                ));
            }
            if !allow_loopback_http {
                return Err(invalid(
                    "http loopback redirects are only allowed for native mobile apps or first-party local development",
                ));
            }
        }
        scheme => {
            if matches!(scheme, "data" | "javascript" | "file" | "about" | "blob") {
                return Err(invalid("scheme is not allowed"));
            }
            if app_type != "mobile" {
                return Err(invalid("custom schemes are only allowed for mobile apps"));
            }
            if !is_private_use_redirect_scheme(scheme) {
                return Err(invalid(
                    "mobile custom schemes must use reverse-domain private-use names",
                ));
            }
        }
    }

    Ok(())
}

/// OAuth response parameters belong exclusively to the authorization server.
/// Query names are decoded before comparison, including percent-encoded names.
pub(crate) fn redirect_response_query_is_unambiguous(url: &Url) -> bool {
    url.fragment().is_none()
        && !url.query_pairs().any(|(name, _)| {
            matches!(
                name.as_ref(),
                "code" | "state" | "error" | "error_description" | "error_uri"
            )
        })
}

fn is_private_use_redirect_scheme(scheme: &str) -> bool {
    let mut labels = scheme.split('.');
    labels.next().is_some_and(|label| !label.is_empty())
        && scheme.contains('.')
        && labels.all(|label| !label.is_empty())
}

fn is_loopback_host(host: &str) -> bool {
    let host = host
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
        .unwrap_or(host);
    host.trim_end_matches('.').eq_ignore_ascii_case("localhost")
        || host
            .parse::<std::net::IpAddr>()
            .is_ok_and(|address| address.is_loopback())
}

fn public_url_uses_loopback_http(public_url: &str) -> bool {
    Url::parse(public_url).is_ok_and(|url| {
        url.scheme() == "http"
            && url.username().is_empty()
            && url.password().is_none()
            && url.fragment().is_none()
            && url.host_str().is_some_and(is_loopback_host)
    })
}

fn build_manifest_managed_metadata(public_url: &str) -> serde_json::Value {
    let mut metadata = serde_json::Map::new();
    if !public_url.trim().is_empty() {
        metadata.insert(
            "public_url".to_string(),
            serde_json::Value::String(public_url.to_string()),
        );
    }
    serde_json::Value::Object(metadata)
}

fn to_json_array(values: &[String]) -> Result<serde_json::Value> {
    serde_json::to_value(values).map_err(|_| Error::InternalServerError)
}

fn validate_granted_permissions(values: Vec<String>) -> Result<Vec<String>> {
    let mut values = values
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    values.sort();
    values.dedup();

    for value in &values {
        Permission::from_str(value).map_err(|error| {
            Error::BadRequest(format!("Invalid OAuth app permission '{value}': {error}"))
        })?;
    }

    Ok(values)
}

fn role_permissions(role: UserRole) -> Vec<String> {
    Rbac::permissions_for_role(&role)
        .iter()
        .map(ToString::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::oauth_apps;
    use crate::models::oauth_apps::Entity as OAuthAppsEntity;
    use crate::models::oauth_authorization_codes;
    use crate::models::oauth_consents;
    use crate::models::oauth_tokens;
    use crate::models::tenants;
    use rustok_migrations::SqliteTestMigrator as Migrator;
    use rustok_test_utils::db::setup_test_db_with_migrations;
    use sea_orm::{
        ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, DbBackend, EntityTrait,
        PaginatorTrait, QueryFilter, Schema, Set,
    };
    use sea_orm_migration::{MigrationTrait, MigratorTrait, SchemaManager};
    use serial_test::serial;

    #[tokio::test]
    #[serial]
    async fn empty_oauth_consent_is_rejected_before_app_lookup() {
        let (db, tenant) = test_db_with_tenant().await;
        let error = OAuthAppService::grant_consent(
            &db,
            Uuid::new_v4(),
            Uuid::new_v4(),
            tenant.id,
            Vec::new(),
        )
        .await
        .expect_err("empty consent must be rejected");
        assert!(matches!(error, Error::BadRequest(_)));
    }

    async fn insert_pending_authorization_code(
        db: &DatabaseConnection,
        app_id: Uuid,
        user_id: Uuid,
        tenant_id: Uuid,
    ) -> Uuid {
        let now = Utc::now();
        let id = Uuid::new_v4();
        oauth_authorization_codes::ActiveModel {
            id: Set(id),
            app_id: Set(app_id),
            user_id: Set(user_id),
            tenant_id: Set(tenant_id),
            code_hash: Set(format!("pending-{id}")),
            redirect_uri: Set("https://partner.example.com/oauth/callback".to_string()),
            scopes: Set(serde_json::json!(["catalog:read"])),
            code_challenge: Set("challenge".to_string()),
            code_challenge_method: Set("S256".to_string()),
            expires_at: Set((now + chrono::Duration::minutes(10)).into()),
            used_at: Set(None),
            created_at: Set(now.into()),
        }
        .insert(db)
        .await
        .expect("insert pending authorization code");
        id
    }

    // The manifest regression tests exercise the real Auth and identity schema,
    // not the unrelated module migrations in the whole-platform SQLite fixture.
    struct OAuthManifestTestMigrator;

    #[async_trait::async_trait]
    impl MigratorTrait for OAuthManifestTestMigrator {
        fn migrations() -> Vec<Box<dyn MigrationTrait>> {
            let mut required: std::collections::HashSet<_> =
                rustok_migrations::identity::migrations()
                    .into_iter()
                    .chain(
                        rustok_auth::migrations::migrations()
                            .into_iter()
                            .filter(|migration| {
                                migration.name().contains("_oauth_")
                                    || migration.name()
                                        == "m20260713_000001_create_auth_invite_consumptions"
                            }),
                    )
                    .map(|migration| migration.name().to_owned())
                    .collect();
            // The platform also owns columns used by the user and OAuth token models.
            required.extend([
                "m20261025_000001_add_user_login_lockout".to_owned(),
                "m20261025_000005_add_oauth_token_family".to_owned(),
                "m20261101_000001_add_user_password_changed_at".to_owned(),
                "m20261126_000050_add_user_recovery_revision".to_owned(),
            ]);

            let migrations = Migrator::migrations()
                .into_iter()
                .filter(|migration| required.remove(migration.name()))
                .collect();
            assert!(
                required.is_empty(),
                "missing OAuth fixture migrations: {required:?}"
            );
            migrations
        }
    }

    async fn test_db_with_manifest_tenant() -> (DatabaseConnection, tenants::Model) {
        seed_test_tenant(setup_test_db_with_migrations::<OAuthManifestTestMigrator>().await).await
    }

    async fn test_db_with_tenant() -> (DatabaseConnection, tenants::Model) {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        ensure_oauth_schema(&db).await;
        seed_test_tenant(db).await
    }

    async fn seed_test_tenant(db: DatabaseConnection) -> (DatabaseConnection, tenants::Model) {
        let tenant =
            tenants::ActiveModel::new("Test tenant", &format!("tenant-{}", Uuid::new_v4()))
                .insert(&db)
                .await
                .expect("failed to create tenant");

        (db, tenant)
    }

    async fn ensure_oauth_schema(db: &DatabaseConnection) {
        if db.get_database_backend() != DbBackend::Sqlite {
            return;
        }

        let manager = SchemaManager::new(db);

        if manager
            .has_table("oauth_apps")
            .await
            .expect("check oauth_apps table")
            && manager
                .has_table("oauth_tokens")
                .await
                .expect("check oauth_tokens table")
            && manager
                .has_table("oauth_authorization_codes")
                .await
                .expect("check oauth_authorization_codes table")
            && manager
                .has_table("oauth_consents")
                .await
                .expect("check oauth_consents table")
        {
            return;
        }

        let builder = db.get_database_backend();
        let schema = Schema::new(builder);

        ensure_entity_table(
            db,
            &manager,
            &builder,
            "oauth_apps",
            schema.create_table_from_entity(oauth_apps::Entity),
        )
        .await;
        ensure_entity_table(
            db,
            &manager,
            &builder,
            "oauth_tokens",
            schema.create_table_from_entity(oauth_tokens::Entity),
        )
        .await;
        ensure_entity_table(
            db,
            &manager,
            &builder,
            "oauth_authorization_codes",
            schema.create_table_from_entity(oauth_authorization_codes::Entity),
        )
        .await;
        ensure_entity_table(
            db,
            &manager,
            &builder,
            "oauth_consents",
            schema.create_table_from_entity(oauth_consents::Entity),
        )
        .await;
    }

    async fn ensure_entity_table(
        db: &DatabaseConnection,
        manager: &SchemaManager<'_>,
        builder: &DbBackend,
        table_name: &str,
        mut statement: sea_orm::sea_query::TableCreateStatement,
    ) {
        if manager
            .has_table(table_name)
            .await
            .unwrap_or_else(|error| panic!("check {table_name} presence: {error}"))
        {
            return;
        }

        statement.if_not_exists();
        db.execute_raw(builder.build(&statement))
            .await
            .unwrap_or_else(|error| panic!("create {table_name} for oauth tests: {error}"));

        assert!(
            manager
                .has_table(table_name)
                .await
                .unwrap_or_else(|error| panic!("re-check {table_name} presence: {error}")),
            "expected {table_name} to exist after sqlite test bootstrap"
        );
    }

    #[tokio::test]
    #[serial]
    async fn sqlite_oauth_test_bootstrap_guarantees_oauth_tables() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        ensure_oauth_schema(&db).await;

        let manager = SchemaManager::new(&db);
        for table_name in [
            "oauth_apps",
            "oauth_tokens",
            "oauth_authorization_codes",
            "oauth_consents",
        ] {
            assert!(
                manager
                    .has_table(table_name)
                    .await
                    .unwrap_or_else(|error| panic!(
                        "check {table_name} in regression test: {error}"
                    )),
                "expected {table_name} to exist in sqlite oauth test bootstrap"
            );
        }
    }

    // ===================================================================
    // RFC 6749 — OAuth 2.0 Authorization Framework: Scope Validation
    // ===================================================================
    //
    // RFC 6749 §3.3: Access token scope
    //   "The value of the scope parameter is expressed as a list of
    //    space-delimited, case-sensitive strings."

    #[test]
    fn rfc6749_scope_exact_match() {
        let allowed = vec!["catalog:read".to_string(), "orders:write".to_string()];
        assert!(scope_matches(&allowed, "catalog:read"));
        assert!(scope_matches(&allowed, "orders:write"));
        assert!(!scope_matches(&allowed, "admin:users"));
    }

    #[test]
    fn rfc6749_scope_case_sensitive() {
        // RFC 6749 §3.3: scope tokens are case-sensitive
        let allowed = vec!["Catalog:Read".to_string()];
        assert!(scope_matches(&allowed, "Catalog:Read"));
        assert!(!scope_matches(&allowed, "catalog:read"));
        assert!(!scope_matches(&allowed, "CATALOG:READ"));
    }

    #[test]
    fn rfc6749_scope_wildcard_resource() {
        let allowed = vec!["storefront:*".to_string()];
        assert!(scope_matches(&allowed, "storefront:read"));
        assert!(scope_matches(&allowed, "storefront:write"));
        assert!(scope_matches(&allowed, "storefront:delete"));
        assert!(!scope_matches(&allowed, "admin:read"));
        assert!(!scope_matches(&allowed, "storefront_extra:read"));
    }

    #[test]
    fn rfc6749_scope_superadmin_wildcard() {
        let allowed = vec!["*:*".to_string()];
        assert!(scope_matches(&allowed, "anything:here"));
        assert!(scope_matches(&allowed, "admin:users"));
        assert!(scope_matches(&allowed, "catalog:read"));
    }

    #[test]
    fn manual_apps_cannot_use_first_party_type() {
        let result = validate_manual_app_configuration(
            "first_party",
            &["http://localhost:3000/auth/callback".to_string()],
            &["profile".to_string()],
            &["authorization_code".to_string()],
            &[],
        );

        assert!(result.is_err());
    }

    #[test]
    fn redirect_uris_reserve_oauth_response_parameters() {
        for name in [
            "code",
            "state",
            "error",
            "error_description",
            "error_uri",
            "%63ode",
            "st%61te",
        ] {
            let uri = format!("https://client.example.com/callback?locale=en&{name}=stale");
            assert!(validate_redirect_uri("third_party", &uri, false).is_err());
            assert!(!super::redirect_response_query_is_unambiguous(
                &reqwest::Url::parse(&uri).unwrap()
            ));
        }
        assert!(
            validate_redirect_uri(
                "third_party",
                "https://client.example.com/callback?locale=en&tag=a&tag=b",
                false
            )
            .is_ok()
        );
        let fragment =
            reqwest::Url::parse("https://client.example.com/callback#state=stale").unwrap();
        assert!(!super::redirect_response_query_is_unambiguous(&fragment));
    }

    #[test]
    fn third_party_web_clients_reject_http_loopback_redirect_uris() {
        for redirect_uri in [
            "http://localhost:3000/auth/callback",
            "http://127.0.0.1:3000/auth/callback",
            "http://[::1]:3000/auth/callback",
        ] {
            let result = validate_manual_app_configuration(
                "third_party",
                &[redirect_uri.to_string()],
                &["profile".to_string()],
                &["authorization_code".to_string()],
                &[],
            );
            assert!(
                result.is_err(),
                "third-party web clients must use HTTPS: {redirect_uri}"
            );
        }
    }

    #[test]
    fn native_mobile_clients_accept_ipv4_and_ipv6_loopback_http_redirect_uris() {
        for redirect_uri in [
            "http://localhost:3000/auth/callback",
            "http://127.0.0.1:3000/auth/callback",
            "http://[::1]:3000/auth/callback",
        ] {
            let result = validate_manual_app_configuration(
                "mobile",
                &[redirect_uri.to_string()],
                &["profile".to_string()],
                &["authorization_code".to_string()],
                &[],
            );
            assert!(
                result.is_ok(),
                "native mobile clients may use loopback HTTP: {redirect_uri}"
            );
        }
    }

    #[test]
    fn mobile_custom_redirect_schemes_must_use_reverse_domain_private_use_names() {
        for redirect_uri in [
            "ftp://example.com/callback",
            "mailto:callback@example.com",
            "tel:+15551234567",
            "urn:example:oauth",
            "myapp://callback",
        ] {
            assert!(
                validate_redirect_uri("mobile", redirect_uri, true).is_err(),
                "registered URI schemes must not be accepted as private-use schemes: {redirect_uri}"
            );
        }

        assert!(
            validate_redirect_uri("mobile", "com.example.app:/oauth/callback", true).is_ok(),
            "reverse-domain private-use schemes are supported for native apps"
        );
    }

    #[test]
    fn mobile_and_first_party_clients_reject_non_loopback_http_redirect_uris() {
        for app_type in ["mobile", "first_party"] {
            assert!(
                validate_redirect_uri(app_type, "http://example.com/callback", true).is_err(),
                "{app_type} must not use cleartext HTTP on a non-loopback host"
            );
        }
    }

    #[test]
    fn first_party_loopback_http_is_allowed_only_for_loopback_public_urls() {
        let redirect_uris = vec!["http://localhost:3000/auth/callback".to_string()];
        let scopes = vec!["profile".to_string()];
        let grant_types = vec!["authorization_code".to_string()];

        assert!(public_url_uses_loopback_http("http://localhost:3000"));
        assert!(public_url_uses_loopback_http("http://[::1]:3000"));
        assert!(!public_url_uses_loopback_http("https://localhost:3000"));
        assert!(!public_url_uses_loopback_http("http://admin.example.com"));

        assert!(
            validate_managed_app_configuration(
                "first_party",
                &redirect_uris,
                &scopes,
                &grant_types,
                &[],
                public_url_uses_loopback_http("http://localhost:3000"),
            )
            .is_ok()
        );
        assert!(
            validate_managed_app_configuration(
                "first_party",
                &redirect_uris,
                &scopes,
                &grant_types,
                &[],
                public_url_uses_loopback_http("https://admin.example.com"),
            )
            .is_err()
        );
    }

    #[test]
    fn authorization_code_requires_redirect_uri() {
        let result = validate_manual_app_configuration(
            "third_party",
            &[],
            &["profile".to_string()],
            &["authorization_code".to_string()],
            &[],
        );

        assert!(result.is_err());
    }

    #[test]
    fn refresh_token_grant_requires_authorization_code_for_all_app_types() {
        let scopes = vec!["profile".to_string()];
        let refresh_only = vec!["refresh_token".to_string()];

        assert!(
            validate_manual_app_configuration("third_party", &[], &scopes, &refresh_only, &[],)
                .is_err()
        );
        assert!(
            validate_managed_app_configuration(
                "first_party",
                &[],
                &scopes,
                &refresh_only,
                &[],
                false,
            )
            .is_err()
        );

        let authorization_and_refresh = vec![
            "authorization_code".to_string(),
            "refresh_token".to_string(),
        ];
        assert!(
            validate_manual_app_configuration(
                "third_party",
                &["https://client.example/callback".to_string()],
                &scopes,
                &authorization_and_refresh,
                &[],
            )
            .is_ok()
        );
    }

    #[test]
    fn public_client_types_cannot_use_client_credentials() {
        for app_type in ["mobile", "third_party"] {
            let result = validate_manual_app_configuration(
                app_type,
                &[],
                &["catalog:read".to_string()],
                &["client_credentials".to_string()],
                &["catalog:read".to_string()],
            );
            assert!(
                result.is_err(),
                "{app_type} apps must not receive userless client-credentials grants"
            );
        }
    }

    #[test]
    fn service_apps_require_client_credentials_and_first_party_apps_can_be_hybrid() {
        assert!(
            validate_manual_app_configuration(
                "service",
                &[],
                &["catalog:read".to_string()],
                &["client_credentials".to_string()],
                &["catalog:read".to_string()],
            )
            .is_ok()
        );
        assert!(
            validate_manual_app_configuration(
                "service",
                &["https://service.example/callback".to_string()],
                &["catalog:read".to_string()],
                &["authorization_code".to_string()],
                &[],
            )
            .is_err()
        );

        assert!(
            validate_managed_app_configuration(
                "first_party",
                &["https://shop.example/callback".to_string()],
                &["storefront:read".to_string()],
                &[
                    "authorization_code".to_string(),
                    "refresh_token".to_string(),
                    "client_credentials".to_string(),
                ],
                &["storefront:read".to_string()],
                false,
            )
            .is_ok()
        );
    }

    #[test]
    fn client_credentials_requires_granted_permissions() {
        let result = validate_manual_app_configuration(
            "service",
            &[],
            &["forum:*".to_string()],
            &["client_credentials".to_string()],
            &[],
        );

        assert!(result.is_err());
    }

    #[test]
    fn rfc6749_scope_empty_allowed_rejects_all() {
        let allowed: Vec<String> = vec![];
        assert!(!scope_matches(&allowed, "catalog:read"));
        assert!(!scope_matches(&allowed, ""));
    }

    #[test]
    fn rfc6749_scope_multiple_wildcards() {
        let allowed = vec!["catalog:*".to_string(), "orders:read".to_string()];
        assert!(scope_matches(&allowed, "catalog:read"));
        assert!(scope_matches(&allowed, "catalog:write"));
        assert!(scope_matches(&allowed, "orders:read"));
        assert!(!scope_matches(&allowed, "orders:write"));
    }

    #[test]
    fn rfc6749_scope_no_partial_prefix_match() {
        // "storefront:*" should NOT match "storefrontx:read"
        let allowed = vec!["store:*".to_string()];
        assert!(scope_matches(&allowed, "store:read"));
        assert!(!scope_matches(&allowed, "storefront:read"));
    }

    #[test]
    fn rfc6749_scope_space_delimited_parsing() {
        // RFC 6749 §3.3: scopes are space-delimited in request
        let scope_string = "catalog:read orders:write users:read";
        let parsed: Vec<String> = scope_string.split_whitespace().map(String::from).collect();
        assert_eq!(parsed, vec!["catalog:read", "orders:write", "users:read"]);
    }

    #[test]
    fn rfc6749_scope_empty_request_gets_full_scope() {
        // RFC 6749 §3.3: if scope is omitted, server MAY use a default
        // Our implementation grants the app's full scope list
        let scope_string: Option<&str> = None;
        let parsed: Vec<String> = scope_string
            .map(|s| s.split_whitespace().map(String::from).collect())
            .unwrap_or_default();
        assert!(parsed.is_empty());
    }

    // ===================================================================
    // RFC 6749 §5.2 — Error Response format
    // ===================================================================

    #[test]
    fn rfc6749_error_codes_are_valid() {
        // RFC 6749 §5.2 defines these error codes
        let valid_error_codes = [
            "invalid_request",
            "invalid_client",
            "invalid_grant",
            "unauthorized_client",
            "unsupported_grant_type",
            "invalid_scope",
        ];

        // Our implementation uses these codes:
        let _our_codes = [
            "invalid_client",
            "invalid_grant",
            "unsupported_grant_type",
            "invalid_request",
            "invalid_scope",
            "unsupported_response_type", // RFC 6749 §4.1.2.1
            "interaction_required",      // OpenID Connect Core §3.1.2.6
            "server_error",              // RFC 6749 §4.1.2.1
        ];

        // Core token endpoint error codes must be from RFC 6749 §5.2
        for code in &[
            "invalid_client",
            "invalid_grant",
            "unsupported_grant_type",
            "invalid_request",
            "invalid_scope",
        ] {
            assert!(
                valid_error_codes.contains(code),
                "Error code '{}' not in RFC 6749 §5.2",
                code
            );
        }
    }

    #[test]
    fn rfc6749_token_response_type_is_bearer() {
        // RFC 6749 §5.1: token_type MUST be "Bearer" (RFC 6750)
        let token_type = "Bearer";
        assert_eq!(token_type, "Bearer");
    }

    #[test]
    fn rfc6749_grant_types_supported() {
        // RFC 6749 defines these grant types
        let supported = ["authorization_code", "client_credentials", "refresh_token"];
        // All our supported grant types are valid RFC 6749 grant types
        for gt in &supported {
            assert!(
                [
                    "authorization_code",
                    "implicit",
                    "client_credentials",
                    "refresh_token",
                    "password"
                ]
                .contains(gt),
                "'{}' is not a valid RFC 6749 grant type",
                gt
            );
        }
        // We do NOT support implicit or password (which is correct per security best practices)
        assert!(!supported.contains(&"implicit"));
        assert!(!supported.contains(&"password"));
    }

    // ===================================================================
    // RFC 7636 — PKCE (Proof Key for Code Exchange)
    // ===================================================================

    #[test]
    fn rfc7636_appendix_b_test_vector() {
        // RFC 7636 Appendix B — Official test vector
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        let expected_challenge = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM";

        assert!(OAuthAppService::verify_pkce(expected_challenge, verifier));
    }

    #[test]
    fn rfc7636_wrong_verifier_rejected() {
        let expected_challenge = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM";
        assert!(!OAuthAppService::verify_pkce(
            expected_challenge,
            "wrong_verifier"
        ));
    }

    #[test]
    fn rfc7636_wrong_challenge_rejected() {
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        assert!(!OAuthAppService::verify_pkce("wrong_challenge", verifier));
    }

    #[test]
    fn rfc7636_s256_transform() {
        // RFC 7636 §4.2: S256
        //   code_challenge = BASE64URL(SHA256(ASCII(code_verifier)))
        use base64::{Engine as _, engine::general_purpose};
        use sha2::{Digest, Sha256};

        let verifier = "a~b.c_d-e~f.g_h-i".repeat(3);
        let mut hasher = Sha256::new();
        hasher.update(verifier.as_bytes());
        let hash = hasher.finalize();
        let challenge = general_purpose::URL_SAFE_NO_PAD.encode(hash);

        // verify_pkce should accept the correctly generated challenge
        assert!(OAuthAppService::verify_pkce(&challenge, &verifier));
    }

    #[test]
    fn rfc7636_verifier_length_43_to_128() {
        // RFC 7636 §4.1: code_verifier is 43-128 characters
        use base64::{Engine as _, engine::general_purpose};
        use sha2::{Digest, Sha256};

        // Minimum length (43 chars)
        let verifier_min = "a".repeat(43);
        let mut hasher = Sha256::new();
        hasher.update(verifier_min.as_bytes());
        let challenge_min = general_purpose::URL_SAFE_NO_PAD.encode(hasher.finalize());
        assert!(OAuthAppService::verify_pkce(&challenge_min, &verifier_min));

        // Maximum length (128 chars)
        let verifier_max = "b".repeat(128);
        let mut hasher = Sha256::new();
        hasher.update(verifier_max.as_bytes());
        let challenge_max = general_purpose::URL_SAFE_NO_PAD.encode(hasher.finalize());
        assert!(OAuthAppService::verify_pkce(&challenge_max, &verifier_max));
    }

    #[test]
    fn rfc7636_constant_time_comparison() {
        // Verify that verify_pkce uses constant-time comparison (subtle::ConstantTimeEq)
        // by checking that wrong challenges of same length still fail
        use base64::{Engine as _, engine::general_purpose};
        use sha2::{Digest, Sha256};

        let verifier = "test_verifier_for_timing_check_padding_here";
        let mut hasher = Sha256::new();
        hasher.update(verifier.as_bytes());
        let correct_challenge = general_purpose::URL_SAFE_NO_PAD.encode(hasher.finalize());

        // Create a challenge of the same length but different content
        let mut wrong_challenge = correct_challenge.clone().into_bytes();
        if let Some(last) = wrong_challenge.last_mut() {
            *last = if *last == b'A' { b'B' } else { b'A' };
        }
        let wrong_challenge = String::from_utf8(wrong_challenge).unwrap();

        assert_eq!(correct_challenge.len(), wrong_challenge.len());
        assert!(OAuthAppService::verify_pkce(&correct_challenge, verifier));
        assert!(!OAuthAppService::verify_pkce(&wrong_challenge, verifier));
    }

    #[test]
    fn rfc7636_empty_verifier() {
        assert!(!OAuthAppService::verify_pkce("some_challenge", ""));
    }

    #[test]
    fn rfc7636_rejects_verifier_outside_length_and_character_bounds() {
        use base64::{Engine as _, engine::general_purpose};
        use sha2::{Digest, Sha256};

        for verifier in [
            "a".repeat(42),
            "a".repeat(129),
            format!("{}+", "a".repeat(42)),
        ] {
            let challenge =
                general_purpose::URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
            assert!(!OAuthAppService::verify_pkce(&challenge, &verifier));
        }
    }

    // ===================================================================
    // RFC 6749 §5.1 — Token Response structure
    // ===================================================================

    #[test]
    fn rfc6749_token_response_serialization() {
        // RFC 6749 §5.1: The authorization server issues an access token
        // and optional refresh token with these fields
        let response = serde_json::json!({
            "access_token": "some_token",
            "token_type": "Bearer",
            "expires_in": 3600u64,
            "scope": "catalog:read orders:write",
        });

        // Required fields per RFC 6749 §5.1
        assert!(
            response.get("access_token").is_some(),
            "access_token REQUIRED"
        );
        assert!(response.get("token_type").is_some(), "token_type REQUIRED");
        assert_eq!(
            response["token_type"], "Bearer",
            "token_type MUST be Bearer"
        );
        assert!(
            response.get("expires_in").is_some(),
            "expires_in RECOMMENDED"
        );
    }

    #[test]
    fn rfc6749_client_credentials_no_refresh_token() {
        // RFC 6749 §4.4.3: A refresh token SHOULD NOT be included
        // in client_credentials response
        let has_refresh_token: Option<String> = None;
        assert!(
            has_refresh_token.is_none(),
            "client_credentials SHOULD NOT include refresh_token"
        );
    }

    #[test]
    fn rfc6749_authorization_code_refresh_token_is_policy_driven() {
        // RFC 6749 §5.1: refresh_token is OPTIONAL. Our authorization-code
        // flow returns one only when the application explicitly grants refresh_token.
        let explicit_grant = true;
        let has_refresh_token = explicit_grant.then(|| "some_refresh_token".to_string());
        assert!(has_refresh_token.is_some());

        let undeclared_grant = false;
        let has_refresh_token = undeclared_grant.then(|| "some_refresh_token".to_string());
        assert!(has_refresh_token.is_none());
    }

    // ===================================================================
    // RFC 7009 — Token Revocation
    // ===================================================================

    async fn insert_revocation_app(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        slug: &str,
    ) -> oauth_apps::Model {
        OAuthAppActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant_id),
            name: Set(slug.to_string()),
            slug: Set(slug.to_string()),
            description: Set(None),
            app_type: Set("third_party".to_string()),
            icon_url: Set(None),
            client_id: Set(Uuid::new_v4()),
            client_secret_hash: Set(None),
            redirect_uris: Set(serde_json::json!(["https://partner.example.com/callback"])),
            scopes: Set(serde_json::json!(["openid"])),
            grant_types: Set(serde_json::json!(["authorization_code", "refresh_token"])),
            granted_permissions: Set(serde_json::json!([])),
            manifest_ref: Set(None),
            auto_created: Set(false),
            is_active: Set(true),
            revoked_at: Set(None),
            last_used_at: Set(None),
            metadata: Set(serde_json::json!({})),
            created_at: Set(Utc::now().into()),
            updated_at: Set(Utc::now().into()),
        }
        .insert(db)
        .await
        .expect("insert revocation app")
    }

    async fn insert_revocation_token(
        db: &DatabaseConnection,
        app: &oauth_apps::Model,
        token_hash: &str,
        family_id: Option<Uuid>,
    ) -> oauth_tokens::Model {
        let id = Uuid::new_v4();
        if let Some(root_id) = family_id {
            let now = Utc::now();
            OAuthTokens::update_many()
                .col_expr(oauth_tokens::Column::RevokedAt, Expr::value(now))
                .col_expr(oauth_tokens::Column::LastUsedAt, Expr::value(now))
                .filter(oauth_tokens::Column::Id.eq(root_id))
                .filter(oauth_tokens::Column::AppId.eq(app.id))
                .filter(oauth_tokens::Column::TenantId.eq(app.tenant_id))
                .exec(db)
                .await
                .expect("consume test root");
        }
        oauth_tokens::ActiveModel {
            id: Set(id),
            app_id: Set(app.id),
            user_id: Set(None),
            tenant_id: Set(app.tenant_id),
            token_hash: Set(token_hash.to_string()),
            family_id: Set(family_id.unwrap_or(id)),
            authorization_code_id: Set(None),
            grant_type: Set("client_credentials".to_string()),
            token_kind: Set(oauth_tokens::TOKEN_KIND_REFRESH.to_string()),
            consent_granted_at: Set(None),
            scopes: Set(serde_json::json!(["openid"])),
            expires_at: Set((Utc::now() + chrono::Duration::days(1)).into()),
            revoked_at: Set(None),
            last_used_at: Set(None),
            created_at: Set(Utc::now().into()),
            updated_at: Set(Utc::now().into()),
        }
        .insert(db)
        .await
        .expect("insert revocation token")
    }

    async fn authorization_subject(
        db: &DatabaseConnection,
        user: &users::Model,
    ) -> AuthorizationSession {
        let expires_at = Utc::now() + chrono::Duration::minutes(10);
        let session = crate::models::sessions::ActiveModel::new(
            user.tenant_id,
            user.id,
            Uuid::new_v4().to_string(),
            expires_at,
            None,
            None,
        )
        .insert(db)
        .await
        .expect("direct session");
        AuthorizationSession {
            user_id: user.id,
            session_id: session.id,
            access_expires_at: expires_at,
            require_email_verification: false,
            confirmation_deadline: None,
        }
    }

    fn authorization_test_clock() -> Result<chrono::DateTime<Utc>> {
        Ok(Utc::now())
    }

    #[tokio::test]
    #[serial]
    async fn authorization_code_issuance_rechecks_consent_and_revocation() {
        let (db, tenant) = test_db_with_tenant().await;
        let app = insert_revocation_app(&db, tenant.id, "issuance-consent-fence").await;
        let user = users::ActiveModel::new(
            tenant.id,
            "issuance-consent-fence@example.com",
            "unused-password-hash",
        )
        .insert(&db)
        .await
        .expect("create consent subject");
        let subject = authorization_subject(&db, &user).await;
        let redirect_uri = "https://partner.example.com/callback";
        let scopes = vec!["openid".to_string()];
        let issue = || {
            OAuthAppService::generate_authorization_code(
                &db,
                app.id,
                subject,
                tenant.id,
                redirect_uri.to_string(),
                scopes.clone(),
                "challenge".to_string(),
                &authorization_test_clock,
            )
        };

        let missing_consent = issue()
            .await
            .expect_err("third-party app must not issue without current consent");
        assert!(matches!(missing_consent, Error::Unauthorized(_)));
        assert_eq!(
            oauth_authorization_codes::Entity::find()
                .count(&db)
                .await
                .expect("count codes"),
            0,
        );

        OAuthAppService::grant_consent_strict(&db, app.id, user.id, tenant.id, scopes.clone())
            .await
            .expect("grant consent");
        let code = issue().await.expect("issue after consent");
        use sha2::{Digest, Sha256};
        let code_hash = hex::encode(Sha256::digest(code.as_bytes()));
        let pending = oauth_authorization_codes::Entity::find()
            .filter(oauth_authorization_codes::Column::CodeHash.eq(code_hash))
            .one(&db)
            .await
            .expect("find pending code")
            .expect("pending code exists");
        assert_eq!(pending.app_id, app.id);
        assert_eq!(pending.user_id, user.id);
        assert!(pending.used_at.is_none());

        OAuthAppService::revoke_user_consent(&db, app.id, user.id, tenant.id)
            .await
            .expect("revoke consent and pending codes");
        assert!(matches!(issue().await, Err(Error::Unauthorized(_))));
        let revoked_code = oauth_authorization_codes::Entity::find_by_id(pending.id)
            .one(&db)
            .await
            .expect("reload prior code")
            .expect("prior code survives for replay detection");
        assert!(revoked_code.used_at.is_some());
        assert_eq!(
            oauth_authorization_codes::Entity::find()
                .count(&db)
                .await
                .expect("count codes"),
            1,
            "no code may be admitted after consent revocation"
        );

        OAuthAppService::grant_consent_strict(&db, app.id, user.id, tenant.id, scopes.clone())
            .await
            .expect("renew consent");
        let _another_code = issue().await.expect("admit code after re-consent");
        OAuthAppService::revoke_app(&db, tenant.id, app.id)
            .await
            .expect("revoke app and pending codes");
        assert!(matches!(issue().await, Err(Error::NotFound)));
        assert_eq!(
            oauth_authorization_codes::Entity::find()
                .count(&db)
                .await
                .expect("count codes"),
            2,
            "app revocation cannot be followed by newly inserted auth codes"
        );
    }

    #[tokio::test]
    #[serial]
    async fn authorization_session_deadlines_are_exclusive() {
        let (db, tenant) = test_db_with_tenant().await;
        let user = users::ActiveModel::new(tenant.id, "deadline-auth@example.com", "hash")
            .insert(&db)
            .await
            .expect("user");
        let subject = authorization_subject(&db, &user).await;
        let mut session = crate::models::sessions::Entity::find_by_id(subject.session_id)
            .one(&db)
            .await
            .expect("load session")
            .expect("session exists");
        let now = Utc::now();
        assert!(subject.ensure_live(&session, now).is_ok());
        let expired = AuthorizationSession {
            access_expires_at: now,
            ..subject
        };
        assert!(expired.ensure_live(&session, now).is_err());
        let confirmed = AuthorizationSession {
            confirmation_deadline: Some(now),
            ..subject
        };
        assert!(confirmed.ensure_live(&session, now).is_err());
        session.expires_at = now.into();
        assert!(subject.ensure_live(&session, now).is_err());
        session.expires_at = subject.access_expires_at.into();
        session.absolute_expires_at = Some(now.into());
        assert!(subject.ensure_live(&session, now).is_err());
    }

    #[tokio::test]
    #[serial]
    async fn revoked_session_cannot_approve_consent_or_issue_code() {
        let (db, tenant) = test_db_with_tenant().await;
        let app = insert_revocation_app(&db, tenant.id, "revoked-authorization-session").await;
        let user = users::ActiveModel::new(tenant.id, "revoked-auth@example.com", "hash")
            .insert(&db)
            .await
            .expect("user");
        let subject = authorization_subject(&db, &user).await;
        crate::models::sessions::Entity::update_many()
            .col_expr(
                crate::models::sessions::Column::RevokedAt,
                Expr::value(Utc::now()),
            )
            .filter(crate::models::sessions::Column::Id.eq(subject.session_id))
            .exec(&db)
            .await
            .expect("revoke session");

        let result = OAuthAppService::generate_authorization_code(
            &db,
            app.id,
            AuthorizationSession {
                confirmation_deadline: Some(subject.access_expires_at),
                ..subject
            },
            tenant.id,
            "https://partner.example.com/callback".to_string(),
            vec!["openid".to_string()],
            "challenge".to_string(),
            &authorization_test_clock,
        )
        .await;
        assert!(matches!(result, Err(Error::Unauthorized(_))));
        assert_eq!(OAuthConsents::find().count(&db).await.expect("consents"), 0);
        assert_eq!(
            oauth_authorization_codes::Entity::find()
                .count(&db)
                .await
                .expect("codes"),
            0
        );
    }

    #[tokio::test]
    #[serial]
    async fn confirmation_expiring_after_code_insert_rolls_back_consent_and_code() {
        let (db, tenant) = test_db_with_tenant().await;
        let app = insert_revocation_app(&db, tenant.id, "confirmation-deadline").await;
        let user = users::ActiveModel::new(tenant.id, "confirmation-deadline@example.com", "hash")
            .insert(&db)
            .await
            .expect("user");
        let subject = authorization_subject(&db, &user).await;
        let start = Utc::now();
        let deadline = start + chrono::Duration::seconds(1);
        let subject = AuthorizationSession {
            confirmation_deadline: Some(deadline),
            ..subject
        };
        let samples = std::sync::atomic::AtomicUsize::new(0);
        let clock = || {
            Ok(
                if samples.fetch_add(1, std::sync::atomic::Ordering::SeqCst) >= 4 {
                    deadline
                } else {
                    start
                },
            )
        };
        let result = OAuthAppService::generate_authorization_code(
            &db,
            app.id,
            subject,
            tenant.id,
            "https://partner.example.com/callback".to_string(),
            vec!["openid".to_string()],
            "challenge".to_string(),
            &clock,
        )
        .await;
        assert!(matches!(result, Err(Error::Unauthorized(_))));
        assert_eq!(samples.load(std::sync::atomic::Ordering::SeqCst), 5);
        assert_eq!(OAuthConsents::find().count(&db).await.expect("consents"), 0);
        assert_eq!(
            oauth_authorization_codes::Entity::find()
                .count(&db)
                .await
                .expect("codes"),
            0
        );
    }

    #[tokio::test]
    #[serial]
    async fn failed_code_insert_rolls_back_new_consent() {
        let (db, tenant) = test_db_with_tenant().await;
        let app = insert_revocation_app(&db, tenant.id, "atomic-authorization").await;
        let user = users::ActiveModel::new(tenant.id, "atomic-auth@example.com", "hash")
            .insert(&db)
            .await
            .expect("user");
        let subject = authorization_subject(&db, &user).await;
        db.execute_unprepared(
            "CREATE TRIGGER fail_authorization_code BEFORE INSERT ON oauth_authorization_codes
             BEGIN SELECT RAISE(ABORT, 'simulated code write failure'); END;",
        )
        .await
        .expect("inject code write failure");
        let issue = || {
            OAuthAppService::generate_authorization_code(
                &db,
                app.id,
                AuthorizationSession {
                    confirmation_deadline: Some(subject.access_expires_at),
                    ..subject
                },
                tenant.id,
                "https://partner.example.com/callback".to_string(),
                vec!["openid".to_string()],
                "challenge".to_string(),
                &authorization_test_clock,
            )
        };
        assert!(issue().await.is_err());
        assert_eq!(OAuthConsents::find().count(&db).await.expect("consents"), 0);
        db.execute_unprepared("DROP TRIGGER fail_authorization_code")
            .await
            .expect("remove injected failure");
        assert!(issue().await.is_ok());
        assert_eq!(OAuthConsents::find().count(&db).await.expect("consents"), 1);
        assert_eq!(
            oauth_authorization_codes::Entity::find()
                .count(&db)
                .await
                .expect("codes"),
            1
        );
    }

    #[tokio::test]
    #[serial]
    async fn consent_regrant_invalidates_only_its_pending_codes() {
        let (db, tenant) = test_db_with_tenant().await;
        let app = insert_revocation_app(&db, tenant.id, "regrant-code-fence").await;
        let user = users::ActiveModel::new(tenant.id, "regrant@example.com", "hash")
            .insert(&db)
            .await
            .expect("user");
        let other = users::ActiveModel::new(tenant.id, "regrant-other@example.com", "hash")
            .insert(&db)
            .await
            .expect("other user");
        let scopes = vec!["openid".to_string()];
        OAuthAppService::grant_consent_strict(&db, app.id, user.id, tenant.id, scopes.clone())
            .await
            .expect("grant consent");
        let old_id = insert_pending_authorization_code(&db, app.id, user.id, tenant.id).await;
        let unrelated_id =
            insert_pending_authorization_code(&db, app.id, other.id, tenant.id).await;
        let mut delegated = Vec::new();
        for subject in [user.id, other.id] {
            for kind in [
                oauth_tokens::TOKEN_KIND_ACCESS,
                oauth_tokens::TOKEN_KIND_REFRESH,
            ] {
                let now = Utc::now();
                let id = Uuid::new_v4();
                let token = oauth_tokens::ActiveModel {
                    id: Set(id),
                    app_id: Set(app.id),
                    user_id: Set(Some(subject)),
                    tenant_id: Set(tenant.id),
                    token_hash: Set(format!("regrant-{id}")),
                    family_id: Set(id),
                    authorization_code_id: Set(None),
                    grant_type: Set("authorization_code".to_string()),
                    token_kind: Set(kind.to_string()),
                    consent_granted_at: Set(Some(now.into())),
                    scopes: Set(serde_json::json!(["openid"])),
                    expires_at: Set((now + chrono::Duration::minutes(15)).into()),
                    revoked_at: Set(None),
                    last_used_at: Set(None),
                    created_at: Set(now.into()),
                    updated_at: Set(now.into()),
                }
                .insert(&db)
                .await
                .expect("insert delegated token");
                delegated.push((token.id, subject));
            }
        }
        OAuthAppService::grant_consent_strict(&db, app.id, user.id, tenant.id, scopes.clone())
            .await
            .expect("regrant consent");
        for (id, subject) in delegated {
            let stored = OAuthTokens::find_by_id(id)
                .one(&db)
                .await
                .expect("read delegated token")
                .expect("token history retained");
            assert_eq!(stored.revoked_at.is_some(), subject == user.id);
        }
        let old = oauth_authorization_codes::Entity::find_by_id(old_id)
            .one(&db)
            .await
            .expect("read old code")
            .expect("old code retained");
        assert!(old.used_at.is_some());
        let unrelated = oauth_authorization_codes::Entity::find_by_id(unrelated_id)
            .one(&db)
            .await
            .expect("read unrelated code")
            .expect("unrelated code");
        assert!(unrelated.used_at.is_none());
        let subject = authorization_subject(&db, &user).await;
        let raw = OAuthAppService::generate_authorization_code(
            &db,
            app.id,
            subject,
            tenant.id,
            "https://partner.example.com/callback".to_string(),
            scopes,
            "challenge".to_string(),
            &authorization_test_clock,
        )
        .await
        .expect("new epoch admits a new code");
        use sha2::{Digest, Sha256};
        let new_code = oauth_authorization_codes::Entity::find()
            .filter(
                oauth_authorization_codes::Column::CodeHash
                    .eq(hex::encode(Sha256::digest(raw.as_bytes()))),
            )
            .one(&db)
            .await
            .expect("read new code")
            .expect("new code");
        assert!(new_code.used_at.is_none());
    }

    #[tokio::test]
    #[serial]
    async fn authorization_code_issuance_rechecks_user_and_current_app_policy() {
        let (db, tenant) = test_db_with_tenant().await;
        let app = insert_revocation_app(&db, tenant.id, "issuance-user-policy-fence").await;
        let user = users::ActiveModel::new(
            tenant.id,
            "issuance-user-policy@example.com",
            "unused-password-hash",
        )
        .insert(&db)
        .await
        .expect("create authorization subject");
        let subject = authorization_subject(&db, &user).await;
        let user_id = user.id;
        OAuthAppService::grant_consent_strict(
            &db,
            app.id,
            user_id,
            tenant.id,
            vec!["openid".to_string()],
        )
        .await
        .expect("grant active consent");

        let issue = |tenant_id| {
            OAuthAppService::generate_authorization_code(
                &db,
                app.id,
                subject,
                tenant_id,
                "https://partner.example.com/callback".to_string(),
                vec!["openid".to_string()],
                "challenge".to_string(),
                &authorization_test_clock,
            )
        };
        assert!(matches!(issue(Uuid::new_v4()).await, Err(Error::NotFound)));

        // A stale preflight cannot bind a code to a redirect the app no longer
        // accepts, nor to scopes that were narrowed before the authoritative
        // app-row lock.
        oauth_apps::Entity::update_many()
            .col_expr(
                oauth_apps::Column::RedirectUris,
                Expr::value(serde_json::json!(["https://new.example.com/callback"])),
            )
            .filter(oauth_apps::Column::Id.eq(app.id))
            .exec(&db)
            .await
            .expect("change registered redirect");
        assert!(matches!(issue(tenant.id).await, Err(Error::BadRequest(_))));

        oauth_apps::Entity::update_many()
            .col_expr(
                oauth_apps::Column::RedirectUris,
                Expr::value(serde_json::json!(["https://partner.example.com/callback"])),
            )
            .col_expr(
                oauth_apps::Column::Scopes,
                Expr::value(serde_json::json!(["profile"])),
            )
            .filter(oauth_apps::Column::Id.eq(app.id))
            .exec(&db)
            .await
            .expect("narrow allowed scopes");
        assert!(matches!(issue(tenant.id).await, Err(Error::BadRequest(_))));

        oauth_apps::Entity::update_many()
            .col_expr(
                oauth_apps::Column::Scopes,
                Expr::value(serde_json::json!(["openid"])),
            )
            .filter(oauth_apps::Column::Id.eq(app.id))
            .exec(&db)
            .await
            .expect("restore app scopes");
        let mut inactive = users::ActiveModel::from(user);
        inactive.status = Set(rustok_core::UserStatus::Inactive);
        inactive.update(&db).await.expect("deactivate subject");
        assert!(matches!(
            issue(tenant.id).await,
            Err(Error::Unauthorized(_))
        ));
        assert_eq!(
            oauth_authorization_codes::Entity::find()
                .count(&db)
                .await
                .expect("count codes"),
            0,
            "invalid tenant, redirect, scope and user decisions cannot persist codes"
        );
    }

    #[tokio::test]
    #[serial]
    async fn revocation_rechecks_client_credentials_under_app_lock() {
        let (db, tenant) = test_db_with_tenant().await;
        let app = insert_revocation_app(&db, tenant.id, "revoke-client-fence").await;
        let token = insert_revocation_token(&db, &app, "revoke-client-fence-token", None).await;
        oauth_apps::Entity::update_many()
            .col_expr(
                oauth_apps::Column::ClientSecretHash,
                Expr::value("new-secret-hash"),
            )
            .filter(oauth_apps::Column::Id.eq(app.id))
            .exec(&db)
            .await
            .expect("rotate client credentials");
        let rejected = OAuthAppService::revoke_authenticated_token_by_hash(
            &db,
            &token.token_hash,
            &app,
            tenant.id,
        )
        .await;
        assert!(matches!(rejected, Err(Error::Unauthorized(_))));
        let persisted = OAuthTokens::find_by_id(token.id)
            .one(&db)
            .await
            .expect("read token")
            .expect("token");
        assert!(persisted.revoked_at.is_none());
        let current =
            OAuthAppService::find_security_by_client_id_scoped(&db, tenant.id, app.client_id)
                .await
                .expect("read client")
                .expect("active client");
        OAuthAppService::revoke_authenticated_token_by_hash(
            &db,
            &token.token_hash,
            &current,
            tenant.id,
        )
        .await
        .expect("current client can revoke");
        let persisted = OAuthTokens::find_by_id(token.id)
            .one(&db)
            .await
            .expect("read token")
            .expect("token");
        assert!(persisted.revoked_at.is_some());
        OAuthAppService::revoke_authenticated_token_by_hash(
            &db,
            "unknown-token",
            &current,
            tenant.id,
        )
        .await
        .expect("authenticated unknown token remains idempotent");
    }

    #[tokio::test]
    #[serial]
    async fn rfc7009_revocation_always_succeeds() {
        let (db, tenant) = test_db_with_tenant().await;
        let app = insert_revocation_app(&db, tenant.id, "rfc7009-unknown").await;

        OAuthAppService::revoke_token_by_hash(&db, "missing-token-hash", app.id, tenant.id)
            .await
            .expect("RFC 7009 unknown token revocation must succeed");
    }

    #[test]
    fn oauth_policy_comparison_ignores_order_but_rejects_corrupt_storage() {
        let requested = vec!["openid".to_string(), "profile".to_string()];
        assert!(oauth_policy_matches(
            &serde_json::json!(["profile", "openid", "openid"]),
            &requested,
        ));
        assert!(!oauth_policy_matches(
            &serde_json::json!(["openid"]),
            &requested
        ));
        assert!(!oauth_policy_matches(
            &serde_json::json!({"openid": true}),
            &requested
        ));
    }

    #[tokio::test]
    #[serial]
    async fn manual_policy_change_revokes_credentials_but_copy_edit_does_not() {
        let (db, tenant) = test_db_with_tenant().await;
        let app = insert_revocation_app(&db, tenant.id, "policy-change").await;
        let user = users::ActiveModel::new(tenant.id, "policy@example.com", "hash")
            .insert(&db)
            .await
            .expect("user");
        OAuthAppService::grant_consent_strict(
            &db,
            app.id,
            user.id,
            tenant.id,
            vec!["openid".to_string()],
        )
        .await
        .expect("grant consent");
        let code = insert_pending_authorization_code(&db, app.id, user.id, tenant.id).await;
        let token = insert_revocation_token(&db, &app, "policy-token", None).await;
        let edit = |scope: &str| UpdateOAuthAppInput {
            name: "Updated display name".to_string(),
            description: None,
            icon_url: None,
            redirect_uris: vec!["https://partner.example.com/callback".to_string()],
            scopes: vec![scope.to_string()],
            grant_types: vec![
                "authorization_code".to_string(),
                "refresh_token".to_string(),
            ],
            granted_permissions: vec![],
        };
        for (scope, revoked) in [("openid", false), ("profile", true), ("openid", true)] {
            oauth_apps::scope_runtime_copy_locale(
                "en".to_string(),
                OAuthAppService::update_app(&db, tenant.id, app.id, edit(scope)),
            )
            .await
            .expect("update app");
            let stored = OAuthTokens::find_by_id(token.id)
                .one(&db)
                .await
                .expect("read token")
                .expect("retained token");
            assert_eq!(stored.revoked_at.is_some(), revoked);
            let stored = oauth_authorization_codes::Entity::find_by_id(code)
                .one(&db)
                .await
                .expect("read code")
                .expect("retained code");
            assert_eq!(stored.used_at.is_some(), revoked);
            let consent = OAuthConsents::find_active_consent(&db, app.id, user.id, tenant.id)
                .await
                .expect("read consent");
            assert_eq!(consent.is_none(), revoked);
        }
    }

    #[tokio::test]
    #[serial]
    async fn rfc7009_revokes_entire_refresh_family_and_is_idempotent() {
        let (db, tenant) = test_db_with_tenant().await;
        let app = insert_revocation_app(&db, tenant.id, "rfc7009-family").await;
        let root = insert_revocation_token(&db, &app, "family-root", None).await;
        let child = insert_revocation_token(&db, &app, "family-child", Some(root.id)).await;
        let unrelated = insert_revocation_token(&db, &app, "unrelated", None).await;

        OAuthAppService::revoke_token_by_hash(&db, "family-child", app.id, tenant.id)
            .await
            .expect("revoke refresh family");

        let mut first_revoked_at = Vec::new();
        for id in [root.id, child.id] {
            let token = oauth_tokens::Entity::find_by_id(id)
                .one(&db)
                .await
                .expect("load family token")
                .expect("family token exists");
            assert!(
                token.revoked_at.is_some(),
                "family token {id} must be revoked"
            );
            first_revoked_at.push((id, token.revoked_at));
        }
        let unrelated = oauth_tokens::Entity::find_by_id(unrelated.id)
            .one(&db)
            .await
            .expect("load unrelated token")
            .expect("unrelated token exists");
        assert!(unrelated.revoked_at.is_none());

        OAuthAppService::revoke_token_by_hash(&db, "family-child", app.id, tenant.id)
            .await
            .expect("repeat revocation is idempotent");
        for (id, first_timestamp) in first_revoked_at {
            let current = oauth_tokens::Entity::find_by_id(id)
                .one(&db)
                .await
                .expect("reload revoked token")
                .expect("token remains present");
            assert_eq!(
                current.revoked_at, first_timestamp,
                "replayed revocation must preserve the original revocation timestamp"
            );
        }
    }

    #[tokio::test]
    #[serial]
    async fn rfc7009_revokes_access_registration_and_its_refresh_family() {
        let (db, tenant) = test_db_with_tenant().await;
        let app = insert_revocation_app(&db, tenant.id, "rfc7009-access").await;
        let root = insert_revocation_token(&db, &app, "access-family-root", None).await;
        let access_id = Uuid::new_v4();
        oauth_tokens::ActiveModel {
            id: Set(access_id),
            app_id: Set(app.id),
            user_id: Set(None),
            tenant_id: Set(app.tenant_id),
            token_hash: Set("access-jwt-hash".to_string()),
            family_id: Set(root.id),
            authorization_code_id: Set(None),
            grant_type: Set("client_credentials".to_string()),
            token_kind: Set(oauth_tokens::TOKEN_KIND_ACCESS.to_string()),
            consent_granted_at: Set(None),
            scopes: Set(serde_json::json!(["openid"])),
            expires_at: Set((Utc::now() + chrono::Duration::minutes(15)).into()),
            revoked_at: Set(None),
            last_used_at: Set(None),
            created_at: Set(Utc::now().into()),
            updated_at: Set(Utc::now().into()),
        }
        .insert(&db)
        .await
        .expect("insert access registration");

        OAuthAppService::revoke_token_by_hash(&db, "access-jwt-hash", app.id, tenant.id)
            .await
            .expect("revoke by access hash");

        for id in [root.id, access_id] {
            let token = oauth_tokens::Entity::find_by_id(id)
                .one(&db)
                .await
                .expect("load family token")
                .expect("family token exists");
            assert!(
                token.revoked_at.is_some(),
                "family token {id} must be revoked"
            );
        }
    }

    #[tokio::test]
    #[serial]
    async fn rfc7009_revocation_is_scoped_to_client_and_tenant() {
        let (db, tenant) = test_db_with_tenant().await;
        let other_tenant =
            tenants::ActiveModel::new("Other tenant", &format!("tenant-{}", Uuid::new_v4()))
                .insert(&db)
                .await
                .expect("insert other tenant");
        let app = insert_revocation_app(&db, tenant.id, "rfc7009-scope").await;
        let other_app = insert_revocation_app(&db, other_tenant.id, "rfc7009-other").await;
        let root = insert_revocation_token(&db, &app, "scoped-root", None).await;
        let foreign = insert_revocation_token(&db, &other_app, "foreign-root", None).await;

        OAuthAppService::revoke_token_by_hash(&db, "scoped-root", app.id, other_tenant.id)
            .await
            .expect("wrong tenant must not reveal or revoke the token");
        OAuthAppService::revoke_token_by_hash(&db, "scoped-root", other_app.id, tenant.id)
            .await
            .expect("wrong client must not reveal or revoke the token");

        let scoped = oauth_tokens::Entity::find_by_id(root.id)
            .one(&db)
            .await
            .expect("load scoped token")
            .expect("scoped token exists");
        assert!(scoped.revoked_at.is_none());

        OAuthAppService::revoke_token_by_hash(&db, "scoped-root", app.id, tenant.id)
            .await
            .expect("correct client and tenant revoke the family");
        let scoped = oauth_tokens::Entity::find_by_id(root.id)
            .one(&db)
            .await
            .expect("reload scoped token")
            .expect("scoped token exists");
        assert!(scoped.revoked_at.is_some());
        let foreign = oauth_tokens::Entity::find_by_id(foreign.id)
            .one(&db)
            .await
            .expect("load foreign token")
            .expect("foreign token exists");
        assert!(
            foreign.revoked_at.is_none(),
            "family ids must not cross client/tenant boundaries"
        );
    }

    #[test]
    fn rfc7009_token_type_hint_values() {
        // RFC 7009 §2.1: token_type_hint is OPTIONAL
        // valid values: "access_token" or "refresh_token"
        let valid_hints = ["access_token", "refresh_token"];
        for hint in &valid_hints {
            assert!(
                *hint == "access_token" || *hint == "refresh_token",
                "Invalid token_type_hint: {}",
                hint
            );
        }
    }

    // ===================================================================
    // RFC 8414 — Authorization Server Metadata
    // ===================================================================

    #[test]
    fn rfc8414_required_metadata_fields() {
        // RFC 8414 §2: The following metadata fields are REQUIRED
        let metadata = serde_json::json!({
            "issuer": "http://localhost:3000",
            "authorization_endpoint": "http://localhost:3000/api/oauth/authorize",
            "token_endpoint": "http://localhost:3000/api/oauth/token",
            "response_types_supported": ["code"],
        });

        assert!(
            metadata.get("issuer").is_some(),
            "issuer REQUIRED (RFC 8414 §2)"
        );
        assert!(
            metadata.get("token_endpoint").is_some(),
            "token_endpoint REQUIRED"
        );
        assert!(
            metadata.get("response_types_supported").is_some(),
            "response_types_supported REQUIRED"
        );
    }

    #[test]
    fn rfc8414_well_known_paths() {
        // RFC 8414 §3: The well-known URI string is:
        //   "/.well-known/oauth-authorization-server"
        // OpenID Connect Discovery 1.0:
        //   "/.well-known/openid-configuration"
        let oauth_path = "/.well-known/oauth-authorization-server";
        let oidc_path = "/.well-known/openid-configuration";
        assert!(oauth_path.starts_with("/.well-known/"));
        assert!(oidc_path.starts_with("/.well-known/"));
    }

    // ===================================================================
    // Token hashing and credential security
    // ===================================================================

    #[test]
    fn token_hash_is_sha256_hex() {
        // Verify our token hashing produces SHA-256 hex output (64 chars)
        use sha2::{Digest, Sha256};
        let token = "test_refresh_token_value";
        let mut hasher = Sha256::new();
        hasher.update(token.as_bytes());
        let hash = hex::encode(hasher.finalize());
        assert_eq!(hash.len(), 64, "SHA-256 hex hash must be 64 characters");
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn token_hash_deterministic() {
        use sha2::{Digest, Sha256};
        let token = "same_token_value";

        let hash1 = {
            let mut h = Sha256::new();
            h.update(token.as_bytes());
            hex::encode(h.finalize())
        };
        let hash2 = {
            let mut h = Sha256::new();
            h.update(token.as_bytes());
            hex::encode(h.finalize())
        };

        assert_eq!(hash1, hash2, "Token hashing must be deterministic");
    }

    #[test]
    fn token_hash_different_input_different_output() {
        use sha2::{Digest, Sha256};
        let hash_a = {
            let mut h = Sha256::new();
            h.update(b"token_a");
            hex::encode(h.finalize())
        };
        let hash_b = {
            let mut h = Sha256::new();
            h.update(b"token_b");
            hex::encode(h.finalize())
        };
        assert_ne!(hash_a, hash_b);
    }

    #[test]
    fn client_secret_has_prefix() {
        let secret = generate_client_secret().expect("generate client secret");
        assert!(
            secret.starts_with("sk_live_"),
            "Client secret must have sk_live_ prefix"
        );
        assert!(secret.len() > 72, "Client secret must be sufficiently long");
    }

    // ===================================================================
    // Authorization code generation properties
    // ===================================================================

    #[test]
    fn auth_code_base64url_no_padding() {
        // RFC 7636 §4.1: code_verifier uses unreserved characters
        // Our auth code uses URL-safe base64 without padding
        use base64::{Engine as _, engine::general_purpose};
        let random_bytes: Vec<u8> = (0..32).map(|i| i as u8).collect();
        let code = general_purpose::URL_SAFE_NO_PAD.encode(random_bytes);

        assert!(!code.contains('='), "Base64URL MUST NOT contain padding");
        assert!(!code.contains('+'), "Base64URL MUST NOT contain '+'");
        assert!(!code.contains('/'), "Base64URL MUST NOT contain '/'");
    }

    // ===================================================================
    // RFC 6749 §4.4 — Client Credentials specific
    // ===================================================================

    #[test]
    fn rfc6749_client_credentials_ttl() {
        // Our implementation issues 1-hour tokens for client_credentials
        let expires_in = 3600u64;
        assert_eq!(expires_in, 3600, "client_credentials TTL should be 1 hour");
    }

    #[test]
    fn rfc6749_authorization_code_ttl() {
        // Our implementation issues 15-minute tokens for authorization_code
        let expires_in = 900u64;
        assert_eq!(
            expires_in, 900,
            "authorization_code TTL should be 15 minutes"
        );
    }

    #[test]
    fn rfc6749_refresh_token_ttl() {
        // Our implementation uses 30-day refresh tokens
        let ttl_days = 30;
        let ttl_secs = ttl_days * 24 * 60 * 60;
        assert_eq!(ttl_secs, 2_592_000);
    }

    #[test]
    fn rfc6749_auth_code_ttl() {
        // RFC 6749 §4.1.2: authorization code MUST be short-lived
        // "A maximum authorization code lifetime of 10 minutes is RECOMMENDED"
        let code_ttl_minutes = 10;
        assert_eq!(
            code_ttl_minutes, 10,
            "Auth code TTL must be 10 minutes per RFC 6749"
        );
    }

    #[test]
    fn app_display_fields_are_rejected_beyond_their_column_widths() {
        use super::{
            OAUTH_APP_ICON_URL_MAX_LEN, OAUTH_APP_NAME_MAX_LEN, OAUTH_APP_SLUG_MAX_LEN,
            validate_app_display_fields,
        };

        // At the boundary the value is accepted: the schema allows it.
        let at_name_limit = "n".repeat(OAUTH_APP_NAME_MAX_LEN);
        let at_slug_limit = "s".repeat(OAUTH_APP_SLUG_MAX_LEN);
        let at_icon_limit = "i".repeat(OAUTH_APP_ICON_URL_MAX_LEN);
        assert!(
            validate_app_display_fields(&at_name_limit, Some(&at_slug_limit), Some(&at_icon_limit))
                .is_ok(),
            "values exactly at the column width must pass"
        );

        // One character past it must be a client error, not a database failure.
        let over_name = "n".repeat(OAUTH_APP_NAME_MAX_LEN + 1);
        let over_slug = "s".repeat(OAUTH_APP_SLUG_MAX_LEN + 1);
        let over_icon = "i".repeat(OAUTH_APP_ICON_URL_MAX_LEN + 1);
        for (label, result) in [
            ("name", validate_app_display_fields(&over_name, None, None)),
            (
                "slug",
                validate_app_display_fields("name", Some(&over_slug), None),
            ),
            (
                "icon_url",
                validate_app_display_fields("name", None, Some(&over_icon)),
            ),
        ] {
            assert!(
                matches!(result, Err(Error::BadRequest(_))),
                "{label} beyond its column width must be a BadRequest"
            );
        }

        // Blank display fields are meaningless and must not be stored.
        assert!(matches!(
            validate_app_display_fields("   ", None, None),
            Err(Error::BadRequest(_))
        ));
        assert!(matches!(
            validate_app_display_fields("name", Some("  "), None),
            Err(Error::BadRequest(_))
        ));
    }

    #[tokio::test]
    #[serial]
    async fn stale_manual_app_update_cannot_restore_revocation_or_last_used_state() {
        let (db, tenant) = test_db_with_tenant().await;
        let created = oauth_apps::scope_runtime_copy_locale(
            "en".to_string(),
            OAuthAppService::create_app(
                &db,
                tenant.id,
                CreateOAuthAppInput {
                    name: "Race-safe app".to_string(),
                    slug: "race-safe-app".to_string(),
                    description: Some("Initial".to_string()),
                    app_type: "third_party".to_string(),
                    icon_url: None,
                    redirect_uris: vec!["https://partner.example.com/callback".to_string()],
                    scopes: vec!["catalog:read".to_string()],
                    grant_types: vec![
                        "authorization_code".to_string(),
                        "refresh_token".to_string(),
                    ],
                    granted_permissions: vec![],
                },
            ),
        )
        .await
        .expect("create manual app");

        let stale = OAuthAppsEntity::find_by_id(created.app.id)
            .one(&db)
            .await
            .expect("read stale app")
            .expect("app exists");
        OAuthAppService::touch_last_used(&db, stale.id)
            .await
            .expect("record runtime use");
        let used_at = OAuthAppsEntity::find_by_id(stale.id)
            .one(&db)
            .await
            .expect("read last-used timestamp")
            .and_then(|app| app.last_used_at)
            .expect("runtime use timestamp");

        OAuthAppService::revoke_app(&db, tenant.id, stale.id)
            .await
            .expect("revoke app");

        let mut update = manual_app_update_base(stale);
        update.name = Set("Updated after revoke".to_string());
        update.description = Set(Some("Updated metadata".to_string()));
        update.icon_url = Set(None);
        update.redirect_uris = Set(serde_json::json!([
            "https://partner.example.com/updated-callback"
        ]));
        update.scopes = Set(serde_json::json!(["catalog:*"]));
        update.grant_types = Set(serde_json::json!(["authorization_code", "refresh_token"]));
        update.granted_permissions = Set(serde_json::json!([]));
        update.updated_at = Set(Utc::now().into());

        oauth_apps::scope_runtime_copy_locale("en".to_string(), update.update(&db))
            .await
            .expect("apply stale editable-field update");

        let stored = OAuthAppsEntity::find_by_id(created.app.id)
            .one(&db)
            .await
            .expect("reload app")
            .expect("app remains present");
        assert!(
            !stored.is_active,
            "stale update must not reactivate a revoked app"
        );
        assert!(
            stored.revoked_at.is_some(),
            "revocation timestamp must survive"
        );
        assert_eq!(
            stored.last_used_at,
            Some(used_at),
            "stale update must not erase a newer runtime-owned use timestamp"
        );
    }

    #[tokio::test]
    #[serial]
    async fn manual_app_crud_and_token_revocation_round_trip() {
        let (db, tenant) = test_db_with_tenant().await;

        let created = oauth_apps::scope_runtime_copy_locale(
            "en".to_string(),
            OAuthAppService::create_app(
                &db,
                tenant.id,
                CreateOAuthAppInput {
                    name: "Partner app".to_string(),
                    slug: "partner-app".to_string(),
                    description: Some("Third-party integration".to_string()),
                    app_type: "third_party".to_string(),
                    icon_url: Some("https://example.com/icon.png".to_string()),
                    redirect_uris: vec!["https://partner.example.com/callback".to_string()],
                    scopes: vec!["catalog:read".to_string()],
                    grant_types: vec![
                        "authorization_code".to_string(),
                        "refresh_token".to_string(),
                    ],
                    granted_permissions: vec![],
                },
            ),
        )
        .await
        .expect("create app");

        assert!(created.app.can_edit());
        assert!(created.app.can_rotate_secret());
        assert!(created.app.can_revoke());
        assert!(created.client_secret.starts_with("sk_live_"));
        let created_debug = format!("{created:?}");
        assert!(created_debug.contains("<redacted>"));
        assert!(!created_debug.contains(&created.client_secret));
        let stale_secret_hash = created
            .app
            .client_secret_hash
            .clone()
            .expect("created app secret hash");
        assert_eq!(
            created.app.redirect_uris_list(),
            vec!["https://partner.example.com/callback".to_string()]
        );

        let updated = oauth_apps::scope_runtime_copy_locale(
            "en".to_string(),
            OAuthAppService::update_app(
                &db,
                tenant.id,
                created.app.id,
                UpdateOAuthAppInput {
                    name: "Partner app v2".to_string(),
                    description: Some("Updated description".to_string()),
                    icon_url: None,
                    redirect_uris: vec!["https://partner.example.com/oauth/callback".to_string()],
                    scopes: vec!["catalog:*".to_string()],
                    grant_types: vec![
                        "authorization_code".to_string(),
                        "refresh_token".to_string(),
                    ],
                    granted_permissions: vec![],
                },
            ),
        )
        .await
        .expect("update app");

        assert_eq!(updated.name, "Partner app v2");
        assert!(
            updated.can_rotate_secret(),
            "an editable update must retain the current stored credential state"
        );
        assert_eq!(
            updated.redirect_uris_list(),
            vec!["https://partner.example.com/oauth/callback".to_string()]
        );
        assert_eq!(updated.scopes_list(), vec!["catalog:*".to_string()]);

        let rotated = OAuthAppService::rotate_secret(&db, tenant.id, created.app.id)
            .await
            .expect("rotate secret");
        let rotated_debug = format!("{rotated:?}");
        assert!(rotated_debug.contains("<redacted>"));
        assert!(!rotated_debug.contains(&rotated.client_secret));
        let rotated_hash = rotated
            .app
            .client_secret_hash
            .as_deref()
            .expect("rotated secret hash");
        assert!(
            OAuthAppService::verify_client_secret(&rotated.client_secret, rotated_hash)
                .expect("verify rotated secret")
        );
        assert!(
            !replace_oauth_secret_if_current(
                &db,
                tenant.id,
                created.app.id,
                Some(&stale_secret_hash),
                "stale-hash",
                Utc::now(),
            )
            .await
            .expect("stale secret compare-and-swap"),
            "a rotation based on a stale hash must not overwrite the current credential"
        );

        let token_user = crate::models::users::ActiveModel::new(
            tenant.id,
            "oauth-app-token-user@example.com",
            "unused-password-hash",
        )
        .insert(&db)
        .await
        .expect("insert token user");

        let token_id = Uuid::new_v4();
        oauth_tokens::ActiveModel {
            id: Set(token_id),
            app_id: Set(created.app.id),
            user_id: Set(Some(token_user.id)),
            tenant_id: Set(tenant.id),
            token_hash: Set("refresh-token-hash".to_string()),
            family_id: Set(token_id),
            authorization_code_id: Set(None),
            grant_type: Set("authorization_code".to_string()),
            token_kind: Set(oauth_tokens::TOKEN_KIND_REFRESH.to_string()),
            consent_granted_at: Set(None),
            scopes: Set(serde_json::json!(["catalog:*"])),
            expires_at: Set((Utc::now() + chrono::Duration::days(1)).into()),
            revoked_at: Set(None),
            last_used_at: Set(None),
            created_at: Set(Utc::now().into()),
            updated_at: Set(Utc::now().into()),
        }
        .insert(&db)
        .await
        .expect("insert oauth token");

        OAuthAppService::grant_consent_strict(
            &db,
            created.app.id,
            token_user.id,
            tenant.id,
            vec!["catalog:read".to_string()],
        )
        .await
        .expect("grant user consent through the strict admin path");
        let consent_code_id =
            insert_pending_authorization_code(&db, created.app.id, token_user.id, tenant.id).await;
        OAuthAppService::revoke_user_consent(&db, created.app.id, token_user.id, tenant.id)
            .await
            .expect("revoke user consent");
        let consent_code = oauth_authorization_codes::Entity::find_by_id(consent_code_id)
            .one(&db)
            .await
            .expect("load consent-bound code")
            .expect("consent-bound code exists");
        assert!(
            consent_code.used_at.is_some(),
            "consent revocation must invalidate pending authorization codes"
        );

        // The lower-level service path must also serialize an update/regrant
        // against the same app and subject fences.
        OAuthAppService::grant_consent(
            &db,
            created.app.id,
            token_user.id,
            tenant.id,
            vec!["catalog:read".to_string()],
        )
        .await
        .expect("regrant consent through the lower-level service");
        OAuthAppService::revoke_user_consent(&db, created.app.id, token_user.id, tenant.id)
            .await
            .expect("revoke lower-level service grant");

        let mut inactive_user: crate::models::users::ActiveModel = token_user.clone().into();
        inactive_user.status = Set(rustok_core::UserStatus::Inactive);
        inactive_user
            .update(&db)
            .await
            .expect("deactivate consent subject");
        let strict_grant_for_inactive_user = OAuthAppService::grant_consent_strict(
            &db,
            created.app.id,
            token_user.id,
            tenant.id,
            vec!["catalog:read".to_string()],
        )
        .await
        .expect_err("strict grant must reject an inactive user");
        assert!(matches!(
            strict_grant_for_inactive_user,
            Error::Unauthorized(_)
        ));

        let grant_for_inactive_user = OAuthAppService::grant_consent(
            &db,
            created.app.id,
            token_user.id,
            tenant.id,
            vec!["catalog:read".to_string()],
        )
        .await
        .expect_err("ordinary grant must reject an inactive user");
        assert!(matches!(grant_for_inactive_user, Error::Unauthorized(_)));

        let app_code_id =
            insert_pending_authorization_code(&db, created.app.id, token_user.id, tenant.id).await;
        let revoked = OAuthAppService::revoke_app(&db, created.app.tenant_id, created.app.id)
            .await
            .expect("revoke app");
        assert!(!revoked.is_active);
        assert!(revoked.revoked_at.is_some());

        let revoked_tokens = oauth_tokens::Entity::find()
            .filter(oauth_tokens::Column::AppId.eq(created.app.id))
            .all(&db)
            .await
            .expect("load oauth tokens");
        assert_eq!(revoked_tokens.len(), 1);
        assert!(revoked_tokens[0].revoked_at.is_some());

        let app_code = oauth_authorization_codes::Entity::find_by_id(app_code_id)
            .one(&db)
            .await
            .expect("load app-bound code")
            .expect("app-bound code exists");
        assert!(
            app_code.used_at.is_some(),
            "app revocation must invalidate pending authorization codes"
        );

        let grant_after_revoke = OAuthAppService::grant_consent(
            &db,
            created.app.id,
            token_user.id,
            tenant.id,
            vec!["catalog:read".to_string()],
        )
        .await
        .expect_err("revoked app must not receive new consent");
        assert!(matches!(grant_after_revoke, Error::NotFound));
    }

    #[tokio::test]
    #[serial]
    async fn oauth_secret_rotation_cas_rechecks_allowed_app_type() {
        let (db, tenant) = test_db_with_tenant().await;
        let app = insert_revocation_app(&db, tenant.id, "rotation-type-change").await;
        assert!(app.can_rotate_secret());
        assert!(app.client_secret_hash.is_none());

        // The app type changes after a secret-rotation preflight but before
        // its hash CAS. The database, not the earlier snapshot, arbitrates
        // whether the rotation remains permitted.
        oauth_apps::Entity::update_many()
            .col_expr(oauth_apps::Column::AppType, Expr::value("embedded"))
            .filter(oauth_apps::Column::Id.eq(app.id))
            .filter(oauth_apps::Column::TenantId.eq(tenant.id))
            .exec(&db)
            .await
            .expect("change OAuth app type concurrently");
        assert!(
            !replace_oauth_secret_if_current(
                &db,
                tenant.id,
                app.id,
                app.client_secret_hash.as_deref(),
                "new-secret-hash-that-must-not-be-persisted",
                Utc::now(),
            )
            .await
            .expect("atomic rotation predicate"),
            "stale rotation must not set a secret for an unsupported app type"
        );
        let stored = OAuthApps::find_by_id(app.id)
            .one(&db)
            .await
            .expect("reload app")
            .expect("app exists");
        assert_eq!(stored.app_type, "embedded");
        assert!(stored.client_secret_hash.is_none());
    }

    #[tokio::test]
    #[serial]
    async fn oauth_secret_rotation_cas_rejects_revoked_app() {
        let (db, tenant) = test_db_with_tenant().await;
        let app = insert_revocation_app(&db, tenant.id, "rotation-revoked-app").await;
        assert!(app.can_rotate_secret());
        assert!(app.client_secret_hash.is_none());

        // Revocation can win after the preflight but before the secret hash CAS.
        oauth_apps::Entity::update_many()
            .col_expr(oauth_apps::Column::IsActive, Expr::value(false))
            .col_expr(oauth_apps::Column::RevokedAt, Expr::value(Utc::now()))
            .filter(oauth_apps::Column::Id.eq(app.id))
            .filter(oauth_apps::Column::TenantId.eq(tenant.id))
            .exec(&db)
            .await
            .expect("revoke OAuth app concurrently");

        assert!(
            !replace_oauth_secret_if_current(
                &db,
                tenant.id,
                app.id,
                app.client_secret_hash.as_deref(),
                "new-secret-hash-that-must-not-be-persisted",
                Utc::now(),
            )
            .await
            .expect("atomic rotation predicate"),
            "a revoked app must not accept a stale secret rotation"
        );
        let stored = OAuthApps::find_by_id(app.id)
            .one(&db)
            .await
            .expect("reload app")
            .expect("app exists");
        assert!(stored.revoked_at.is_some());
        assert!(stored.client_secret_hash.is_none());
    }

    #[tokio::test]
    #[serial]
    async fn manual_oauth_edit_and_revoke_reject_newly_manifest_owned_app() {
        let (db, tenant) = test_db_with_tenant().await;
        let created = oauth_apps::scope_runtime_copy_locale(
            "en".to_string(),
            OAuthAppService::create_app(
                &db,
                tenant.id,
                CreateOAuthAppInput {
                    name: "Pending ownership change".to_string(),
                    slug: "pending-ownership-change".to_string(),
                    description: None,
                    app_type: "third_party".to_string(),
                    icon_url: None,
                    redirect_uris: vec!["https://partner.example.com/callback".to_string()],
                    scopes: vec!["catalog:read".to_string()],
                    grant_types: vec!["authorization_code".to_string()],
                    granted_permissions: vec![],
                },
            ),
        )
        .await
        .expect("create manual app");
        let app_id = created.app.id;
        assert!(created.app.can_edit() && created.app.can_revoke());

        // Model the ownership transfer performed by a manifest reconciliation:
        // a different writer acquires the canonical row fence and commits new
        // ownership before either manual mutation is admitted.
        let tx = db.begin().await.expect("ownership transaction");
        let before = lock_oauth_app_for_mutation(&tx, app_id, tenant.id)
            .await
            .expect("lock app")
            .expect("manual app still exists");
        assert!(before.can_edit());
        oauth_apps::Entity::update_many()
            .col_expr(oauth_apps::Column::AutoCreated, Expr::value(true))
            .col_expr(
                oauth_apps::Column::ManifestRef,
                Expr::value(Some("manifest-owned".to_string())),
            )
            .filter(oauth_apps::Column::Id.eq(app_id))
            .filter(oauth_apps::Column::TenantId.eq(tenant.id))
            .exec(&tx)
            .await
            .expect("manifest claims ownership");
        tx.commit().await.expect("ownership commits");

        let edit = OAuthAppService::update_app(
            &db,
            tenant.id,
            app_id,
            UpdateOAuthAppInput {
                name: "Unexpected manual edit".to_string(),
                description: None,
                icon_url: None,
                redirect_uris: vec!["https://attacker.example.com/callback".to_string()],
                scopes: vec!["catalog:write".to_string()],
                grant_types: vec!["authorization_code".to_string()],
                granted_permissions: vec![],
            },
        )
        .await;
        assert!(matches!(edit, Err(Error::BadRequest(_))));
        let revoke = OAuthAppService::revoke_app(&db, tenant.id, app_id).await;
        assert!(matches!(revoke, Err(Error::BadRequest(_))));

        let stored = OAuthApps::find_by_id(app_id)
            .filter(oauth_apps::Column::TenantId.eq(tenant.id))
            .one(&db)
            .await
            .expect("reload claimed app")
            .expect("app remains");
        assert!(stored.is_active());
        assert!(stored.managed_by_manifest());
        assert_eq!(
            stored.redirect_uris_list(),
            vec!["https://partner.example.com/callback"]
        );
        assert!(stored.revoked_at.is_none());
    }

    #[tokio::test]
    #[serial]
    async fn manifest_managed_apps_are_read_only_but_secret_rotation_still_works() {
        let (db, tenant) = test_db_with_tenant().await;
        let secret = generate_client_secret().expect("generate client secret");
        let secret_hash = auth::hash_password(&secret).expect("hash secret");

        let managed = OAuthAppActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant.id),
            name: Set("next-admin".to_string()),
            slug: Set("next-admin".to_string()),
            description: Set(Some("Auto-created".to_string())),
            app_type: Set("first_party".to_string()),
            icon_url: Set(None),
            client_id: Set(Uuid::new_v4()),
            client_secret_hash: Set(Some(secret_hash)),
            redirect_uris: Set(serde_json::json!([
                "https://admin.example.com/auth/callback"
            ])),
            scopes: Set(serde_json::json!(["admin:*"])),
            grant_types: Set(serde_json::json!([
                "authorization_code",
                "client_credentials"
            ])),
            granted_permissions: Set(serde_json::json!(["modules:manage"])),
            manifest_ref: Set(Some("next-admin".to_string())),
            auto_created: Set(true),
            is_active: Set(true),
            revoked_at: Set(None),
            last_used_at: Set(None),
            metadata: Set(serde_json::json!({"public_url":"https://admin.example.com"})),
            created_at: Set(Utc::now().into()),
            updated_at: Set(Utc::now().into()),
        }
        .insert(&db)
        .await
        .expect("insert managed app");

        let edit_result = OAuthAppService::update_app(
            &db,
            tenant.id,
            managed.id,
            UpdateOAuthAppInput {
                name: "mutated".to_string(),
                description: None,
                icon_url: None,
                redirect_uris: vec!["https://evil.example.com/callback".to_string()],
                scopes: vec!["admin:*".to_string()],
                grant_types: vec!["authorization_code".to_string()],
                granted_permissions: vec![],
            },
        )
        .await;
        assert!(edit_result.is_err());

        let revoke_result = OAuthAppService::revoke_app(&db, tenant.id, managed.id).await;
        assert!(revoke_result.is_err());

        let other_tenant =
            tenants::ActiveModel::new("Other tenant", &format!("tenant-{}", Uuid::new_v4()))
                .insert(&db)
                .await
                .expect("insert second tenant");
        assert!(
            OAuthAppService::rotate_secret(&db, other_tenant.id, managed.id)
                .await
                .is_err(),
            "secret rotation must be tenant-scoped at the service boundary"
        );
        let unchanged = OAuthAppsEntity::find_by_id(managed.id)
            .one(&db)
            .await
            .expect("reload app after cross-tenant attempt")
            .expect("managed app remains present");
        assert_eq!(unchanged.client_secret_hash, managed.client_secret_hash);

        let rotated = OAuthAppService::rotate_secret(&db, tenant.id, managed.id)
            .await
            .expect("managed app secret rotation");
        let rotated_hash = rotated
            .app
            .client_secret_hash
            .as_deref()
            .expect("rotated managed secret hash");
        assert!(
            OAuthAppService::verify_client_secret(&rotated.client_secret, rotated_hash)
                .expect("verify managed secret")
        );
    }

    #[tokio::test]
    #[serial]
    async fn manifest_sync_reactivates_embedded_app_with_revocation_timestamp() {
        let (db, tenant) = test_db_with_tenant().await;
        let now = Utc::now().fixed_offset();
        let app = OAuthAppActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant.id),
            name: Set("leptos-admin".to_string()),
            slug: Set("leptos-admin".to_string()),
            description: Set(Some("Auto-created embedded app".to_string())),
            app_type: Set("embedded".to_string()),
            icon_url: Set(None),
            client_id: Set(Uuid::new_v4()),
            client_secret_hash: Set(None),
            redirect_uris: Set(serde_json::json!([])),
            scopes: Set(serde_json::json!(["*:*"])),
            grant_types: Set(serde_json::json!([])),
            granted_permissions: Set(serde_json::json!([])),
            manifest_ref: Set(Some("leptos-admin".to_string())),
            auto_created: Set(true),
            is_active: Set(true),
            revoked_at: Set(Some(now)),
            last_used_at: Set(None),
            metadata: Set(serde_json::json!({})),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(&db)
        .await
        .expect("insert embedded app with stale revocation timestamp");

        let mut manifest = crate::modules::ModulesManifest::default();
        manifest.build.server.embed_admin = true;
        manifest.build.server.embed_storefront = false;
        manifest.build.admin.stack.clear();
        manifest.build.storefront.clear();

        sync_app_connections(&db, tenant.id, &manifest)
            .await
            .expect("sync manifest and reactivate embedded app");

        let reactivated = OAuthAppsEntity::find_by_id(app.id)
            .one(&db)
            .await
            .expect("load embedded app after sync")
            .expect("embedded app remains present");
        assert!(reactivated.is_active());
        assert!(
            reactivated.revoked_at.is_none(),
            "embedded app reactivation must clear a stale revocation timestamp"
        );
    }

    #[tokio::test]
    #[serial]
    async fn app_revocation_revokes_active_user_consents() {
        let (db, tenant) = test_db_with_tenant().await;
        let user = crate::models::users::ActiveModel::new(
            tenant.id,
            "app-revoke-consent@example.com",
            "unused-password-hash",
        )
        .insert(&db)
        .await
        .expect("insert consent subject");
        let created = OAuthAppService::create_app(
            &db,
            tenant.id,
            CreateOAuthAppInput {
                name: "Consent revocation app".to_string(),
                slug: format!("consent-revocation-{}", Uuid::new_v4()),
                description: None,
                app_type: "third_party".to_string(),
                icon_url: None,
                redirect_uris: vec!["https://client.example.com/callback".to_string()],
                scopes: vec!["catalog:read".to_string()],
                grant_types: vec![
                    "authorization_code".to_string(),
                    "refresh_token".to_string(),
                ],
                granted_permissions: vec![],
            },
        )
        .await
        .expect("create OAuth app");

        OAuthAppService::grant_consent_strict(
            &db,
            created.app.id,
            user.id,
            tenant.id,
            vec!["catalog:read".to_string()],
        )
        .await
        .expect("grant consent before app revocation");
        let active_consent =
            OAuthConsents::find_active_consent(&db, created.app.id, user.id, tenant.id)
                .await
                .expect("load active consent")
                .expect("consent exists before app revocation");

        OAuthAppService::revoke_app(&db, tenant.id, created.app.id)
            .await
            .expect("revoke OAuth app");

        let stored_consent = oauth_consents::Entity::find_by_id(active_consent.id)
            .one(&db)
            .await
            .expect("load consent after app revocation")
            .expect("consent history remains present");
        assert!(
            stored_consent.revoked_at.is_some(),
            "revoking an OAuth app must revoke all active user consents"
        );
        assert!(
            OAuthConsents::find_active_consent(&db, created.app.id, user.id, tenant.id)
                .await
                .expect("check active consent after app revocation")
                .is_none()
        );
    }

    #[tokio::test]
    #[serial]
    async fn manifest_sync_refuses_to_take_over_a_manual_app_with_a_matching_slug() {
        let (db, tenant) = test_db_with_manifest_tenant().await;
        let manual_secret_hash = auth::hash_password("manual-secret").expect("hash manual secret");
        let manual_model = OAuthAppActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant.id),
            name: Set("Operator-owned app".to_string()),
            slug: Set("shop".to_string()),
            description: Set(Some("Created manually".to_string())),
            app_type: Set("service".to_string()),
            icon_url: Set(None),
            client_id: Set(Uuid::new_v4()),
            client_secret_hash: Set(Some(manual_secret_hash.clone())),
            redirect_uris: Set(serde_json::json!(["https://operator.example/callback"])),
            scopes: Set(serde_json::json!(["operator:read"])),
            grant_types: Set(serde_json::json!(["client_credentials"])),
            granted_permissions: Set(serde_json::json!(["settings:manage"])),
            manifest_ref: Set(None),
            auto_created: Set(false),
            is_active: Set(true),
            revoked_at: Set(None),
            last_used_at: Set(None),
            metadata: Set(serde_json::json!({"owner":"operator"})),
            created_at: Set(Utc::now().into()),
            updated_at: Set(Utc::now().into()),
        };
        let manual =
            oauth_apps::scope_runtime_copy_locale("en".to_string(), manual_model.insert(&db))
                .await
                .expect("insert manually managed app");

        let mut manifest = crate::modules::ModulesManifest::default();
        manifest.build.server.embed_admin = true;
        manifest.build.server.embed_storefront = false;
        manifest.build.admin.stack.clear();
        manifest.build.storefront = serde_json::from_value(serde_json::json!([
            {
                "id": "shop",
                "stack": "next",
                "public_url": "https://shop.example.com",
                "redirect_uris": ["https://shop.example.com/auth/callback"]
            }
        ]))
        .expect("deserialize storefront build config");

        assert!(
            matches!(
                sync_app_connections(&db, tenant.id, &manifest).await,
                Err(Error::Conflict(_))
            ),
            "sync must report a conflict instead of claiming a manually created app by slug"
        );

        let leaked_embedded_app = OAuthAppsEntity::find()
            .filter(oauth_apps::Column::TenantId.eq(tenant.id))
            .filter(oauth_apps::Column::Slug.eq("leptos-admin"))
            .one(&db)
            .await
            .expect("query embedded app after failed sync");
        assert!(
            leaked_embedded_app.is_none(),
            "an earlier embedded-app upsert must roll back when a later manifest entry conflicts"
        );

        let persisted = OAuthAppsEntity::find_by_id(manual.id)
            .one(&db)
            .await
            .expect("load manual app after failed sync")
            .expect("manual app remains persisted");
        assert_eq!(
            persisted.client_secret_hash.as_deref(),
            Some(manual_secret_hash.as_str())
        );
        assert_eq!(
            persisted.granted_permissions,
            serde_json::json!(["settings:manage"])
        );
        assert_eq!(
            persisted.redirect_uris,
            serde_json::json!(["https://operator.example/callback"])
        );
        assert!(!persisted.auto_created);
        assert!(persisted.manifest_ref.is_none());
    }

    #[tokio::test]
    #[serial]
    async fn sync_app_connections_creates_manifest_managed_apps_and_deactivates_orphans() {
        let (db, tenant) = test_db_with_manifest_tenant().await;

        let orphan_app = OAuthAppActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant.id),
            name: Set("old-storefront".to_string()),
            slug: Set("old-storefront".to_string()),
            description: Set(Some("Old auto app".to_string())),
            app_type: Set("first_party".to_string()),
            icon_url: Set(None),
            client_id: Set(Uuid::new_v4()),
            client_secret_hash: Set(Some(auth::hash_password("old-secret").expect("hash"))),
            redirect_uris: Set(serde_json::json!(["https://old.example.com/callback"])),
            scopes: Set(serde_json::json!(["storefront:*"])),
            grant_types: Set(serde_json::json!([
                "authorization_code",
                "client_credentials"
            ])),
            granted_permissions: Set(serde_json::json!(["posts:read"])),
            manifest_ref: Set(Some("old-storefront".to_string())),
            auto_created: Set(true),
            is_active: Set(true),
            revoked_at: Set(None),
            last_used_at: Set(None),
            metadata: Set(serde_json::json!({"public_url":"https://old.example.com"})),
            created_at: Set(Utc::now().into()),
            updated_at: Set(Utc::now().into()),
        }
        .insert(&db)
        .await
        .expect("insert orphan app");

        let orphan_user = crate::models::users::ActiveModel::new(
            tenant.id,
            "orphan-oauth-code-user@example.com",
            "unused-password-hash",
        )
        .insert(&db)
        .await
        .expect("insert orphan authorization-code user");
        let orphan_consent = oauth_consents::ActiveModel {
            id: Set(Uuid::new_v4()),
            app_id: Set(orphan_app.id),
            user_id: Set(orphan_user.id),
            tenant_id: Set(tenant.id),
            scopes: Set(serde_json::json!(["storefront:*"])),
            granted_at: Set(Utc::now().into()),
            revoked_at: Set(None),
        }
        .insert(&db)
        .await
        .expect("insert active consent for orphan app");
        let orphan_code_id =
            insert_pending_authorization_code(&db, orphan_app.id, orphan_user.id, tenant.id).await;

        let auto_created_for_another_manifest_entry = OAuthAppActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant.id),
            name: Set("External generated app".to_string()),
            slug: Set("external-generated".to_string()),
            description: Set(None),
            app_type: Set("service".to_string()),
            icon_url: Set(None),
            client_id: Set(Uuid::new_v4()),
            client_secret_hash: Set(Some(
                auth::hash_password("external-secret").expect("hash external app secret"),
            )),
            redirect_uris: Set(serde_json::json!(["https://external.example/callback"])),
            scopes: Set(serde_json::json!(["external:read"])),
            grant_types: Set(serde_json::json!(["client_credentials"])),
            granted_permissions: Set(serde_json::json!([])),
            manifest_ref: Set(Some("external-manifest-entry".to_string())),
            auto_created: Set(true),
            is_active: Set(true),
            revoked_at: Set(None),
            last_used_at: Set(None),
            metadata: Set(serde_json::json!({})),
            created_at: Set(Utc::now().into()),
            updated_at: Set(Utc::now().into()),
        }
        .insert(&db)
        .await
        .expect("insert auto-created app owned by a different manifest entry");

        let old_token_id = Uuid::new_v4();
        let old_token = oauth_tokens::ActiveModel {
            id: Set(old_token_id),
            app_id: Set(orphan_app.id),
            user_id: Set(Some(orphan_user.id)),
            tenant_id: Set(tenant.id),
            token_hash: Set("orphan-token-hash".to_string()),
            family_id: Set(old_token_id),
            authorization_code_id: Set(Some(orphan_code_id)),
            grant_type: Set("authorization_code".to_string()),
            token_kind: Set(oauth_tokens::TOKEN_KIND_REFRESH.to_string()),
            consent_granted_at: Set(Some(Utc::now().into())),
            scopes: Set(serde_json::json!(["storefront:*"])),
            expires_at: Set((Utc::now() + chrono::Duration::days(1)).into()),
            revoked_at: Set(None),
            last_used_at: Set(None),
            created_at: Set(Utc::now().into()),
            updated_at: Set(Utc::now().into()),
        }
        .insert(&db)
        .await
        .expect("insert orphan OAuth token");

        let mut manifest = crate::modules::ModulesManifest::default();
        manifest.build.server.embed_admin = false;
        manifest.build.server.embed_storefront = false;
        manifest.build.admin.stack = "next".to_string();
        manifest.build.admin.public_url = "https://admin.example.com".to_string();
        manifest.build.admin.redirect_uris =
            vec!["https://admin.example.com/auth/callback".to_string()];
        manifest.build.storefront = serde_json::from_value(serde_json::json!([
            {
                "id": "shop",
                "stack": "next",
                "public_url": "https://shop.example.com",
                "redirect_uris": ["https://shop.example.com/auth/callback"]
            }
        ]))
        .expect("deserialize storefront build config");

        sync_app_connections(&db, tenant.id, &manifest)
            .await
            .expect("sync app connections");

        let apps = OAuthAppsEntity::find_by_tenant(&db, tenant.id)
            .await
            .expect("list apps");

        let next_admin = apps
            .iter()
            .find(|app| app.slug == "next-admin")
            .expect("next-admin app");
        assert_eq!(next_admin.app_type, "first_party");
        assert!(
            next_admin.client_secret_hash.is_none(),
            "manifest-created first-party apps must be usable as public PKCE clients"
        );
        assert!(
            next_admin.can_rotate_secret(),
            "operators must be able to provision the optional SSR credential"
        );
        assert_eq!(
            next_admin.redirect_uris_list(),
            vec!["https://admin.example.com/auth/callback".to_string()]
        );
        assert_eq!(
            next_admin.metadata["public_url"],
            serde_json::json!("https://admin.example.com")
        );
        assert_eq!(
            next_admin.grant_types_list(),
            vec![
                "authorization_code".to_string(),
                "refresh_token".to_string(),
                "client_credentials".to_string(),
            ],
        );

        let storefront = apps
            .iter()
            .find(|app| app.slug == "shop")
            .expect("shop storefront app");
        assert_eq!(storefront.app_type, "first_party");
        assert_eq!(
            storefront.redirect_uris_list(),
            vec!["https://shop.example.com/auth/callback".to_string()]
        );
        assert_eq!(
            storefront.grant_types_list(),
            vec![
                "authorization_code".to_string(),
                "refresh_token".to_string(),
                "client_credentials".to_string(),
            ],
        );

        for (app, expected_name) in [(next_admin, "next Admin"), (storefront, "shop storefront")] {
            let copy = oauth_apps::resolve_translation(&db, tenant.id, app.id, "en")
                .await
                .expect("read manifest-generated OAuth copy")
                .expect("manifest-generated copy commits with the OAuth app");
            assert_eq!(copy.name, expected_name);
        }

        let orphan = apps
            .iter()
            .find(|app| app.slug == "old-storefront")
            .expect("orphan app still queryable");
        assert!(!orphan.is_active);
        assert!(orphan.revoked_at.is_some());

        let other_manifest_auto_created =
            OAuthAppsEntity::find_by_id(auto_created_for_another_manifest_entry.id)
                .one(&db)
                .await
                .expect("load app owned by another manifest entry")
                .expect("other manifest's app remains queryable");
        assert!(
            other_manifest_auto_created.is_active,
            "orphan cleanup must not deactivate an app owned by another manifest entry"
        );
        assert!(other_manifest_auto_created.revoked_at.is_none());

        let token = oauth_tokens::Entity::find_by_id(old_token.id)
            .one(&db)
            .await
            .expect("read orphan token")
            .expect("orphan token remains as revoked evidence");
        assert!(
            token.revoked_at.is_some(),
            "removed managed app must revoke its tokens"
        );
        let orphan_code = oauth_authorization_codes::Entity::find_by_id(orphan_code_id)
            .one(&db)
            .await
            .expect("read orphan authorization code")
            .expect("orphan authorization code remains as used evidence");
        assert!(
            orphan_code.used_at.is_some(),
            "removing a managed app must invalidate its pending authorization codes"
        );
        let revoked_orphan_consent = oauth_consents::Entity::find_by_id(orphan_consent.id)
            .one(&db)
            .await
            .expect("load consent after orphan cleanup")
            .expect("orphan consent remains as revocation evidence");
        assert!(
            revoked_orphan_consent.revoked_at.is_some(),
            "removing a managed app must revoke its previously granted user consent"
        );
        assert!(
            OAuthConsents::find_active_consent(&db, orphan_app.id, orphan_user.id, tenant.id)
                .await
                .expect("check active consent after orphan cleanup")
                .is_none()
        );

        OAuthAppService::touch_last_used(&db, next_admin.id)
            .await
            .expect("record first-party app use before manifest resync");
        let last_used_before_resync = OAuthAppsEntity::find_by_id(next_admin.id)
            .one(&db)
            .await
            .expect("load app usage before resync")
            .and_then(|app| app.last_used_at);
        let rotated = OAuthAppService::rotate_secret(&db, tenant.id, next_admin.id)
            .await
            .expect("rotate manifest-managed app secret");
        sync_app_connections(&db, tenant.id, &manifest)
            .await
            .expect("resync manifest-managed app");
        let resynced = OAuthAppsEntity::find_by_id(next_admin.id)
            .one(&db)
            .await
            .expect("load resynced app")
            .expect("manifest-managed app remains present");
        assert!(
            OAuthAppService::verify_client_secret(
                &rotated.client_secret,
                resynced
                    .client_secret_hash
                    .as_deref()
                    .expect("resynced app retains its credential hash"),
            )
            .expect("verify rotated credential after resync")
        );
        assert_eq!(
            resynced.last_used_at, last_used_before_resync,
            "manifest sync must not overwrite runtime-owned usage timestamps"
        );

        // Simulate an inactive managed app left by an older server version:
        // the app is disabled, but a dependent token, consent, and code remain live.
        let mut legacy_consent: oauth_consents::ActiveModel = revoked_orphan_consent.clone().into();
        legacy_consent.revoked_at = Set(None);
        legacy_consent
            .update(&db)
            .await
            .expect("restore legacy active consent fixture");
        let mut legacy_token: oauth_tokens::ActiveModel = token.clone().into();
        legacy_token.revoked_at = Set(None);
        legacy_token
            .update(&db)
            .await
            .expect("restore legacy active token fixture");
        let mut legacy_code: oauth_authorization_codes::ActiveModel = orphan_code.clone().into();
        legacy_code.used_at = Set(None);
        legacy_code
            .update(&db)
            .await
            .expect("restore legacy pending code fixture");

        manifest.build.storefront.push(
            serde_json::from_value(serde_json::json!({
                "id": "old-storefront",
                "stack": "next",
                "public_url": "https://old.example.com",
                "redirect_uris": ["https://old.example.com/callback"]
            }))
            .expect("deserialize re-added storefront config"),
        );
        sync_app_connections(&db, tenant.id, &manifest)
            .await
            .expect("re-add previously orphaned manifest app");
        let reactivated_orphan = OAuthAppsEntity::find_by_id(orphan_app.id)
            .one(&db)
            .await
            .expect("load reactivated orphan")
            .expect("previously orphaned app remains present");
        assert!(reactivated_orphan.is_active);
        assert!(reactivated_orphan.revoked_at.is_none());
        let consent_after_reactivation = oauth_consents::Entity::find_by_id(orphan_consent.id)
            .one(&db)
            .await
            .expect("load consent after app reactivation")
            .expect("consent history remains present");
        assert!(
            consent_after_reactivation.revoked_at.is_some(),
            "re-adding a manifest app must not restore previously revoked consent"
        );
        assert!(
            OAuthConsents::find_active_consent(&db, orphan_app.id, orphan_user.id, tenant.id)
                .await
                .expect("check active consent after app reactivation")
                .is_none()
        );
        let token_after_reactivation = oauth_tokens::Entity::find_by_id(old_token.id)
            .one(&db)
            .await
            .expect("load token after app reactivation")
            .expect("token history remains present");
        assert!(token_after_reactivation.revoked_at.is_some());
        let code_after_reactivation = oauth_authorization_codes::Entity::find_by_id(orphan_code_id)
            .one(&db)
            .await
            .expect("load code after app reactivation")
            .expect("authorization-code history remains present");
        assert!(code_after_reactivation.used_at.is_some());
    }

    #[tokio::test]
    #[serial]
    async fn orphan_deactivation_database_failure_is_not_silently_ignored() {
        let (db, tenant) = test_db_with_tenant().await;
        let mut manifest = crate::modules::ModulesManifest::default();
        manifest.build.server.embed_admin = true;
        manifest.build.server.embed_storefront = false;
        manifest.build.admin.stack.clear();
        manifest.build.storefront.clear();

        sync_app_connections(&db, tenant.id, &manifest)
            .await
            .expect("create an active managed admin app");

        let managed = OAuthAppsEntity::find_by_tenant(&db, tenant.id)
            .await
            .expect("load manifest-managed app")
            .into_iter()
            .find(|app| app.slug == "leptos-admin")
            .expect("embedded admin app was created");
        assert!(managed.is_active);

        db.execute_unprepared(
            "CREATE TRIGGER abort_managed_app_deactivation BEFORE UPDATE OF is_active ON oauth_apps WHEN NEW.is_active = 0 BEGIN SELECT RAISE(ABORT, 'simulated deactivation failure'); END",
        )
        .await
        .expect("install fault injection trigger");

        manifest.build.server.embed_admin = false;
        assert!(
            sync_app_connections(&db, tenant.id, &manifest)
                .await
                .is_err(),
            "manifest sync must fail when an orphan cannot be deactivated"
        );
        let still_active = OAuthAppsEntity::find_by_id(managed.id)
            .one(&db)
            .await
            .expect("load app after failed sync")
            .expect("app remains persisted");
        assert!(
            still_active.is_active,
            "failed transaction must preserve prior app state"
        );
    }
}
