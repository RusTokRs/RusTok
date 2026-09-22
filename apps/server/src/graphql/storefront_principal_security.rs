use std::sync::Arc;

use async_graphql::extensions::{
    Extension, ExtensionContext, ExtensionFactory, NextExecute, NextPrepareRequest,
};
use async_graphql::parser::types::{ExecutableDocument, Selection, SelectionSet};
use async_graphql::{FieldError, Pos, Request, Response, ServerResult};
use rustok_api::{AuthContext, graphql::GraphQLError};

#[derive(Clone, Copy, Debug)]
struct StorefrontOperationRequested;

fn is_storefront_field(name: &str) -> bool {
    name.starts_with("storefront") || name.contains("Storefront")
}

fn selection_contains_storefront(
    selection_set: &SelectionSet,
    document: &ExecutableDocument,
) -> bool {
    selection_set
        .items
        .iter()
        .any(|selection| match &selection.node {
            Selection::Field(field) => is_storefront_field(field.node.name.node.as_str()),
            Selection::FragmentSpread(fragment) => document
                .fragments
                .get(&fragment.node.fragment_name.node)
                .is_some_and(|definition| {
                    selection_contains_storefront(&definition.node.selection_set.node, document)
                }),
            Selection::InlineFragment(fragment) => {
                selection_contains_storefront(&fragment.node.selection_set.node, document)
            }
        })
}

fn document_contains_storefront(document: &ExecutableDocument) -> bool {
    document.operations.iter().any(|(_, operation)| {
        selection_contains_storefront(&operation.node.selection_set.node, document)
    })
}

#[derive(Default)]
pub struct StorefrontPrincipalPolicy;

impl ExtensionFactory for StorefrontPrincipalPolicy {
    fn create(&self) -> Arc<dyn Extension> {
        Arc::new(StorefrontPrincipalPolicyExtension)
    }
}

struct StorefrontPrincipalPolicyExtension;

#[async_trait::async_trait]
impl Extension for StorefrontPrincipalPolicyExtension {
    async fn prepare_request(
        &self,
        ctx: &ExtensionContext<'_>,
        request: Request,
        next: NextPrepareRequest<'_>,
    ) -> ServerResult<Request> {
        let mut request = next.run(ctx, request).await?;
        if !request.query.trim().is_empty() {
            let document = request.parsed_query()?;
            if document_contains_storefront(document) {
                request.data.insert(StorefrontOperationRequested);
            }
        }
        Ok(request)
    }

    async fn execute(
        &self,
        ctx: &ExtensionContext<'_>,
        operation_name: Option<&str>,
        next: NextExecute<'_>,
    ) -> Response {
        let service_principal = ctx
            .data_opt::<AuthContext>()
            .is_some_and(AuthContext::is_service_principal);
        if service_principal && ctx.data_opt::<StorefrontOperationRequested>().is_some() {
            tracing::warn!(
                operation_name = ?operation_name,
                "Rejected GraphQL storefront operation authenticated as a service principal"
            );
            return Response::from_errors(vec![
                <FieldError as GraphQLError>::permission_denied(
                    "Storefront GraphQL operations accept anonymous guests or human-user credentials, not service credentials",
                )
                .into_server_error(Pos::default()),
            ]);
        }

        next.run(ctx, operation_name).await
    }
}

#[cfg(test)]
mod tests {
    use super::document_contains_storefront;

    #[test]
    fn detects_storefront_queries_mutations_aliases_and_fragments() {
        for query in [
            r#"query { storefrontMe { id } }"#,
            r#"mutation { createStorefrontCart(input: {}) { cart { id } } }"#,
            r#"query { customer: storefrontOrder(id: "00000000-0000-0000-0000-000000000001") { id } }"#,
            r#"
                mutation Checkout {
                    ...CheckoutFields
                }
                fragment CheckoutFields on Mutation {
                    completeStorefrontCheckout(
                        idempotencyKey: "key"
                        input: { cartId: "00000000-0000-0000-0000-000000000001" }
                    ) { order { id } }
                }
            "#,
        ] {
            let document = async_graphql::parser::parse_query(query).expect("query should parse");
            assert!(document_contains_storefront(&document));
        }
    }

    #[test]
    fn leaves_admin_commerce_operations_available() {
        let document = async_graphql::parser::parse_query(
            r#"
                mutation {
                    createShippingOption(
                        tenantId: "00000000-0000-0000-0000-000000000001"
                        input: { translations: [], currencyCode: "USD", amount: "10" }
                    ) { id }
                }
            "#,
        )
        .expect("query should parse");

        assert!(!document_contains_storefront(&document));
    }
}
