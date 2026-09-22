use async_trait::async_trait;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

use rustok_auth::{
    AcceptInviteRecord, AuthLifecycleContext, AuthLifecycleMutationError, AuthLifecyclePort,
    AuthSessionRecord, AuthTokenRecord, AuthUserBackfillDbReader, AuthUserBackfillReadPort,
    AuthUserBackfillReadRequest, AuthUserBackfillRecord, AuthUserRecord,
};

use crate::auth::{AuthConfig, encode_password_reset_token};
use crate::context::infer_user_role_from_permissions;
use crate::models::users;
use crate::services::auth_invite::InviteAcceptanceError;
use crate::services::auth_lifecycle::{AuthLifecycleError, AuthLifecycleService, AuthTokens};
use crate::services::email::{PasswordResetEmail, email_service_from_ctx, password_reset_url};
use crate::services::rbac_service::RbacService;
use crate::services::server_runtime_context::ServerRuntimeContext;

const DEFAULT_RESET_TOKEN_TTL_SECS: u64 = 15 * 60;

pub struct ServerAuthLifecycleProvider {
    runtime_ctx: ServerRuntimeContext,
    auth_config: AuthConfig,
}

impl ServerAuthLifecycleProvider {
    pub fn new(runtime_ctx: ServerRuntimeContext, auth_config: AuthConfig) -> Self {
        Self {
            runtime_ctx,
            auth_config,
        }
    }

    async fn permission_strings(
        &self,
        tenant_id: uuid::Uuid,
        user_id: uuid::Uuid,
    ) -> Result<Vec<String>, AuthLifecycleMutationError> {
        let permissions =
            RbacService::get_user_permissions(self.runtime_ctx.db(), &tenant_id, &user_id)
                .await
                .map_err(|err| AuthLifecycleMutationError::Internal(err.to_string()))?;
        let mut values = permissions
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        values.sort();
        values.dedup();
        Ok(values)
    }

    async fn token_record(
        &self,
        tenant_id: uuid::Uuid,
        user: users::Model,
        tokens: AuthTokens,
    ) -> Result<AuthTokenRecord, AuthLifecycleMutationError> {
        let permissions = self.permission_strings(tenant_id, user.id).await?;
        Ok(AuthTokenRecord {
            access_token: tokens.access_token,
            refresh_token: tokens.refresh_token,
            expires_in: tokens.expires_in,
            user: AuthUserRecord {
                id: user.id,
                email: user.email,
                name: user.name,
                role: tokens.effective_role,
                status: user.status.to_string(),
                permissions,
            },
        })
    }

    fn require_user_id(
        context: &AuthLifecycleContext,
    ) -> Result<uuid::Uuid, AuthLifecycleMutationError> {
        context
            .user_id
            .ok_or(AuthLifecycleMutationError::Unauthorized)
    }

    fn require_session_id(
        context: &AuthLifecycleContext,
    ) -> Result<uuid::Uuid, AuthLifecycleMutationError> {
        context
            .session_id
            .ok_or(AuthLifecycleMutationError::Unauthorized)
    }
}

#[async_trait]
impl AuthLifecyclePort for ServerAuthLifecycleProvider {
    async fn current_user(
        &self,
        context: &AuthLifecycleContext,
    ) -> Result<AuthUserRecord, AuthLifecycleMutationError> {
        let user_id = Self::require_user_id(context)?;
        let user = users::Entity::find_by_id(user_id)
            .filter(users::Column::TenantId.eq(context.tenant_id))
            .one(self.runtime_ctx.db())
            .await
            .map_err(|err| AuthLifecycleMutationError::Internal(err.to_string()))?
            .ok_or(AuthLifecycleMutationError::Unauthorized)?;

        Ok(AuthUserRecord {
            id: user.id,
            email: user.email,
            name: user.name,
            role: infer_user_role_from_permissions(&context.permissions),
            status: user.status.to_string(),
            permissions: permission_strings_from_context(context),
        })
    }

    async fn list_sessions(
        &self,
        context: &AuthLifecycleContext,
        limit: u64,
    ) -> Result<Vec<AuthSessionRecord>, AuthLifecycleMutationError> {
        let user_id = Self::require_user_id(context)?;
        AuthLifecycleService::list_sessions_runtime(
            &self.runtime_ctx,
            context.tenant_id,
            user_id,
            limit,
        )
        .await
        .map(|sessions| {
            sessions
                .into_iter()
                .map(|session| AuthSessionRecord {
                    id: session.id,
                    ip_address: session.ip_address,
                    user_agent: session.user_agent,
                    last_used_at: session
                        .last_used_at
                        .map(|value| value.with_timezone(&chrono::Utc)),
                    expires_at: session.expires_at.with_timezone(&chrono::Utc),
                    created_at: session.created_at.with_timezone(&chrono::Utc),
                })
                .collect()
        })
        .map_err(map_lifecycle_error)
    }

    async fn sign_in(
        &self,
        context: &AuthLifecycleContext,
        email: String,
        password: String,
    ) -> Result<AuthTokenRecord, AuthLifecycleMutationError> {
        let (user, tokens) = AuthLifecycleService::login_runtime(
            &self.runtime_ctx,
            &self.auth_config,
            context.tenant_id,
            &email,
            &password,
            None,
            None,
        )
        .await
        .map_err(map_lifecycle_error)?;
        self.token_record(context.tenant_id, user, tokens).await
    }

    async fn sign_up(
        &self,
        context: &AuthLifecycleContext,
        email: String,
        password: String,
        name: Option<String>,
    ) -> Result<AuthTokenRecord, AuthLifecycleMutationError> {
        let (user, tokens) = AuthLifecycleService::register_runtime(
            &self.runtime_ctx,
            &self.auth_config,
            context.tenant_id,
            &email,
            &password,
            name,
        )
        .await
        .map_err(map_lifecycle_error)?;
        self.token_record(context.tenant_id, user, tokens).await
    }

    async fn refresh_token(
        &self,
        context: &AuthLifecycleContext,
        refresh_token: String,
    ) -> Result<AuthTokenRecord, AuthLifecycleMutationError> {
        let (user, tokens) = AuthLifecycleService::refresh_runtime(
            &self.runtime_ctx,
            &self.auth_config,
            context.tenant_id,
            &refresh_token,
        )
        .await
        .map_err(map_lifecycle_error)?;
        self.token_record(context.tenant_id, user, tokens).await
    }

    async fn forgot_password(
        &self,
        context: &AuthLifecycleContext,
        email: String,
    ) -> Result<(), AuthLifecycleMutationError> {
        let user = users::Entity::find_by_email(self.runtime_ctx.db(), context.tenant_id, &email)
            .await
            .map_err(|err| AuthLifecycleMutationError::Internal(err.to_string()))?;

        let Some(user) = user else {
            return Ok(());
        };

        let reset_token = encode_password_reset_token(
            &self.auth_config,
            context.tenant_id,
            &user.email,
            &user.password_hash,
            DEFAULT_RESET_TOKEN_TTL_SECS,
        )
        .map_err(|err| AuthLifecycleMutationError::Internal(err.to_string()))?;
        let email_service = email_service_from_ctx(&self.runtime_ctx, context.locale.as_str())
            .map_err(|err| AuthLifecycleMutationError::Internal(err.to_string()))?;
        let reset_url = password_reset_url(&self.runtime_ctx, &reset_token)
            .map_err(|err| AuthLifecycleMutationError::Internal(err.to_string()))?;
        let recipient = user.email;

        tokio::spawn(async move {
            if let Err(error) = email_service
                .send_password_reset(PasswordResetEmail {
                    to: recipient,
                    reset_url,
                })
                .await
            {
                tracing::warn!(error = %error, "Failed to send password reset email");
            }
        });

        Ok(())
    }

    async fn update_profile(
        &self,
        context: &AuthLifecycleContext,
        name: Option<String>,
    ) -> Result<AuthUserRecord, AuthLifecycleMutationError> {
        let user_id = Self::require_user_id(context)?;
        let updated = AuthLifecycleService::update_profile_runtime(
            &self.runtime_ctx,
            context.tenant_id,
            user_id,
            name,
        )
        .await
        .map_err(map_lifecycle_error)?;

        Ok(AuthUserRecord {
            id: updated.id,
            email: updated.email,
            name: updated.name,
            role: infer_user_role_from_permissions(&context.permissions),
            status: updated.status.to_string(),
            permissions: permission_strings_from_context(context),
        })
    }

    async fn change_password(
        &self,
        context: &AuthLifecycleContext,
        current_password: String,
        new_password: String,
    ) -> Result<(), AuthLifecycleMutationError> {
        AuthLifecycleService::change_password_runtime(
            &self.runtime_ctx,
            context.tenant_id,
            Self::require_user_id(context)?,
            Self::require_session_id(context)?,
            &current_password,
            &new_password,
        )
        .await
        .map_err(map_lifecycle_error)
    }

    async fn reset_password(
        &self,
        context: &AuthLifecycleContext,
        token: String,
        new_password: String,
    ) -> Result<(), AuthLifecycleMutationError> {
        AuthLifecycleService::confirm_bound_password_reset_runtime(
            &self.runtime_ctx,
            &self.auth_config,
            context.tenant_id,
            &token,
            &new_password,
        )
        .await
        .map_err(map_lifecycle_error)
    }

    async fn logout(
        &self,
        context: &AuthLifecycleContext,
    ) -> Result<(), AuthLifecycleMutationError> {
        AuthLifecycleService::logout_runtime(
            &self.runtime_ctx,
            context.tenant_id,
            Self::require_session_id(context)?,
        )
        .await
        .map_err(map_lifecycle_error)
    }

    async fn revoke_session(
        &self,
        context: &AuthLifecycleContext,
        session_id: uuid::Uuid,
    ) -> Result<bool, AuthLifecycleMutationError> {
        AuthLifecycleService::revoke_session_runtime(
            &self.runtime_ctx,
            context.tenant_id,
            Self::require_user_id(context)?,
            session_id,
        )
        .await
        .map_err(map_lifecycle_error)
    }

    async fn revoke_all_sessions(
        &self,
        context: &AuthLifecycleContext,
    ) -> Result<u64, AuthLifecycleMutationError> {
        AuthLifecycleService::revoke_all_other_sessions_runtime(
            &self.runtime_ctx,
            context.tenant_id,
            Self::require_user_id(context)?,
            Self::require_session_id(context)?,
        )
        .await
        .map_err(map_lifecycle_error)
    }

    async fn accept_invite(
        &self,
        context: &AuthLifecycleContext,
        token: String,
        password: String,
        name: Option<String>,
    ) -> Result<AcceptInviteRecord, AuthLifecycleMutationError> {
        let accepted = AuthLifecycleService::accept_invite_once_runtime(
            &self.runtime_ctx,
            &self.auth_config,
            context.tenant_id,
            &token,
            &password,
            name,
        )
        .await
        .map_err(map_invite_error)?;

        Ok(AcceptInviteRecord {
            email: accepted.email,
            role: accepted.role,
        })
    }
}

#[async_trait]
impl AuthUserBackfillReadPort for ServerAuthLifecycleProvider {
    async fn list_users_for_profile_backfill(
        &self,
        request: AuthUserBackfillReadRequest,
    ) -> Result<Vec<AuthUserBackfillRecord>, AuthLifecycleMutationError> {
        AuthUserBackfillDbReader::new(self.runtime_ctx.db_clone())
            .list_users_for_profile_backfill(request)
            .await
    }
}

fn permission_strings_from_context(context: &AuthLifecycleContext) -> Vec<String> {
    let mut values = context
        .permissions
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    values.sort();
    values.dedup();
    values
}

fn map_invite_error(error: InviteAcceptanceError) -> AuthLifecycleMutationError {
    match error {
        InviteAcceptanceError::InvalidToken => AuthLifecycleMutationError::InvalidInviteToken,
        InviteAcceptanceError::EmailAlreadyExists => AuthLifecycleMutationError::EmailAlreadyExists,
        InviteAcceptanceError::Internal(error) => {
            AuthLifecycleMutationError::Internal(error.to_string())
        }
    }
}

fn map_lifecycle_error(error: AuthLifecycleError) -> AuthLifecycleMutationError {
    match error {
        AuthLifecycleError::EmailAlreadyExists => AuthLifecycleMutationError::EmailAlreadyExists,
        AuthLifecycleError::InvalidCredentials => AuthLifecycleMutationError::InvalidCredentials,
        AuthLifecycleError::UserInactive => AuthLifecycleMutationError::UserInactive,
        AuthLifecycleError::InvalidRefreshToken => AuthLifecycleMutationError::InvalidRefreshToken,
        AuthLifecycleError::SessionExpired => AuthLifecycleMutationError::SessionExpired,
        AuthLifecycleError::UserNotFound => AuthLifecycleMutationError::UserNotFound,
        AuthLifecycleError::InvalidResetToken => AuthLifecycleMutationError::InvalidResetToken,
        AuthLifecycleError::Internal(err) => AuthLifecycleMutationError::Internal(err.to_string()),
    }
}
