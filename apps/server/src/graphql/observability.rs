use std::sync::Arc;
use std::time::Instant;

use async_graphql::extensions::{
    Extension, ExtensionContext, ExtensionFactory, NextResolve, ResolveInfo,
};
use async_graphql::{QueryPathSegment, ServerResult, Value};

#[derive(Default)]
pub struct GraphqlObservability;

impl ExtensionFactory for GraphqlObservability {
    fn create(&self) -> Arc<dyn Extension> {
        Arc::new(GraphqlObservabilityExtension)
    }
}

struct GraphqlObservabilityExtension;

#[async_trait::async_trait]
impl Extension for GraphqlObservabilityExtension {
    async fn resolve(
        &self,
        _ctx: &ExtensionContext<'_>,
        info: ResolveInfo<'_>,
        next: NextResolve<'_>,
    ) -> ServerResult<Option<Value>> {
        let started_at = Instant::now();
        let parent_type = info.parent_type;
        let return_type = info.return_type;
        let field_name = info.path_node.field_name().to_string();
        let cardinality = match info.path_node.segment {
            QueryPathSegment::Index(idx) => idx.to_string(),
            QueryPathSegment::Name(_) => "single".to_string(),
        };

        let result = next.run(_ctx, info).await;
        let duration_ms = started_at.elapsed().as_secs_f64() * 1000.0;
        let status = if result.is_ok() { "ok" } else { "error" };

        tracing::info!(
            target: "graphql.resolver",
            parent_type,
            return_type,
            field_name,
            cardinality,
            status,
            latency_ms = duration_ms,
            "graphql resolver completed"
        );

        result
    }
}
