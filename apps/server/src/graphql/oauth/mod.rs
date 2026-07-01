pub mod mutation;
pub mod query;
pub mod types;

use async_graphql::{FieldError, Result};
use sea_orm::DatabaseConnection;

use crate::context::AuthContext;
use crate::graphql::errors::GraphQLError;
use crate::services::rbac_service::RbacService;
use rustok_api::Permission;

pub(super) async fn ensure_oauth_admin(auth: &AuthContext, db: &DatabaseConnection) -> Result<()> {
    let has_admin_permission = RbacService::has_any_permission(
        db,
        &auth.tenant_id,
        &auth.user_id,
        &[Permission::SETTINGS_MANAGE, Permission::USERS_MANAGE],
    )
    .await
    .map_err(|e| <FieldError as GraphQLError>::internal_error(&e.to_string()))?;

    if !has_admin_permission {
        return Err(<FieldError as GraphQLError>::permission_denied(
            "Permission denied: settings:manage or users:manage required",
        ));
    }

    Ok(())
}

pub use mutation::OAuthMutation;
pub use query::OAuthQuery;
pub use types::*;
