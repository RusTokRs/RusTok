mod mutation;
mod query;
mod runtime_data;
mod types;

use async_graphql::{Context, FieldError, Result};
use rustok_api::{AuthContext, TenantContext};
use sea_orm::DatabaseConnection;

pub(crate) async fn require_auth_context<'a>(
    ctx: &'a Context<'_>,
) -> Result<(&'a TenantContext, &'a AuthContext, &'a DatabaseConnection)> {
    let tenant = ctx.data::<TenantContext>()?;
    let auth = ctx.data::<AuthContext>()?;
    if tenant.id.is_nil() {
        return Err(FieldError::new("Invalid tenant ID: nil UUID is not permitted"));
    }
    if auth.user_id.is_nil() {
        return Err(FieldError::new("Invalid user ID: nil UUID is not permitted"));
    }
    if auth.tenant_id != tenant.id {
        return Err(FieldError::new("Permission denied: tenant mismatch"));
    }
    let db = ctx.data::<DatabaseConnection>()?;
    crate::controllers::ensure_messenger_module_enabled(db, tenant.id)
        .await
        .map_err(|e| FieldError::new(e.message))?;
    Ok((tenant, auth, db))
}

pub use mutation::MessengerMutation;
pub use query::MessengerQuery;
pub use runtime_data::{attach_schema_data, MessengerGraphqlRuntimeData};
pub use types::*;

