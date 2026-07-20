use std::collections::BTreeSet;
use std::sync::Arc;

use async_graphql::extensions::{
    Extension, ExtensionContext, ExtensionFactory, NextExecute, NextPrepareRequest,
};
use async_graphql::parser::types::{ExecutableDocument, OperationType, Selection, SelectionSet};
use async_graphql::{FieldError, Pos, Request, Response, ServerResult};
use rustok_api::{Permission, graphql::GraphQLError, has_effective_permission};

use crate::context::AuthContext;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum DashboardField {
    DashboardStats,
    RecentActivity,
}

impl DashboardField {
    fn classify(operation_type: OperationType, field_name: &str) -> Option<Self> {
        if operation_type != OperationType::Query {
            return None;
        }
        match field_name {
            "dashboardStats" => Some(Self::DashboardStats),
            "recentActivity" => Some(Self::RecentActivity),
            _ => None,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::DashboardStats => "dashboardStats",
            Self::RecentActivity => "recentActivity",
        }
    }
}

#[derive(Clone, Debug)]
struct DashboardDocumentPolicy(Vec<DashboardField>);

fn collect_fields(
    operation_type: OperationType,
    selection_set: &SelectionSet,
    document: &ExecutableDocument,
    fields: &mut BTreeSet<DashboardField>,
) {
    for selection in &selection_set.items {
        match &selection.node {
            Selection::Field(field) => {
                if let Some(field) =
                    DashboardField::classify(operation_type, field.node.name.node.as_str())
                {
                    fields.insert(field);
                }
            }
            Selection::FragmentSpread(fragment) => {
                if let Some(definition) = document.fragments.get(&fragment.node.fragment_name.node)
                {
                    collect_fields(
                        operation_type,
                        &definition.node.selection_set.node,
                        document,
                        fields,
                    );
                }
            }
            Selection::InlineFragment(fragment) => collect_fields(
                operation_type,
                &fragment.node.selection_set.node,
                document,
                fields,
            ),
        }
    }
}

fn classify_document(request: &mut Request) -> ServerResult<()> {
    if request.query.trim().is_empty() {
        return Ok(());
    }

    let document = request.parsed_query()?;
    let mut fields = BTreeSet::new();
    for (_, operation) in document.operations.iter() {
        collect_fields(
            operation.node.ty,
            &operation.node.selection_set.node,
            document,
            &mut fields,
        );
    }
    if !fields.is_empty() {
        request
            .data
            .insert(DashboardDocumentPolicy(fields.into_iter().collect()));
    }
    Ok(())
}

#[derive(Default)]
pub struct GraphqlDashboardSecurityPolicy;

impl ExtensionFactory for GraphqlDashboardSecurityPolicy {
    fn create(&self) -> Arc<dyn Extension> {
        Arc::new(GraphqlDashboardSecurityPolicyExtension)
    }
}

struct GraphqlDashboardSecurityPolicyExtension;

#[async_trait::async_trait]
impl Extension for GraphqlDashboardSecurityPolicyExtension {
    async fn prepare_request(
        &self,
        ctx: &ExtensionContext<'_>,
        request: Request,
        next: NextPrepareRequest<'_>,
    ) -> ServerResult<Request> {
        let mut request = next.run(ctx, request).await?;
        classify_document(&mut request)?;
        Ok(request)
    }

    async fn execute(
        &self,
        ctx: &ExtensionContext<'_>,
        operation_name: Option<&str>,
        next: NextExecute<'_>,
    ) -> Response {
        if let Some(policy) = ctx.data_opt::<DashboardDocumentPolicy>() {
            let allowed = ctx.data_opt::<AuthContext>().is_some_and(|auth| {
                has_effective_permission(&auth.permissions, &Permission::ANALYTICS_READ)
            });
            if !allowed {
                tracing::warn!(
                    fields = ?policy.0.iter().map(|field| field.name()).collect::<Vec<_>>(),
                    operation_name = ?operation_name,
                    "Rejected dashboard analytics GraphQL document before resolver execution"
                );
                return Response::from_errors(vec![
                    <FieldError as GraphQLError>::permission_denied(
                        "analytics:read required for dashboard statistics and recent activity",
                    )
                    .into_server_error(Pos::default()),
                ]);
            }
        }

        next.run(ctx, operation_name).await
    }
}

#[cfg(test)]
mod tests {
    use super::{DashboardDocumentPolicy, DashboardField, classify_document};
    use async_graphql::Request;

    #[test]
    fn finds_dashboard_fields_inside_fragments() {
        let mut request = Request::new(
            r#"
                query Dashboard { ...DashboardFields }
                fragment DashboardFields on Query {
                    dashboardStats { totalUsers }
                    recentActivity { id }
                }
            "#,
        );

        classify_document(&mut request).expect("dashboard document should parse");
        let policy = request
            .data
            .get(&std::any::TypeId::of::<DashboardDocumentPolicy>())
            .and_then(|value| value.downcast_ref::<DashboardDocumentPolicy>())
            .expect("dashboard policy should be attached");
        assert_eq!(
            policy.0,
            vec![
                DashboardField::DashboardStats,
                DashboardField::RecentActivity
            ]
        );
    }
}
