use async_graphql::{Context, FieldError, Object, Result};
use uuid::Uuid;

use super::{
    AcceptInviteInput, AcceptInvitePayload, AuthPayload, ChangePasswordInput,
    ChangePasswordPayload, ForgotPasswordInput, ForgotPasswordPayload, RefreshTokenInput,
    ResetPasswordInput, ResetPasswordPayload, RevokeAllSessionsPayload, RevokeSessionPayload,
    SignInInput, SignOutPayload, SignUpInput, UpdateProfileInput, auth_lifecycle_context,
    auth_runtime, optional_auth_context,
};

#[derive(Default)]
pub struct AuthMutation;

#[Object]
impl AuthMutation {
    async fn sign_in(&self, ctx: &Context<'_>, input: SignInInput) -> Result<AuthPayload> {
        auth_runtime(ctx)?
            .port()
            .sign_in(
                &auth_lifecycle_context(ctx, optional_auth_context(ctx))?,
                input.email,
                input.password,
            )
            .await
            .map(AuthPayload::from)
            .map_err(|error| super::map_auth_lifecycle_error(ctx, error))
    }

    async fn sign_up(&self, ctx: &Context<'_>, input: SignUpInput) -> Result<AuthPayload> {
        auth_runtime(ctx)?
            .port()
            .sign_up(
                &auth_lifecycle_context(ctx, optional_auth_context(ctx))?,
                input.email,
                input.password,
                input.name,
            )
            .await
            .map(AuthPayload::from)
            .map_err(|error| super::map_auth_lifecycle_error(ctx, error))
    }

    async fn refresh_token(
        &self,
        ctx: &Context<'_>,
        input: RefreshTokenInput,
    ) -> Result<AuthPayload> {
        auth_runtime(ctx)?
            .port()
            .refresh_token(
                &auth_lifecycle_context(ctx, optional_auth_context(ctx))?,
                input.refresh_token,
            )
            .await
            .map(AuthPayload::from)
            .map_err(|error| super::map_auth_lifecycle_error(ctx, error))
    }

    async fn forgot_password(
        &self,
        ctx: &Context<'_>,
        input: ForgotPasswordInput,
    ) -> Result<ForgotPasswordPayload> {
        auth_runtime(ctx)?
            .port()
            .forgot_password(
                &auth_lifecycle_context(ctx, optional_auth_context(ctx))?,
                input.email,
            )
            .await
            .map_err(|error| super::map_auth_lifecycle_error(ctx, error))?;
        Ok(ForgotPasswordPayload {
            success: true,
            message: "If the email exists, a password reset link has been sent".to_string(),
        })
    }

    async fn update_profile(
        &self,
        ctx: &Context<'_>,
        input: UpdateProfileInput,
    ) -> Result<super::AuthUser> {
        let auth = super::require_auth_context(ctx)?;
        auth_runtime(ctx)?
            .port()
            .update_profile(&auth_lifecycle_context(ctx, Some(auth))?, input.name)
            .await
            .map(super::AuthUser::from)
            .map_err(|error| super::map_auth_lifecycle_error(ctx, error))
    }

    async fn change_password(
        &self,
        ctx: &Context<'_>,
        input: ChangePasswordInput,
    ) -> Result<ChangePasswordPayload> {
        let auth = super::require_auth_context(ctx)?;
        auth_runtime(ctx)?
            .port()
            .change_password(
                &auth_lifecycle_context(ctx, Some(auth))?,
                input.current_password,
                input.new_password,
            )
            .await
            .map_err(|error| super::map_auth_lifecycle_error(ctx, error))?;
        Ok(ChangePasswordPayload { success: true })
    }

    async fn reset_password(
        &self,
        ctx: &Context<'_>,
        input: ResetPasswordInput,
    ) -> Result<ResetPasswordPayload> {
        auth_runtime(ctx)?
            .port()
            .reset_password(
                &auth_lifecycle_context(ctx, optional_auth_context(ctx))?,
                input.token,
                input.new_password,
            )
            .await
            .map_err(|error| super::map_auth_lifecycle_error(ctx, error))?;
        Ok(ResetPasswordPayload { success: true })
    }

    async fn logout(&self, ctx: &Context<'_>) -> Result<SignOutPayload> {
        let auth = super::require_auth_context(ctx)?;
        auth_runtime(ctx)?
            .port()
            .logout(&auth_lifecycle_context(ctx, Some(auth))?)
            .await
            .map_err(|error| super::map_auth_lifecycle_error(ctx, error))?;
        Ok(SignOutPayload { success: true })
    }

    async fn revoke_session(
        &self,
        ctx: &Context<'_>,
        session_id: String,
    ) -> Result<RevokeSessionPayload> {
        let auth = super::require_auth_context(ctx)?;
        let session_id = Uuid::parse_str(&session_id)
            .map_err(|_| FieldError::new("Invalid session ID format"))?;
        let revoked = auth_runtime(ctx)?
            .port()
            .revoke_session(&auth_lifecycle_context(ctx, Some(auth))?, session_id)
            .await
            .map_err(|error| super::map_auth_lifecycle_error(ctx, error))?;

        Ok(RevokeSessionPayload {
            success: true,
            revoked,
        })
    }

    async fn revoke_all_sessions(&self, ctx: &Context<'_>) -> Result<RevokeAllSessionsPayload> {
        let auth = super::require_auth_context(ctx)?;
        let revoked_count = auth_runtime(ctx)?
            .port()
            .revoke_all_sessions(&auth_lifecycle_context(ctx, Some(auth))?)
            .await
            .map_err(|error| super::map_auth_lifecycle_error(ctx, error))?;

        Ok(RevokeAllSessionsPayload {
            success: true,
            revoked_count: revoked_count as i32,
        })
    }

    async fn accept_invite(
        &self,
        ctx: &Context<'_>,
        input: AcceptInviteInput,
    ) -> Result<AcceptInvitePayload> {
        let accepted = auth_runtime(ctx)?
            .port()
            .accept_invite(
                &auth_lifecycle_context(ctx, optional_auth_context(ctx))?,
                input.token,
                input.password,
                input.name,
            )
            .await
            .map_err(|error| super::map_auth_lifecycle_error(ctx, error))?;

        Ok(AcceptInvitePayload {
            success: true,
            email: accepted.email,
            role: accepted.role.to_string(),
        })
    }
}
