use std::{any::TypeId, sync::Arc};

use async_graphql::dataloader::DataLoader;
use async_graphql::extensions::{
    Extension, ExtensionContext, ExtensionFactory, NextPrepareRequest,
};
use async_graphql::{Request, ServerResult};
use rustok_profiles::{ProfileAccessAudience, ProfileSummaryLoader};
use sea_orm::DatabaseConnection;

use crate::context::AuthContext;

#[derive(Default)]
pub struct ProfileSummaryAudiencePolicy;

impl ExtensionFactory for ProfileSummaryAudiencePolicy {
    fn create(&self) -> Arc<dyn Extension> {
        Arc::new(ProfileSummaryAudiencePolicyExtension)
    }
}

struct ProfileSummaryAudiencePolicyExtension;

#[async_trait::async_trait]
impl Extension for ProfileSummaryAudiencePolicyExtension {
    async fn prepare_request(
        &self,
        ctx: &ExtensionContext<'_>,
        request: Request,
        next: NextPrepareRequest<'_>,
    ) -> ServerResult<Request> {
        let mut request = next.run(ctx, request).await?;
        let db = request_data::<DatabaseConnection>(&request)
            .cloned()
            .or_else(|| ctx.data_opt::<DatabaseConnection>().cloned());
        let Some(db) = db else {
            tracing::error!(
                "Profile summary audience policy could not resolve the GraphQL database"
            );
            return Ok(request);
        };
        let auth = request_data::<AuthContext>(&request).or_else(|| ctx.data_opt::<AuthContext>());
        let audience = profile_access_audience(auth);

        request.data.insert(audience);
        request.data.insert(DataLoader::new(
            ProfileSummaryLoader::for_audience(db, audience),
            tokio::spawn,
        ));
        Ok(request)
    }
}

fn request_data<T>(request: &Request) -> Option<&T>
where
    T: Send + Sync + 'static,
{
    request
        .data
        .get(&TypeId::of::<T>())
        .and_then(|value| value.downcast_ref::<T>())
}

fn profile_access_audience(auth: Option<&AuthContext>) -> ProfileAccessAudience {
    match auth {
        None => ProfileAccessAudience::Anonymous,
        Some(auth) if auth.is_service_principal() => {
            ProfileAccessAudience::TrustedService { actor_id: None }
        }
        Some(auth) => ProfileAccessAudience::Authenticated {
            actor_id: auth.user_id,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::{ProfileSummaryAudiencePolicy, profile_access_audience};
    use crate::context::AuthContext;
    use async_graphql::dataloader::DataLoader;
    use async_graphql::{Context, EmptyMutation, EmptySubscription, Object, Request, Schema};
    use rustok_profiles::{ProfileAccessAudience, ProfileSummaryLoader};
    use sea_orm::Database;
    use serde_json::json;
    use uuid::Uuid;

    struct QueryRoot;

    #[Object]
    impl QueryRoot {
        async fn profile_audience(&self, ctx: &Context<'_>) -> &'static str {
            ctx.data::<DataLoader<ProfileSummaryLoader>>()
                .expect("profile summary loader should be request-scoped");
            match ctx
                .data::<ProfileAccessAudience>()
                .expect("profile audience should be request-scoped")
            {
                ProfileAccessAudience::Anonymous => "anonymous",
                ProfileAccessAudience::Authenticated { .. } => "authenticated",
                ProfileAccessAudience::TrustedService { .. } => "trusted_service",
            }
        }
    }

    fn auth_context(grant_type: &str, client_id: Option<&str>) -> AuthContext {
        AuthContext {
            user_id: Uuid::new_v4(),
            session_id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            permissions: Vec::new(),
            client_id: client_id.map(str::to_string),
            scopes: Vec::new(),
            grant_type: grant_type.to_string(),
        }
    }

    #[test]
    fn anonymous_requests_receive_anonymous_profile_audience() {
        assert_eq!(
            profile_access_audience(None),
            ProfileAccessAudience::Anonymous
        );
    }

    #[test]
    fn human_requests_bind_profile_audience_to_user() {
        let auth = auth_context("password", None);
        assert_eq!(
            profile_access_audience(Some(&auth)),
            ProfileAccessAudience::Authenticated {
                actor_id: auth.user_id
            }
        );
    }

    #[test]
    fn service_principals_do_not_claim_profile_ownership() {
        let auth = auth_context("client_credentials", Some("internal-worker"));
        assert_eq!(
            profile_access_audience(Some(&auth)),
            ProfileAccessAudience::TrustedService { actor_id: None }
        );
    }

    #[tokio::test]
    async fn extension_attaches_authenticated_audience_and_loader_from_request_data() {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("test database should connect");
        let schema = Schema::build(QueryRoot, EmptyMutation, EmptySubscription)
            .data(db)
            .extension(ProfileSummaryAudiencePolicy)
            .finish();
        let response = schema
            .execute(Request::new("{ profileAudience }").data(auth_context("password", None)))
            .await;

        assert!(response.errors.is_empty());
        assert_eq!(
            response
                .data
                .into_json()
                .expect("response should serialize"),
            json!({ "profileAudience": "authenticated" })
        );
    }
}
