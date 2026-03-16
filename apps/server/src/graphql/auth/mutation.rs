use async_graphql::{Context, FieldError, Object, Result};
use rustok_core::i18n::{Locale, translate};
use loco_rs::app::AppContext;
use crate::error::Error;

use crate::auth::{auth_config_from_ctx, encode_password_reset_token};
use crate::context::{infer_user_role_from_permissions, TenantContext};
use crate::graphql::errors::{ErrorCode, GraphQLError};
use crate::models::users;
use crate::services::auth_lifecycle::{AuthLifecycleError, AuthLifecycleService};
use crate::services::email::{
    email_service_from_ctx, password_reset_url, PasswordResetEmail, PasswordResetEmailSender,
};

use crate::context::AuthContext;

use super::types::*;

const DEFAULT_RESET_TOKEN_TTL_SECS: u64 = 15 * 60;

fn unauthenticated_auth_error(message: &str) -> FieldError {
    use async_graphql::ErrorExtensions;

    FieldError::new(message).extend_with(|_, e| {
        e.set("code", ErrorCode::Unauthenticated.as_str());
    })
}

fn map_auth_lifecycle_error(error: AuthLifecycleError, locale: Locale) -> FieldError {
    let t = |key: &str| translate(locale, key);
    match error {
        AuthLifecycleError::EmailAlreadyExists => {
            FieldError::new(t("auth.email_already_exists"))
        }
        AuthLifecycleError::InvalidCredentials => {
            unauthenticated_auth_error(&t("auth.invalid_credentials"))
        }
        AuthLifecycleError::UserInactive => {
            unauthenticated_auth_error(&t("auth.user_inactive"))
        }
        AuthLifecycleError::InvalidRefreshToken => {
            unauthenticated_auth_error(&t("auth.invalid_refresh_token"))
        }
        AuthLifecycleError::SessionExpired => {
            unauthenticated_auth_error(&t("auth.session_expired"))
        }
        AuthLifecycleError::UserNotFound => {
            unauthenticated_auth_error(&t("auth.user_not_found"))
        }
        AuthLifecycleError::InvalidResetToken => {
            unauthenticated_auth_error(&t("auth.invalid_reset_token"))
        }
        AuthLifecycleError::Internal(err) => {
            <FieldError as GraphQLError>::internal_error(&err.to_string())
        }
    }
}


fn locale_from_ctx(ctx: &Context<'_>) -> Locale {
    ctx.data::<Locale>().copied().unwrap_or_default()
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
        .map_err(|e| map_auth_lifecycle_error(e, locale_from_ctx(ctx)))?;

        Ok(AuthPayload {
            access_token: tokens.access_token,
            refresh_token: tokens.refresh_token,
            token_type: "Bearer".to_string(),
            expires_in: tokens.expires_in as i32,
            user: AuthUser {
                id: user.id.to_string(),
                email: user.email,
                name: user.name,
                role: tokens.effective_role.to_string(),
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
        .map_err(|e| map_auth_lifecycle_error(e, locale_from_ctx(ctx)))?;

        Ok(AuthPayload {
            access_token: tokens.access_token,
            refresh_token: tokens.refresh_token,
            token_type: "Bearer".to_string(),
            expires_in: tokens.expires_in as i32,
            user: AuthUser {
                id: user.id.to_string(),
                email: user.email,
                name: user.name,
                role: tokens.effective_role.to_string(),
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
                .map_err(|e| map_auth_lifecycle_error(e, locale_from_ctx(ctx)))?;

        Ok(AuthPayload {
            access_token: tokens.access_token,
            refresh_token: tokens.refresh_token,
            token_type: "Bearer".to_string(),
            expires_in: tokens.expires_in as i32,
            user: AuthUser {
                id: user.id.to_string(),
                email: user.email,
                name: user.name,
                role: tokens.effective_role.to_string(),
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
        let config = auth_config_from_ctx(app_ctx)
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

        let email_service = email_service_from_ctx(app_ctx)
            .map_err(|e| <FieldError as GraphQLError>::internal_error(&e.to_string()))?;
        let reset_url = password_reset_url(app_ctx, &reset_token)
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

        let updated =
            AuthLifecycleService::update_profile(app_ctx, tenant.id, auth.user_id, input.name)
                .await
                .map_err(|e| map_auth_lifecycle_error(e, locale_from_ctx(ctx)))?;

        Ok(AuthUser {
            id: updated.id.to_string(),
            email: updated.email,
            name: updated.name,
            role: infer_user_role_from_permissions(&auth.permissions).to_string(),
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
        .map_err(|e| map_auth_lifecycle_error(e, locale_from_ctx(ctx)))?;

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
        .map_err(|e| map_auth_lifecycle_error(e, locale_from_ctx(ctx)))?;

        Ok(ResetPasswordPayload { success: true })
    }
}

#[cfg(test)]
mod tests {
    use super::{map_auth_lifecycle_error, locale_from_ctx, AuthLifecycleError};
    use rustok_core::i18n::Locale;

    #[test]
    fn maps_invalid_refresh_token_message() {
        let err = map_auth_lifecycle_error(AuthLifecycleError::InvalidRefreshToken, Locale::En);
        assert!(err.message.contains("Invalid refresh token"));
    }

    #[test]
    fn maps_user_inactive_message() {
        let err = map_auth_lifecycle_error(AuthLifecycleError::UserInactive, Locale::En);
        assert!(err.message.contains("User is inactive"));
    }

    #[test]
    fn maps_invalid_reset_token_message() {
        let err = map_auth_lifecycle_error(AuthLifecycleError::InvalidResetToken, Locale::En);
        assert!(err.message.contains("Invalid reset token"));
    }

    #[test]
    fn maps_user_not_found_message() {
        let err = map_auth_lifecycle_error(AuthLifecycleError::UserNotFound, Locale::En);
        assert!(err.message.contains("User not found"));
    }

    #[test]
    fn maps_auth_errors_with_unauthenticated_code() {
        let err = map_auth_lifecycle_error(AuthLifecycleError::InvalidCredentials, Locale::En);
        let code = err
            .extensions
            .as_ref()
            .and_then(|ext| ext.get("code"))
            .and_then(|value| match value {
                async_graphql::Value::String(s) => Some(s.as_str()),
                _ => None,
            });
        assert_eq!(code, Some("UNAUTHENTICATED"));
    }

    #[test]
    fn maps_internal_to_internal_graphql_error() {
        let err = map_auth_lifecycle_error(
            AuthLifecycleError::Internal(crate::error::Error::InternalServerError),
            Locale::En,
        );
        assert!(!err.message.is_empty());
    }
}
