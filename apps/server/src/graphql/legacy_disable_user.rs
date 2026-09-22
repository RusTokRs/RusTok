use std::sync::Arc;

use async_graphql::extensions::{
    Extension, ExtensionContext, ExtensionFactory, NextExecute, NextPrepareRequest,
};
use async_graphql::parser::types::{ExecutableDocument, OperationType, Selection, SelectionSet};
use async_graphql::{FieldError, Pos, Request, Response, ServerResult};
use rustok_api::graphql::GraphQLError;

#[derive(Clone, Copy, Debug)]
struct LegacyDisableUserRequested;

fn selection_contains_disable_user(
    operation_type: OperationType,
    selection_set: &SelectionSet,
    document: &ExecutableDocument,
) -> bool {
    selection_set
        .items
        .iter()
        .any(|selection| match &selection.node {
            Selection::Field(field) => {
                operation_type == OperationType::Mutation
                    && field.node.name.node.as_str() == "disableUser"
            }
            Selection::FragmentSpread(fragment) => document
                .fragments
                .get(&fragment.node.fragment_name.node)
                .is_some_and(|definition| {
                    selection_contains_disable_user(
                        operation_type,
                        &definition.node.selection_set.node,
                        document,
                    )
                }),
            Selection::InlineFragment(fragment) => selection_contains_disable_user(
                operation_type,
                &fragment.node.selection_set.node,
                document,
            ),
        })
}

fn document_contains_disable_user(document: &ExecutableDocument) -> bool {
    document.operations.iter().any(|(_, operation)| {
        selection_contains_disable_user(
            operation.node.ty,
            &operation.node.selection_set.node,
            document,
        )
    })
}

#[derive(Default)]
pub struct LegacyDisableUserPolicy;

impl ExtensionFactory for LegacyDisableUserPolicy {
    fn create(&self) -> Arc<dyn Extension> {
        Arc::new(LegacyDisableUserPolicyExtension)
    }
}

struct LegacyDisableUserPolicyExtension;

#[async_trait::async_trait]
impl Extension for LegacyDisableUserPolicyExtension {
    async fn prepare_request(
        &self,
        ctx: &ExtensionContext<'_>,
        request: Request,
        next: NextPrepareRequest<'_>,
    ) -> ServerResult<Request> {
        let mut request = next.run(ctx, request).await?;
        if !request.query.trim().is_empty() {
            let document = request.parsed_query()?;
            if document_contains_disable_user(document) {
                request.data.insert(LegacyDisableUserRequested);
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
        if ctx.data_opt::<LegacyDisableUserRequested>().is_some() {
            tracing::warn!(
                operation_name = ?operation_name,
                "Rejected legacy disableUser mutation before resolver execution"
            );
            return Response::from_errors(vec![
                <FieldError as GraphQLError>::permission_denied(
                    "The legacy disableUser mutation is disabled because it bypasses the canonical user administration boundary; use updateUser with status INACTIVE",
                )
                .into_server_error(Pos::default()),
            ]);
        }

        next.run(ctx, operation_name).await
    }
}

#[cfg(test)]
mod tests {
    use super::document_contains_disable_user;

    #[test]
    fn detects_disable_user_inside_fragments() {
        let document = async_graphql::parser::parse_query(
            r#"
                mutation DisableAccount {
                    ...DisableFields
                }

                fragment DisableFields on Mutation {
                    disableUser(id: "00000000-0000-0000-0000-000000000001") {
                        id
                    }
                }
            "#,
        )
        .expect("query should parse");

        assert!(document_contains_disable_user(&document));
    }

    #[test]
    fn allows_guarded_update_user_path() {
        let document = async_graphql::parser::parse_query(
            r#"
                mutation DisableAccount {
                    updateUser(
                        id: "00000000-0000-0000-0000-000000000001"
                        input: { status: INACTIVE }
                    ) {
                        id
                    }
                }
            "#,
        )
        .expect("query should parse");

        assert!(!document_contains_disable_user(&document));
    }
}
