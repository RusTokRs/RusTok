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

// Response paths contain client-controlled aliases (including for list items).
// Use the validated schema field, never the response key, for telemetry.
fn resolver_field_name<'a>(info: &ResolveInfo<'a>) -> &'a str {
    info.name
}

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
        let field_name = resolver_field_name(&info);
        let cardinality = match info.path_node.segment {
            QueryPathSegment::Index(_) => "indexed",
            QueryPathSegment::Name(_) => "single",
        };

        let result = next.run(_ctx, info).await;
        let duration_ms = started_at.elapsed().as_secs_f64() * 1000.0;
        let status = if result.is_ok() { "ok" } else { "error" };

        tracing::debug!(
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

#[cfg(test)]
mod tests {
    use super::*;
    use async_graphql::context::QueryPathNode;
    use async_graphql::parser::{parse_query, types::Selection};

    #[test]
    fn resolver_field_name_uses_schema_identity_for_aliases_and_list_items() {
        let document = parse_query("{ private_alias: items }").expect("valid query");
        let operations = crate::graphql::selected_operations(&document, None);
        let Selection::Field(field) = &operations[0].node.selection_set.node.items[0].node else {
            panic!("expected a field selection");
        };
        let path = QueryPathNode {
            parent: None,
            segment: QueryPathSegment::Name("private_alias"),
        };
        let indexed_path = QueryPathNode {
            parent: Some(&path),
            segment: QueryPathSegment::Index(0),
        };
        for path_node in [&path, &indexed_path] {
            let info = ResolveInfo {
                path_node,
                parent_type: "Query",
                return_type: "[String!]!",
                name: field.node.name.node.as_str(),
                alias: Some("private_alias"),
                is_for_introspection: false,
                field: &field.node,
            };
            assert_eq!(path_node.field_name(), "private_alias");
            assert_eq!(resolver_field_name(&info), "items");
        }
    }
}
