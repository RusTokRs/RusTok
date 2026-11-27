use std::sync::Arc;

use async_graphql::{Context, FieldError, Result};
use uuid::Uuid;

use crate::{AuthContext, TenantContext};
use super::GraphQLError;

/// Host-owned admission check for a fresh protected GraphQL read. This is
/// separate from serializing a previously admitted command receipt. Providers
/// must validate the original credential ceiling, never reconstruct grants.
#[async_trait::async_trait]
pub trait GraphqlReadAuthority: Send + Sync {
    async fn revalidate(&self, ctx: &Context<'_>) -> Result<()>;
}

#[derive(Clone)]
pub struct GraphqlReadAuthorityHandle(pub Arc<dyn GraphqlReadAuthority>);

/// Check the request/target tenant and require a live authority provider.
/// The caller still owns its specific permission and resource policy.
pub async fn require_live_graphql_read<'a>(
    ctx: &'a Context<'_>,
    target_tenant_id: Uuid,
) -> Result<&'a AuthContext> {
    let auth = ctx.data::<AuthContext>()
        .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
    let tenant = ctx.data::<TenantContext>()
        .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
    if target_tenant_id != tenant.id || target_tenant_id != auth.tenant_id {
        return Err(<FieldError as GraphQLError>::permission_denied(
            "Read must use the authenticated tenant",
        ));
    }
    let authority = ctx.data::<GraphqlReadAuthorityHandle>()
        .map_err(|_| <FieldError as GraphQLError>::internal_error(
            "Request authority is temporarily unavailable",
        ))?;
    authority.0.revalidate(ctx).await?;
    Ok(auth)
}
