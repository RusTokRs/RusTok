use std::sync::Arc;

use async_graphql::extensions::{
    Extension, ExtensionContext, ExtensionFactory, NextExecute, NextPrepareRequest, NextResolve,
    ResolveInfo,
};
use async_graphql::{Request, Response, ServerError, ServerResult, Value};
use rustok_api::{AuthContext, RequestContext, TenantContext};

use crate::moderation_transport::{
    ForumModerationTransport, ForumModerationTransportScopeData,
    moderation_audience_port_context, with_forum_moderation_transport_scope,
};

use super::error_extension::ForumGraphqlErrorExtension as BaseForumGraphqlErrorExtension;
use super::runtime_data::ForumGraphqlRuntimeData;

const MODERATION_SOLUTION_FIELDS: [&str; 2] = [
    "markForumTopicSolution",
    "clearForumTopicSolution",
];

/// Preserves the existing Forum GraphQL error/pagination contract while adding
/// an exact resolver-local moderation context for the two mounted solution
/// mutations. The legacy resolvers keep their public field names and DTOs; their
/// context-free `ModerationService` construction consumes this task-local scope.
#[derive(Default)]
pub struct ForumGraphqlErrorExtension;

impl ExtensionFactory for ForumGraphqlErrorExtension {
    fn create(&self) -> Arc<dyn Extension> {
        Arc::new(ForumGraphqlTransportExtension {
            base: BaseForumGraphqlErrorExtension.create(),
        })
    }
}

struct ForumGraphqlTransportExtension {
    base: Arc<dyn Extension>,
}

#[async_trait::async_trait]
impl Extension for ForumGraphqlTransportExtension {
    async fn prepare_request(
        &self,
        ctx: &ExtensionContext<'_>,
        request: Request,
        next: NextPrepareRequest<'_>,
    ) -> ServerResult<Request> {
        self.base.prepare_request(ctx, request, next).await
    }

    async fn execute(
        &self,
        ctx: &ExtensionContext<'_>,
        operation_name: Option<&str>,
        next: NextExecute<'_>,
    ) -> Response {
        self.base.execute(ctx, operation_name, next).await
    }

    async fn resolve(
        &self,
        ctx: &ExtensionContext<'_>,
        info: ResolveInfo<'_>,
        next: NextResolve<'_>,
    ) -> ServerResult<Option<Value>> {
        if !MODERATION_SOLUTION_FIELDS.contains(&info.name) {
            return self.base.resolve(ctx, info, next).await;
        }

        let Some(auth) = ctx.data_opt::<AuthContext>() else {
            return self.base.resolve(ctx, info, next).await;
        };
        let Some(tenant) = ctx.data_opt::<TenantContext>() else {
            return self.base.resolve(ctx, info, next).await;
        };
        let Some(runtime) = ctx.data_opt::<ForumGraphqlRuntimeData>() else {
            return self.base.resolve(ctx, info, next).await;
        };

        let context = moderation_audience_port_context(
            ForumModerationTransport::Graphql,
            tenant.id,
            auth,
            ctx.data_opt::<RequestContext>(),
            tenant.default_locale.as_str(),
        )
        .map_err(|error| ServerError::new(error.to_string(), None))?;
        let scope = ForumModerationTransportScopeData::new(
            runtime.audience_facts(),
            context,
        );

        with_forum_moderation_transport_scope(scope, self.base.resolve(ctx, info, next)).await
    }
}
