use async_graphql::{Context, FieldError, Object, Result};
use rustok_api::{
    graphql::{require_module_enabled, GraphQLError},
    AuthContext, TenantContext,
};
use sea_orm::DatabaseConnection;

use crate::{ProfileError, ProfileService};

use super::{types::*, MODULE_SLUG};

#[derive(Default)]
pub struct ProfilesMutation;

#[Object]
impl ProfilesMutation {
    async fn upsert_my_profile(
        &self,
        ctx: &Context<'_>,
        input: GqlUpsertProfileInput,
    ) -> Result<GqlProfile> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let auth = require_auth(ctx)?;
        let db = ctx.data::<DatabaseConnection>()?;
        let tenant = ctx.data::<TenantContext>()?;

        let service = ProfileService::new(db.clone());
        let profile = service
            .upsert_profile(
                tenant.id,
                auth.user_id,
                input.into(),
                Some(tenant.default_locale.as_str()),
            )
            .await
            .map_err(map_profile_error)?;

        Ok(profile.into())
    }
}

fn require_auth(ctx: &Context<'_>) -> Result<AuthContext> {
    ctx.data::<AuthContext>()
        .cloned()
        .map_err(|_| <FieldError as GraphQLError>::unauthenticated().into())
}

fn map_profile_error(err: ProfileError) -> async_graphql::Error {
    match err {
        ProfileError::EmptyHandle
        | ProfileError::InvalidHandle
        | ProfileError::HandleTooShort
        | ProfileError::HandleTooLong
        | ProfileError::ReservedHandle(_)
        | ProfileError::InvalidLocale(_)
        | ProfileError::DuplicateHandle(_) => {
            <FieldError as GraphQLError>::bad_user_input(&err.to_string()).into()
        }
        ProfileError::ProfileNotFound(_) | ProfileError::ProfileByHandleNotFound(_) => {
            <FieldError as GraphQLError>::not_found(&err.to_string()).into()
        }
        ProfileError::Database(_) => {
            <FieldError as GraphQLError>::internal_error(&err.to_string()).into()
        }
    }
}
