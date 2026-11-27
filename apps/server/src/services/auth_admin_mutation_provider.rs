use async_trait::async_trait;
use rustok_api::{
    AuthPrincipalKind, Permission, has_any_effective_permission, has_effective_permission,
};
use rustok_auth::{
    AuthAdminMutationContext, AuthAdminMutationError, AuthorizedOAuthAppRecord,
    CreateOAuthAppCommand, OAuthAdminPort, OAuthAppMutationRecord, OAuthAppSecretResult,
    UpdateOAuthAppCommand, UserMutationRecord,
};
use rustok_cache::CacheService;
use rustok_core::UserRole;
use rustok_outbox::TransactionalEventBus;
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, QuerySelect, TransactionTrait,
};
use std::str::FromStr;
use uuid::Uuid;

use crate::models::{oauth_apps, oauth_consents, oauth_tokens, tenants, users};
use crate::services::cache_runtime::ensure_cache_service;
use crate::services::event_bus::transactional_event_bus_from_context;
use crate::services::oauth_app::{self, OAuthAppService};
use crate::services::rbac_request_scope::{bind_admin_context, credential_for_admin, permissions_for};
use crate::services::auth_lifecycle::AuthLifecycleService;
use crate::services::rbac_service::RbacService;
use crate::services::server_runtime_context::ServerRuntimeContext;

mod super_admin_guard;
mod user_admin;

struct LockedOAuthAdminAuthority {
    target: Option<oauth_apps::Model>,
    actor: Option<users::Model>,
    permissions: Vec<Permission>,
    deadline: Option<chrono::DateTime<chrono::Utc>>,
}
impl LockedOAuthAdminAuthority {
    fn target(&self) -> Result<oauth_apps::Model, AuthAdminMutationError> {
        self.target.clone().ok_or_else(|| AuthAdminMutationError::NotFound("oauth app".to_string()))
    }
}
fn require_delegation_ceiling(authority: &[Permission], requested: &[String]) -> Result<(), AuthAdminMutationError> {
    for raw in requested {
        let permission = Permission::from_str(raw.trim()).map_err(|error| {
            AuthAdminMutationError::Validation(format!("invalid delegated permission `{raw}`: {error}"))
        })?;
        if !has_effective_permission(authority, &permission) {
            return Err(AuthAdminMutationError::Forbidden(format!("cannot delegate permission outside current authority: {permission}")));
        }
    }
    Ok(())
}

const MAX_AUTH_ADMIN_LIST_LIMIT: u64 = 100;

fn clamp_auth_admin_list_limit(limit: u64) -> u64 {
    limit.clamp(1, MAX_AUTH_ADMIN_LIST_LIMIT)
}

/// Consent is the user personally delegating authority to an application.
/// A delegated (`authorization_code`) or service (`client_credentials`)
/// token must never grant, revoke, or enumerate consent as the user —
/// otherwise any low-scope application could pre-seed or approve consent
/// for other applications. Fail closed unless the caller is a direct
/// user session.
fn require_direct_user_context(
    context: &AuthAdminMutationContext,
) -> Result<(), AuthAdminMutationError> {
    match context.principal_kind {
        Some(AuthPrincipalKind::DirectUser) => Ok(()),
        _ => Err(AuthAdminMutationError::Forbidden(
            "oauth consent operations require a direct user session".to_string(),
        )),
    }
}

fn internal_admin_error<E>(error: E) -> AuthAdminMutationError
where
    E: std::fmt::Display,
{
    tracing::error!(
        error = %error,
        "Auth administration operation failed"
    );
    // The detail goes to the log above only; the returned error is
    // sealed so no consumer of the port can leak it to a client.
    AuthAdminMutationError::Internal("Auth administration operation failed".to_string())
}

pub(crate) async fn build_oauth_app_record<C>(
    db: &C,
    context: &AuthAdminMutationContext,
    app: oauth_apps::Model,
) -> Result<OAuthAppMutationRecord, AuthAdminMutationError>
where
    C: ConnectionTrait,
{
    let locale = context.locale.as_deref().ok_or_else(|| {
        AuthAdminMutationError::Validation(
            "auth administration requires a host-resolved effective locale".to_string(),
        )
    })?;
    let locale = oauth_apps::normalize_runtime_copy_locale(locale).map_err(|_| {
        AuthAdminMutationError::Validation(
            "auth administration requires a valid effective locale other than und".to_string(),
        )
    })?;
    let app = oauth_apps::hydrate_exact_translation(db, app, locale.as_str())
        .await
        .map_err(internal_admin_error)?;
    let active_token_count = oauth_tokens::Entity::find()
        .filter(oauth_tokens::Column::AppId.eq(app.id))
        .filter(oauth_tokens::Column::TenantId.eq(context.tenant_id))
        .filter(oauth_tokens::Column::TokenKind.eq(oauth_tokens::TOKEN_KIND_REFRESH))
        .filter(oauth_tokens::Column::RevokedAt.is_null())
        .filter(oauth_tokens::Column::ExpiresAt.gt(chrono::Utc::now()))
        .count(db)
        .await
        .map_err(internal_admin_error)?;

    let redirect_uris = app.redirect_uris_list();
    let scopes = app.scopes_list();
    let grant_types = app.grant_types_list();
    let granted_permissions = app.granted_permissions_list();
    let managed_by_manifest = app.managed_by_manifest();
    let is_active = app.is_active();
    let can_edit = app.can_edit();
    let can_rotate_secret = app.can_rotate_secret();
    let can_revoke = app.can_revoke();

    Ok(OAuthAppMutationRecord {
        id: app.id,
        name: app.name,
        slug: app.slug,
        description: app.description,
        icon_url: app.icon_url,
        app_type: app.app_type,
        client_id: app.client_id,
        redirect_uris,
        scopes,
        grant_types,
        granted_permissions,
        manifest_ref: app.manifest_ref,
        auto_created: app.auto_created,
        managed_by_manifest,
        is_active,
        can_edit,
        can_rotate_secret,
        can_revoke,
        active_token_count: i64::try_from(active_token_count).unwrap_or(i64::MAX),
        last_used_at: app.last_used_at.map(Into::into),
        created_at: app.created_at.into(),
    })
}

#[derive(Clone)]
pub struct ServerAuthAdminMutationProvider {
    db: DatabaseConnection,
    auth_config: crate::auth::AuthConfig,
    cache_service: CacheService,
    event_bus: TransactionalEventBus,
    invite_registry: Option<rustok_core::ModuleRegistry>,
}

impl ServerAuthAdminMutationProvider {
    pub fn new(runtime_ctx: &ServerRuntimeContext, auth_config: crate::auth::AuthConfig) -> Self {
        Self {
            db: runtime_ctx.db_clone(),
            auth_config,
            cache_service: ensure_cache_service(runtime_ctx),
            event_bus: transactional_event_bus_from_context(runtime_ctx),
            invite_registry: runtime_ctx.shared_get::<rustok_core::ModuleRegistry>(),
        }
    }

    pub(crate) fn with_invite_module_registry(
        mut self,
        registry: rustok_core::ModuleRegistry,
    ) -> Self {
        self.invite_registry = Some(registry);
        self
    }

    fn request_permissions(
        &self,
        context: &AuthAdminMutationContext,
    ) -> Result<Vec<Permission>, AuthAdminMutationError> {
        credential_for_admin(context)?;
        permissions_for(&context.tenant_id, &context.actor_id).ok_or_else(|| {
            AuthAdminMutationError::Forbidden(
                "auth administration requires a request-bound effective permission snapshot"
                    .to_string(),
            )
        })
    }

    fn effective_locale(
        &self,
        context: &AuthAdminMutationContext,
    ) -> Result<String, AuthAdminMutationError> {
        let locale = context.locale.as_deref().ok_or_else(|| {
            AuthAdminMutationError::Validation(
                "auth administration requires a host-resolved effective locale".to_string(),
            )
        })?;
        oauth_apps::normalize_runtime_copy_locale(locale).map_err(|_| {
            AuthAdminMutationError::Validation(
                "auth administration requires a valid effective locale other than `und`"
                    .to_string(),
            )
        })
    }

    async fn authorize_user(
        &self,
        context: &AuthAdminMutationContext,
        permissions: &[Permission],
        message: &str,
    ) -> Result<(), AuthAdminMutationError> {
        let actor_permissions = self.request_permissions(context)?;
        if has_any_effective_permission(&actor_permissions, permissions) {
            Ok(())
        } else {
            Err(AuthAdminMutationError::Forbidden(message.to_string()))
        }
    }

    fn user_record(
        user: users::Model,
        role: String,
        tenant_name: Option<String>,
    ) -> UserMutationRecord {
        UserMutationRecord {
            id: user.id,
            email: user.email,
            name: user.name,
            role,
            status: user.status.to_string(),
            created_at: user.created_at.with_timezone(&chrono::Utc),
            tenant_name,
            tenant_id: user.tenant_id,
            metadata: user.metadata,
        }
    }

    async fn tenant_name<C>(
        &self,
        db: &C,
        tenant_id: Uuid,
    ) -> Result<Option<String>, AuthAdminMutationError>
    where
        C: sea_orm::ConnectionTrait,
    {
        <tenants::Entity as sea_orm::EntityTrait>::find_by_id(tenant_id)
            .one(db)
            .await
            .map_err(internal_admin_error)
            .map(|tenant| tenant.map(|value| value.name))
    }

    async fn authorize(
        &self,
        context: &AuthAdminMutationContext,
    ) -> Result<(), AuthAdminMutationError> {
        let actor_permissions = self.request_permissions(context)?;
        if has_effective_permission(&actor_permissions, &Permission::SETTINGS_MANAGE) {
            Ok(())
        } else {
            Err(AuthAdminMutationError::Forbidden(
                "settings:manage required for OAuth application administration".to_string(),
            ))
        }
    }

    async fn authorize_delegated_permissions(
        &self,
        context: &AuthAdminMutationContext,
        requested_permissions: &[String],
    ) -> Result<(), AuthAdminMutationError> {
        require_delegation_ceiling(&self.request_permissions(context)?, requested_permissions)
    }

    // Admit the credential clock and initialize policy before any owner
    // transaction or row lock, including RBAC-owned transactions.
    async fn prepare_admin_authority(&self, policy_required: bool) -> Result<(), AuthAdminMutationError> {
        rustok_auth::application_clock::admit_now(&self.db).await.map_err(internal_admin_error)?;
        if policy_required {
            crate::services::rbac_module_availability::prepare_invite_module_policy(
                &self.db, self.invite_registry.as_ref(),
            ).await.map_err(internal_admin_error)?;
        }
        Ok(())
    }

    async fn prepare_admin_transaction(&self, policy_required: bool) -> Result<sea_orm::DatabaseTransaction, AuthAdminMutationError> {
        self.prepare_admin_authority(policy_required).await?;
        self.db.begin().await.map_err(internal_admin_error)
    }

    /// One authority decision on the owner transaction, shared by all OAuth
    /// admin writes. No cache lookup or independent pool checkout after locks.
    async fn lock_oauth_admin_authority(
        &self, tx: &sea_orm::DatabaseTransaction, context: &AuthAdminMutationContext,
        app_id: Option<Uuid>, settings_required: bool,
    ) -> Result<LockedOAuthAdminAuthority, AuthAdminMutationError> {
        let evidence = credential_for_admin(context)?;
        if !settings_required {
            require_direct_user_context(context)?;
            if !evidence.as_ref().is_some_and(|e| e.principal_kind == AuthPrincipalKind::DirectUser) {
                return Err(AuthAdminMutationError::Unauthorized);
            }
        }
        let service = evidence.as_ref().is_some_and(|e| e.principal_kind == AuthPrincipalKind::Service);
        if !service && context.principal_kind == Some(AuthPrincipalKind::Service) {
            return Err(AuthAdminMutationError::Unauthorized);
        }
        let (target, issuer) = self.lock_admin_oauth_apps(tx, context, app_id).await?;
        let (actor, deadline, mut permissions) = if service {
            let e = evidence.as_ref().ok_or(AuthAdminMutationError::Unauthorized)?;
            let registration = e.oauth.as_ref().ok_or(AuthAdminMutationError::Unauthorized)?;
            let app = issuer.as_ref().ok_or(AuthAdminMutationError::Unauthorized)?;
            let expires = self.lock_admin_oauth_registration(tx, context, None, app, registration).await?;
            let grants = app.parsed_granted_permissions().map_err(internal_admin_error)?;
            (None, Some(expires.min(e.expires_at)), grants)
        } else {
            let actor = AuthLifecycleService::lock_user_for_mutation(tx, context.tenant_id, context.actor_id)
                .await.map_err(internal_admin_error)?
                .filter(|actor| actor.is_active()).ok_or(AuthAdminMutationError::Unauthorized)?;
            let deadline = self.lock_user_admin_credential(tx, context, &actor, issuer.as_ref()).await?;
            (Some(actor), deadline, Vec::new())
        };
        if settings_required {
            let (_, availability) = crate::services::rbac_module_availability::lock_invite_module_policy(
                &self.db, tx, self.invite_registry.as_ref(), context.tenant_id,
            ).await.map_err(internal_admin_error)?;
            if !service {
                rustok_rbac::lock_actor_role_grants_on(tx, context.tenant_id, context.actor_id)
                    .await.map_err(internal_admin_error)?;
                permissions = RbacService::get_user_permissions_authoritative(tx, &context.tenant_id, &context.actor_id)
                    .await.map_err(internal_admin_error)?;
            }
            if let Some(availability) = availability {
                permissions = availability.active_permissions(&permissions);
            }
            permissions = user_admin::scope_live_admin_permissions(context, permissions)?;
            if !has_effective_permission(&permissions, &Permission::SETTINGS_MANAGE)
                || !has_effective_permission(&self.request_permissions(context)?, &Permission::SETTINGS_MANAGE)
            {
                return Err(AuthAdminMutationError::Forbidden("current settings:manage required for OAuth administration".to_string()));
            }
        }
        self.ensure_user_admin_credential_deadline(tx, deadline).await?;
        Ok(LockedOAuthAdminAuthority { target, actor, permissions, deadline })
    }

    pub(crate) async fn commit_prepared_oauth_secret_rotation(
        &self, context: &AuthAdminMutationContext, app_id: Uuid, prepared: oauth_app::PreparedOAuthSecret,
    ) -> Result<OAuthAppSecretResult, AuthAdminMutationError> {
        let tx = self.prepare_admin_transaction(true).await?;
        let authority = self.lock_oauth_admin_authority(&tx, context, Some(app_id), true).await?;
        let app = authority.target()?;
        let grants = app.parsed_granted_permissions().map_err(internal_admin_error)?
            .into_iter().map(|permission| permission.to_string()).collect::<Vec<_>>();
        self.authorize_delegated_permissions(context, &grants).await?;
        require_delegation_ceiling(&authority.permissions, &grants)?;
        let result = OAuthAppService::rotate_locked_secret(&tx, app, prepared).await.map_err(map_service_error)?;
        let record = build_oauth_app_record(&tx, context, result.app).await?;
        self.ensure_user_admin_credential_deadline(&tx, authority.deadline).await?;
        tx.commit().await.map_err(internal_admin_error)?;
        Ok(OAuthAppSecretResult { app: record, client_secret: result.client_secret })
    }

    async fn record(
        &self,
        context: &AuthAdminMutationContext,
        app: oauth_apps::Model,
    ) -> Result<OAuthAppMutationRecord, AuthAdminMutationError> {
        build_oauth_app_record(&self.db, context, app).await
    }
}

/// Host implementation of the RBAC owner's transaction admission port.
/// Use the same direct credential fence as Auth/OAuth, but let RBAC own
/// its write transaction and exact destination role locks.
impl rustok_auth::AuthAdminCredentialResolver for ServerAuthAdminMutationProvider {
    fn bind(
        &self,
        auth: rustok_auth::AdminCredentialRequest<'_>,
        principal_kind: AuthPrincipalKind,
        locale: Option<String>,
    ) -> Result<AuthAdminMutationContext, AuthAdminMutationError> {
        let context = bind_admin_context(auth.tenant_id, auth.user_id, principal_kind, locale)?;
        use rustok_auth::CredentialAuthority;
        let matches = match &context.credential_authority {
            CredentialAuthority::DirectSession { session_id, .. } =>
                auth.session_id == *session_id && auth.client_id.is_none() && auth.grant_type == "direct",
            CredentialAuthority::DelegatedOAuth { registration, .. } =>
                auth.client_id == Some(registration.client_id) && auth.grant_type == "authorization_code"
                    && auth.session_id.is_nil() && auth.scopes == registration.scopes.as_slice(),
            CredentialAuthority::ServiceOAuth { registration, .. } =>
                auth.user_id == registration.app_id && auth.client_id == Some(registration.client_id)
                    && auth.grant_type == "client_credentials" && auth.session_id.is_nil() && auth.scopes == registration.scopes.as_slice(),
            CredentialAuthority::TrustedInternal => false,
        };
        if !matches {
            return Err(AuthAdminMutationError::Unauthorized);
        }
        Ok(context)
    }
}

#[async_trait]
impl rustok_rbac::graphql::RbacMutationAuthority for ServerAuthAdminMutationProvider {
    async fn prepare(&self) -> Result<(), rustok_rbac::graphql::RbacGraphqlRoleWriteError> {
        self.prepare_admin_authority(true).await.map_err(map_rbac_authority_error)
    }

    async fn lock(
        &self, tx: &sea_orm::DatabaseTransaction, tenant_id: Uuid, actor_id: Uuid,
        target_role_id: Option<Uuid>, required: &[Permission],
    ) -> Result<rustok_rbac::graphql::RbacLockedMutationAuthority, rustok_rbac::graphql::RbacGraphqlRoleWriteError> {
        self.lock_rbac_control_plane(tx, tenant_id, actor_id, target_role_id, required)
            .await.map_err(map_rbac_authority_error)
    }

    async fn ensure_deadline(
        &self, tx: &sea_orm::DatabaseTransaction, authority: &rustok_rbac::graphql::RbacLockedMutationAuthority,
    ) -> Result<(), rustok_rbac::graphql::RbacGraphqlRoleWriteError> {
        self.ensure_user_admin_credential_deadline(tx, Some(authority.deadline))
            .await.map_err(map_rbac_authority_error)
    }
}

fn map_rbac_authority_error(error: AuthAdminMutationError) -> rustok_rbac::graphql::RbacGraphqlRoleWriteError {
    use rustok_rbac::graphql::RbacGraphqlRoleWriteError as E;
    match error {
        AuthAdminMutationError::Unauthorized => E::Forbidden("live direct credential required".into()),
        AuthAdminMutationError::Forbidden(message) => E::Forbidden(message),
        AuthAdminMutationError::NotFound(message) => E::NotFound(message),
        _ => E::Internal("RBAC transaction authority unavailable".into()),
    }
}

impl ServerAuthAdminMutationProvider {
    async fn lock_rbac_control_plane(
        &self, tx: &sea_orm::DatabaseTransaction, tenant_id: Uuid, actor_id: Uuid,
        target_role_id: Option<Uuid>, required: &[Permission],
    ) -> Result<rustok_rbac::graphql::RbacLockedMutationAuthority, AuthAdminMutationError> {
        let context = bind_admin_context(tenant_id, actor_id, AuthPrincipalKind::DirectUser, None)?;
        let evidence = credential_for_admin(&context)?.ok_or(AuthAdminMutationError::Unauthorized)?;
        let request = self.request_permissions(&context)?;
        if !required.iter().any(|p| has_effective_permission(&request, p)) {
            return Err(AuthAdminMutationError::Forbidden("RBAC request authority required".into()));
        }
        // Shared tenant fence precedes users, credentials, policy and roles.
        self.lock_admin_oauth_apps(tx, &context, None).await?;
        let actor = AuthLifecycleService::lock_user_for_mutation(tx, tenant_id, actor_id)
            .await.map_err(internal_admin_error)?
            .filter(|user| user.is_active()).ok_or(AuthAdminMutationError::Unauthorized)?;
        let deadline = self.lock_user_admin_credential(tx, &context, &actor, None)
            .await?.ok_or(AuthAdminMutationError::Unauthorized)?.min(evidence.expires_at);
        let (_, availability) = crate::services::rbac_module_availability::lock_invite_module_policy(
            &self.db, tx, self.invite_registry.as_ref(), tenant_id,
        ).await.map_err(internal_admin_error)?;
        if !rustok_rbac::lock_role_definition_authority_on(tx, tenant_id, actor_id, target_role_id)
            .await.map_err(internal_admin_error)?
        {
            return Err(AuthAdminMutationError::NotFound("role".into()));
        }
        let mut permissions = RbacService::get_user_permissions_authoritative(tx, &tenant_id, &actor_id)
            .await.map_err(internal_admin_error)?;
        if let Some(snapshot) = &availability {
            permissions = snapshot.active_permissions(&permissions);
        }
        if !required.iter().any(|p| has_effective_permission(&permissions, p)) {
            return Err(AuthAdminMutationError::Forbidden("current RBAC management permission required".into()));
        }
        self.ensure_user_admin_credential_deadline(tx, Some(deadline)).await?;
        Ok(rustok_rbac::graphql::RbacLockedMutationAuthority { permissions, availability, deadline })
    }
}

fn map_service_error(error: crate::error::Error) -> AuthAdminMutationError {
    match error {
        crate::error::Error::NotFound => AuthAdminMutationError::NotFound("oauth app".to_string()),
        crate::error::Error::BadRequest(message) => AuthAdminMutationError::Validation(message),
        crate::error::Error::Conflict(message) => AuthAdminMutationError::Conflict(message),
        crate::error::Error::Unauthorized(_) => AuthAdminMutationError::Unauthorized,
        other => internal_admin_error(other),
    }
}

#[async_trait]
impl OAuthAdminPort for ServerAuthAdminMutationProvider {
    async fn list_oauth_apps(
        &self,
        context: &AuthAdminMutationContext,
        app_type: Option<String>,
        limit: u64,
    ) -> Result<Vec<OAuthAppMutationRecord>, AuthAdminMutationError> {
        self.authorize(context).await?;
        let mut query = oauth_apps::Entity::find()
            .filter(oauth_apps::Column::TenantId.eq(context.tenant_id))
            .filter(oauth_apps::Column::IsActive.eq(true))
            .filter(oauth_apps::Column::RevokedAt.is_null())
            .order_by_desc(oauth_apps::Column::CreatedAt)
            .limit(clamp_auth_admin_list_limit(limit));
        if let Some(app_type) = app_type {
            query = query.filter(oauth_apps::Column::AppType.eq(app_type));
        }
        let apps = query.all(&self.db).await.map_err(internal_admin_error)?;
        let mut records = Vec::with_capacity(apps.len());
        for app in apps {
            records.push(self.record(context, app).await?);
        }
        Ok(records)
    }

    async fn get_oauth_app(
        &self,
        context: &AuthAdminMutationContext,
        app_id: Uuid,
    ) -> Result<Option<OAuthAppMutationRecord>, AuthAdminMutationError> {
        self.authorize(context).await?;
        let app = oauth_apps::Entity::find_by_id(app_id)
            .filter(oauth_apps::Column::TenantId.eq(context.tenant_id))
            .one(&self.db)
            .await
            .map_err(internal_admin_error)?;
        match app {
            Some(app) => Ok(Some(self.record(context, app).await?)),
            None => Ok(None),
        }
    }

    async fn list_authorized_oauth_apps(
        &self,
        context: &AuthAdminMutationContext,
        limit: u64,
    ) -> Result<Vec<AuthorizedOAuthAppRecord>, AuthAdminMutationError> {
        require_direct_user_context(context)?;
        credential_for_admin(context)?.ok_or(AuthAdminMutationError::Unauthorized)?;
        let consents = oauth_consents::Entity::find()
            .filter(oauth_consents::Column::UserId.eq(context.actor_id))
            .filter(oauth_consents::Column::TenantId.eq(context.tenant_id))
            .filter(oauth_consents::Column::RevokedAt.is_null())
            .order_by_desc(oauth_consents::Column::GrantedAt)
            .limit(clamp_auth_admin_list_limit(limit))
            .find_also_related(oauth_apps::Entity)
            .all(&self.db)
            .await
            .map_err(internal_admin_error)?;
        let mut records = Vec::with_capacity(consents.len());
        for (consent, app) in consents {
            if let Some(app) = app.filter(|app| app.is_active()) {
                records.push(AuthorizedOAuthAppRecord {
                    app: self.record(context, app).await?,
                    scopes: consent.scopes_list().map_err(internal_admin_error)?,
                    granted_at: consent.granted_at.into(),
                });
            }
        }
        Ok(records)
    }

    async fn create_oauth_app(
        &self, context: &AuthAdminMutationContext, command: CreateOAuthAppCommand,
    ) -> Result<OAuthAppSecretResult, AuthAdminMutationError> {
        self.authorize(context).await?;
        self.authorize_delegated_permissions(context, &command.granted_permissions).await?;
        let locale = self.effective_locale(context)?;
        let requested = command.granted_permissions.clone();
        let prepared = OAuthAppService::prepare_app(context.tenant_id, oauth_app::CreateOAuthAppInput {
            name: command.name, slug: command.slug, description: command.description,
            app_type: command.app_type, icon_url: command.icon_url, redirect_uris: command.redirect_uris,
            scopes: command.scopes, grant_types: command.grant_types, granted_permissions: command.granted_permissions,
        }).await.map_err(map_service_error)?;
        let tx = self.prepare_admin_transaction(true).await?;
        let authority = self.lock_oauth_admin_authority(&tx, context, None, true).await?;
        require_delegation_ceiling(&authority.permissions, &requested)?;
        let result = oauth_apps::scope_runtime_copy_locale(
            locale, OAuthAppService::create_prepared_app_in_transaction(&tx, prepared),
        ).await.map_err(map_service_error)?;
        let app = build_oauth_app_record(&tx, context, result.app).await?;
        self.ensure_user_admin_credential_deadline(&tx, authority.deadline).await?;
        tx.commit().await.map_err(internal_admin_error)?;
        Ok(OAuthAppSecretResult { app, client_secret: result.client_secret })
    }

    async fn update_oauth_app(
        &self, context: &AuthAdminMutationContext, command: UpdateOAuthAppCommand,
    ) -> Result<OAuthAppMutationRecord, AuthAdminMutationError> {
        self.authorize(context).await?;
        self.authorize_delegated_permissions(context, &command.granted_permissions).await?;
        let locale = self.effective_locale(context)?;
        let tx = self.prepare_admin_transaction(true).await?;
        let authority = self.lock_oauth_admin_authority(&tx, context, Some(command.id), true).await?;
        require_delegation_ceiling(&authority.permissions, &command.granted_permissions)?;
        let app = oauth_apps::scope_runtime_copy_locale(
            locale, OAuthAppService::update_locked_app(&tx, authority.target()?, oauth_app::UpdateOAuthAppInput {
                name: command.name, description: command.description, icon_url: command.icon_url,
                redirect_uris: command.redirect_uris, scopes: command.scopes,
                grant_types: command.grant_types, granted_permissions: command.granted_permissions,
            }),
        ).await.map_err(map_service_error)?;
        let record = build_oauth_app_record(&tx, context, app).await?;
        self.ensure_user_admin_credential_deadline(&tx, authority.deadline).await?;
        tx.commit().await.map_err(internal_admin_error)?;
        Ok(record)
    }

    async fn rotate_oauth_app_secret(
        &self, context: &AuthAdminMutationContext, app_id: Uuid,
    ) -> Result<OAuthAppSecretResult, AuthAdminMutationError> {
        self.authorize(context).await?;
        let app = oauth_apps::Entity::find_by_id(app_id)
            .filter(oauth_apps::Column::TenantId.eq(context.tenant_id))
            .one(&self.db).await.map_err(internal_admin_error)?
            .ok_or_else(|| AuthAdminMutationError::NotFound("oauth app".to_string()))?;
        let grants = app.parsed_granted_permissions().map_err(internal_admin_error)?
            .into_iter().map(|permission| permission.to_string()).collect::<Vec<_>>();
        self.authorize_delegated_permissions(context, &grants).await?;
        let prepared = OAuthAppService::prepare_secret(&app).await.map_err(map_service_error)?;
        self.commit_prepared_oauth_secret_rotation(context, app_id, prepared).await
    }

    async fn revoke_oauth_app(
        &self, context: &AuthAdminMutationContext, app_id: Uuid,
    ) -> Result<OAuthAppMutationRecord, AuthAdminMutationError> {
        self.authorize(context).await?;
        let tx = self.prepare_admin_transaction(true).await?;
        let authority = self.lock_oauth_admin_authority(&tx, context, Some(app_id), true).await?;
        let revoked = OAuthAppService::revoke_locked_app(&tx, authority.target()?).await.map_err(map_service_error)?;
        let record = build_oauth_app_record(&tx, context, revoked).await?;
        self.ensure_user_admin_credential_deadline(&tx, authority.deadline).await?;
        tx.commit().await.map_err(internal_admin_error)?;
        Ok(record)
    }

    async fn grant_oauth_app_consent(
        &self, context: &AuthAdminMutationContext, app_id: Uuid, scopes: Vec<String>,
    ) -> Result<(), AuthAdminMutationError> {
        require_direct_user_context(context)?;
        let tx = self.prepare_admin_transaction(false).await?;
        let authority = self.lock_oauth_admin_authority(&tx, context, Some(app_id), false).await?;
        let actor = authority.actor.as_ref().ok_or(AuthAdminMutationError::Unauthorized)?;
        OAuthAppService::grant_locked_user_consent(&tx, &authority.target()?, actor, scopes)
            .await.map_err(map_service_error)?;
        self.ensure_user_admin_credential_deadline(&tx, authority.deadline).await?;
        tx.commit().await.map_err(internal_admin_error)?;
        Ok(())
    }

    async fn revoke_oauth_app_consent(
        &self, context: &AuthAdminMutationContext, app_id: Uuid,
    ) -> Result<(), AuthAdminMutationError> {
        require_direct_user_context(context)?;
        let tx = self.prepare_admin_transaction(false).await?;
        let authority = self.lock_oauth_admin_authority(&tx, context, Some(app_id), false).await?;
        OAuthAppService::revoke_locked_user_consent(&tx, app_id, context.actor_id, context.tenant_id)
            .await.map_err(map_service_error)?;
        self.ensure_user_admin_credential_deadline(&tx, authority.deadline).await?;
        tx.commit().await.map_err(internal_admin_error)?;
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::{
        ServerAuthAdminMutationProvider, clamp_auth_admin_list_limit, internal_admin_error,
        map_service_error,
    };
    use rustok_auth::AuthAdminMutationError;
    use rustok_core::UserRole;
    use serde_json::json;
    use uuid::Uuid;

    #[test]
    fn oauth_service_conflicts_remain_retryable_admin_conflicts() {
        let error = map_service_error(crate::error::Error::Conflict(
            "secret changed concurrently".to_string(),
        ));
        assert_eq!(
            error,
            AuthAdminMutationError::Conflict("secret changed concurrently".to_string())
        );
    }

    #[test]
    fn internal_admin_errors_are_redacted() {
        let error = internal_admin_error("database password leaked");
        assert!(matches!(
            error,
            AuthAdminMutationError::Internal(message)
                if message == "Auth administration operation failed"
        ));
    }

    #[test]
    fn auth_admin_list_limit_is_bounded() {
        assert_eq!(clamp_auth_admin_list_limit(0), 1);
        assert_eq!(clamp_auth_admin_list_limit(50), 50);
        assert_eq!(clamp_auth_admin_list_limit(100), 100);
        assert_eq!(clamp_auth_admin_list_limit(101), 100);
        assert_eq!(clamp_auth_admin_list_limit(u64::MAX), 100);
    }

    #[test]
    fn user_record_projection_is_post_commit_independent() {
        let user = crate::models::users::Model {
            id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            email: "admin@example.com".to_string(),
            password_hash: String::new(),
            name: Some("Admin".to_string()),
            status: rustok_core::UserStatus::Active,
            email_verified_at: None,
            last_login_at: None,
            failed_login_count: 0,
            locked_until: None,
            password_changed_at: None,
            recovery_revision: 0,
            metadata: json!({"team": "platform"}),
            created_at: chrono::Utc::now().into(),
            updated_at: chrono::Utc::now().into(),
        };
        let record = ServerAuthAdminMutationProvider::user_record(
            user.clone(),
            UserRole::Admin.to_string(),
            Some("Tenant".to_string()),
        );
        assert_eq!(record.id, user.id);
        assert_eq!(record.role, "admin");
        assert_eq!(record.status, "active");
        assert_eq!(record.tenant_name.as_deref(), Some("Tenant"));
        assert_eq!(record.metadata, user.metadata);
    }
}
