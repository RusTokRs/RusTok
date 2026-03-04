use async_graphql::{Context, FieldError, Object, Result};
use loco_rs::prelude::*;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};

use crate::auth::{encode_password_reset_token, AuthConfig};
use crate::context::TenantContext;
use crate::graphql::errors::GraphQLError;
use crate::models::users;
use crate::services::auth_lifecycle::{AuthLifecycleError, AuthLifecycleService};
use crate::services::email::{EmailService, PasswordResetEmail, PasswordResetEmailSender};

use crate::context::AuthContext;

use super::types::*;

const DEFAULT_RESET_TOKEN_TTL_SECS: u64 = 15 * 60;

fn map_auth_lifecycle_error(error: AuthLifecycleError) -> FieldError {
    match error {
        AuthLifecycleError::EmailAlreadyExists => FieldError::new("Email already exists"),
        AuthLifecycleError::InvalidCredentials => FieldError::new("Invalid credentials"),
        AuthLifecycleError::UserInactive => FieldError::new("User is inactive"),
        AuthLifecycleError::InvalidRefreshToken => FieldError::new("Invalid refresh token"),
        AuthLifecycleError::SessionExpired => FieldError::new("Session expired"),
        AuthLifecycleError::UserNotFound => FieldError::new("User not found"),
        AuthLifecycleError::InvalidResetToken => FieldError::new("Invalid reset token"),
        AuthLifecycleError::Internal(err) => {
            <FieldError as GraphQLError>::internal_error(&err.to_string())
        }
    }
}

#[derive(Default)]
pub struct AuthMutation;

#[Object]
impl AuthMutation {
    /// Sign in with email and password
    async fn sign_in(&self, ctx: &Context<'_>, input: SignInInput) -> Result<AuthPayload> {
        let app_ctx = ctx.data::<AppContext>()?;
        let tenant = ctx.data::<TenantContext>()?;

        let (user, tokens) = AuthLifecycleService::login(
            app_ctx,
            tenant.id,
            &input.email,
            &input.password,
            None,
            None,
        )
        .await
        .map_err(map_auth_lifecycle_error)?;

        Ok(AuthPayload {
            access_token: tokens.access_token,
            refresh_token: tokens.refresh_token,
            token_type: "Bearer".to_string(),
            expires_in: tokens.expires_in as i32,
            user: AuthUser {
                id: user.id.to_string(),
                email: user.email,
                name: user.name,
                role: user.role.to_string(),
                status: user.status.to_string(),
            },
        })
    }

    /// Sign up with email, password, and optional name
    async fn sign_up(&self, ctx: &Context<'_>, input: SignUpInput) -> Result<AuthPayload> {
        let app_ctx = ctx.data::<AppContext>()?;
        let tenant = ctx.data::<TenantContext>()?;

        let (user, tokens) = AuthLifecycleService::register(
            app_ctx,
            tenant.id,
            &input.email,
            &input.password,
            input.name,
        )
        .await
        .map_err(map_auth_lifecycle_error)?;

        Ok(AuthPayload {
            access_token: tokens.access_token,
            refresh_token: tokens.refresh_token,
            token_type: "Bearer".to_string(),
            expires_in: tokens.expires_in as i32,
            user: AuthUser {
                id: user.id.to_string(),
                email: user.email,
                name: user.name,
                role: user.role.to_string(),
                status: user.status.to_string(),
            },
        })
    }

    /// Refresh access token using refresh token
    async fn refresh_token(
        &self,
        ctx: &Context<'_>,
        input: RefreshTokenInput,
    ) -> Result<AuthPayload> {
        let app_ctx = ctx.data::<AppContext>()?;
        let tenant = ctx.data::<TenantContext>()?;

        let (user, tokens) =
            AuthLifecycleService::refresh(app_ctx, tenant.id, &input.refresh_token)
                .await
                .map_err(map_auth_lifecycle_error)?;

        Ok(AuthPayload {
            access_token: tokens.access_token,
            refresh_token: tokens.refresh_token,
            token_type: "Bearer".to_string(),
            expires_in: tokens.expires_in as i32,
            user: AuthUser {
                id: user.id.to_string(),
                email: user.email,
                name: user.name,
                role: user.role.to_string(),
                status: user.status.to_string(),
            },
        })
    }

    /// Request password reset email
    async fn forgot_password(
        &self,
        ctx: &Context<'_>,
        input: ForgotPasswordInput,
    ) -> Result<ForgotPasswordPayload> {
        let app_ctx = ctx.data::<AppContext>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let config = AuthConfig::from_ctx(app_ctx)
            .map_err(|e| <FieldError as GraphQLError>::internal_error(&e.to_string()))?;

        // Check if user exists
        let user = users::Entity::find_by_email(&app_ctx.db, tenant.id, &input.email)
            .await
            .map_err(|e| <FieldError as GraphQLError>::internal_error(&e.to_string()))?;

        if user.is_none() {
            // Don't reveal if user exists
            return Ok(ForgotPasswordPayload {
                success: true,
                message: "If the email exists, a password reset link has been sent".to_string(),
            });
        }

        let reset_token = encode_password_reset_token(
            &config,
            tenant.id,
            &input.email,
            DEFAULT_RESET_TOKEN_TTL_SECS,
        )
        .map_err(|e| <FieldError as GraphQLError>::internal_error(&e.to_string()))?;

        let email_service = EmailService::from_ctx(app_ctx)
            .map_err(|e| <FieldError as GraphQLError>::internal_error(&e.to_string()))?;
        let reset_url = email_service
            .password_reset_url(app_ctx, &reset_token)
            .map_err(|e| <FieldError as GraphQLError>::internal_error(&e.to_string()))?;

        email_service
            .send_password_reset(PasswordResetEmail {
                to: input.email,
                reset_url,
            })
            .await
            .map_err(|e| <FieldError as GraphQLError>::internal_error(&e.to_string()))?;

        Ok(ForgotPasswordPayload {
            success: true,
            message: "If the email exists, a password reset link has been sent".to_string(),
        })
    }

    /// Update current user profile (name)
    async fn update_profile(
        &self,
        ctx: &Context<'_>,
        input: UpdateProfileInput,
    ) -> Result<AuthUser> {
        let app_ctx = ctx.data::<AppContext>()?;
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
        let tenant = ctx.data::<TenantContext>()?;

        let user = users::Entity::find_by_id(auth.user_id)
            .filter(crate::models::_entities::users::Column::TenantId.eq(tenant.id))
            .one(&app_ctx.db)
            .await
            .map_err(|e| <FieldError as GraphQLError>::internal_error(&e.to_string()))?
            .ok_or_else(|| FieldError::new("User not found"))?;

        let mut model: users::ActiveModel = user.into();
        if let Some(name) = input.name {
            model.name = Set(if name.trim().is_empty() {
                None
            } else {
                Some(name.trim().to_string())
            });
        }

        let updated = model
            .update(&app_ctx.db)
            .await
            .map_err(|e| <FieldError as GraphQLError>::internal_error(&e.to_string()))?;

        Ok(AuthUser {
            id: updated.id.to_string(),
            email: updated.email,
            name: updated.name,
            role: updated.role.to_string(),
            status: updated.status.to_string(),
        })
    }

    /// Change password for the currently authenticated user
    async fn change_password(
        &self,
        ctx: &Context<'_>,
        input: ChangePasswordInput,
    ) -> Result<ChangePasswordPayload> {
        let app_ctx = ctx.data::<AppContext>()?;
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
        let tenant = ctx.data::<TenantContext>()?;

        AuthLifecycleService::change_password(
            app_ctx,
            tenant.id,
            auth.user_id,
            auth.session_id,
            &input.current_password,
            &input.new_password,
        )
        .await
        .map_err(map_auth_lifecycle_error)?;

        Ok(ChangePasswordPayload { success: true })
    }

    /// Reset password using reset token
    async fn reset_password(
        &self,
        ctx: &Context<'_>,
        input: ResetPasswordInput,
    ) -> Result<ResetPasswordPayload> {
        let app_ctx = ctx.data::<AppContext>()?;
        let tenant = ctx.data::<TenantContext>()?;

        AuthLifecycleService::confirm_password_reset(
            app_ctx,
            tenant.id,
            &input.token,
            &input.new_password,
        )
        .await
        .map_err(map_auth_lifecycle_error)?;

        Ok(ResetPasswordPayload { success: true })
    }
}

#[cfg(test)]
mod tests {
    use super::{map_auth_lifecycle_error, AuthLifecycleError};

    #[test]
    fn maps_invalid_refresh_token_message() {
        let err = map_auth_lifecycle_error(AuthLifecycleError::InvalidRefreshToken);
        assert!(err.message.contains("Invalid refresh token"));
    }

    #[test]
    fn maps_user_inactive_message() {
        let err = map_auth_lifecycle_error(AuthLifecycleError::UserInactive);
        assert!(err.message.contains("User is inactive"));
    }

    #[test]
    fn maps_invalid_reset_token_message() {
        let err = map_auth_lifecycle_error(AuthLifecycleError::InvalidResetToken);
        assert!(err.message.contains("Invalid reset token"));
    }

    #[test]
    fn maps_internal_to_internal_graphql_error() {
        let err = map_auth_lifecycle_error(AuthLifecycleError::Internal(
            loco_rs::prelude::Error::InternalServerError,
        ));
        assert!(!err.message.is_empty());
    }
}
