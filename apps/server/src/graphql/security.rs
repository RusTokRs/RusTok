use std::collections::BTreeSet;
use std::sync::Arc;

use async_graphql::extensions::{
    Extension, ExtensionContext, ExtensionFactory, NextExecute, NextPrepareRequest,
};
use async_graphql::parser::types::{ExecutableDocument, OperationType, Selection, SelectionSet};
use async_graphql::{FieldError, Pos, Request, Response, ServerResult};
use rustok_api::Permission;

use crate::context::AuthContext;
use rustok_api::graphql::GraphQLError;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum SensitiveGraphqlField {
    User,
    Users,
    CreateUser,
    UpdateUser,
    DisableUser,
    DeleteUser,
}

impl SensitiveGraphqlField {
    fn from_root_field(operation_type: OperationType, field_name: &str) -> Option<Self> {
        match (operation_type, field_name) {
            (OperationType::Query, "user") => Some(Self::User),
            (OperationType::Query, "users") => Some(Self::Users),
            (OperationType::Mutation, "createUser") => Some(Self::CreateUser),
            (OperationType::Mutation, "updateUser") => Some(Self::UpdateUser),
            (OperationType::Mutation, "disableUser") => Some(Self::DisableUser),
            (OperationType::Mutation, "deleteUser") => Some(Self::DeleteUser),
            _ => None,
        }
    }

    fn field_name(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Users => "users",
            Self::CreateUser => "createUser",
            Self::UpdateUser => "updateUser",
            Self::DisableUser => "disableUser",
            Self::DeleteUser => "deleteUser",
        }
    }

    fn permission_hint(self) -> &'static str {
        match self {
            Self::User => "users:read (or users:manage)",
            Self::Users => "users:list (or users:manage)",
            Self::CreateUser => "users:create (or users:manage)",
            Self::UpdateUser => "users:update (or users:manage)",
            Self::DisableUser | Self::DeleteUser => "users:manage",
        }
    }

    fn allows(self, permissions: &[Permission]) -> bool {
        let required_permission = match self {
            Self::User => Permission::USERS_READ,
            Self::Users => Permission::USERS_LIST,
            Self::CreateUser => Permission::USERS_CREATE,
            Self::UpdateUser => Permission::USERS_UPDATE,
            Self::DisableUser | Self::DeleteUser => Permission::USERS_MANAGE,
        };

        rustok_api::has_effective_permission(permissions, &required_permission)
    }
}

#[derive(Clone, Debug)]
struct SensitiveGraphqlDocumentPolicy(Vec<SensitiveGraphqlField>);

fn collect_sensitive_fields_from_selection_set(
    operation_type: OperationType,
    selection_set: &SelectionSet,
    document: &ExecutableDocument,
    fields: &mut BTreeSet<SensitiveGraphqlField>,
    visited_fragments: &mut BTreeSet<String>,
) {
    for selection in &selection_set.items {
        match &selection.node {
            Selection::Field(field) => {
                if let Some(sensitive_field) = SensitiveGraphqlField::from_root_field(
                    operation_type,
                    field.node.name.node.as_str(),
                ) {
                    fields.insert(sensitive_field);
                }
            }
            Selection::FragmentSpread(fragment) => {
                let fragment_name = &fragment.node.fragment_name.node;
                if !visited_fragments.insert(fragment_name.to_string()) {
                    continue;
                }

                if let Some(definition) = document.fragments.get(fragment_name) {
                    collect_sensitive_fields_from_selection_set(
                        operation_type,
                        &definition.node.selection_set.node,
                        document,
                        fields,
                        visited_fragments,
                    );
                }
            }
            Selection::InlineFragment(fragment) => collect_sensitive_fields_from_selection_set(
                operation_type,
                &fragment.node.selection_set.node,
                document,
                fields,
                visited_fragments,
            ),
        }
    }
}

fn sensitive_graphql_fields(document: &ExecutableDocument) -> Vec<SensitiveGraphqlField> {
    let mut fields = BTreeSet::new();

    for (_, operation) in document.operations.iter() {
        let mut visited_fragments = BTreeSet::new();
        collect_sensitive_fields_from_selection_set(
            operation.node.ty,
            &operation.node.selection_set.node,
            document,
            &mut fields,
            &mut visited_fragments,
        );
    }

    fields.into_iter().collect()
}

fn classify_sensitive_graphql_document(request: &mut Request) -> ServerResult<()> {
    if request.query.trim().is_empty() {
        return Ok(());
    }

    let document = request.parsed_query()?;
    let sensitive_fields = sensitive_graphql_fields(document);
    if !sensitive_fields.is_empty() {
        request
            .data
            .insert(SensitiveGraphqlDocumentPolicy(sensitive_fields));
    }

    Ok(())
}

fn unauthorized_sensitive_fields(
    fields: &[SensitiveGraphqlField],
    auth: Option<&AuthContext>,
) -> Vec<SensitiveGraphqlField> {
    let Some(auth) = auth else {
        return fields.to_vec();
    };

    fields
        .iter()
        .copied()
        .filter(|field| !field.allows(&auth.permissions))
        .collect()
}

fn forbidden_sensitive_response(fields: &[SensitiveGraphqlField]) -> Response {
    let required_permissions = fields
        .iter()
        .map(|field| format!("{} -> {}", field.field_name(), field.permission_hint()))
        .collect::<Vec<_>>()
        .join(", ");
    Response::from_errors(vec![
        <FieldError as GraphQLError>::permission_denied(&format!(
            "Forbidden admin GraphQL operation. Required permissions: {required_permissions}"
        ))
        .into_server_error(Pos::default()),
    ])
}

#[derive(Default)]
pub struct GraphqlSecurityPolicy;

impl ExtensionFactory for GraphqlSecurityPolicy {
    fn create(&self) -> Arc<dyn Extension> {
        Arc::new(GraphqlSecurityPolicyExtension)
    }
}

struct GraphqlSecurityPolicyExtension;

#[async_trait::async_trait]
impl Extension for GraphqlSecurityPolicyExtension {
    async fn prepare_request(
        &self,
        ctx: &ExtensionContext<'_>,
        request: Request,
        next: NextPrepareRequest<'_>,
    ) -> ServerResult<Request> {
        let mut request = next.run(ctx, request).await?;
        classify_sensitive_graphql_document(&mut request)?;
        Ok(request)
    }

    async fn execute(
        &self,
        ctx: &ExtensionContext<'_>,
        operation_name: Option<&str>,
        next: NextExecute<'_>,
    ) -> Response {
        if let Some(policy) = ctx.data_opt::<SensitiveGraphqlDocumentPolicy>() {
            let denied_fields = unauthorized_sensitive_fields(&policy.0, ctx.data_opt());
            if !denied_fields.is_empty() {
                tracing::warn!(
                    denied_fields = ?denied_fields
                        .iter()
                        .map(|field| field.field_name())
                        .collect::<Vec<_>>(),
                    operation_name = ?operation_name,
                    "Rejected sensitive GraphQL document before resolver execution"
                );
                return forbidden_sensitive_response(&denied_fields);
            }
        }

        next.run(ctx, operation_name).await
    }
}

#[cfg(test)]
mod tests {
    use super::{
        GraphqlSecurityPolicy, SensitiveGraphqlDocumentPolicy, SensitiveGraphqlField,
        classify_sensitive_graphql_document, sensitive_graphql_fields,
    };
    use crate::context::AuthContext;
    use async_graphql::{EmptySubscription, Object, Request, Schema, SimpleObject};
    use rustok_api::Permission;
    use serde_json::json;
    use uuid::Uuid;

    struct QueryRoot;

    #[derive(SimpleObject)]
    struct UserView {
        id: i32,
    }

    #[Object]
    impl QueryRoot {
        async fn ping(&self) -> &str {
            "pong"
        }

        async fn user(&self) -> UserView {
            UserView { id: 1 }
        }

        async fn users(&self) -> Vec<i32> {
            vec![1, 2, 3]
        }
    }

    struct MutationRoot;

    #[Object]
    impl MutationRoot {
        async fn create_user(&self) -> bool {
            true
        }
    }

    fn auth_context(permissions: Vec<Permission>) -> AuthContext {
        AuthContext {
            user_id: Uuid::new_v4(),
            session_id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            permissions,
            client_id: None,
            scopes: Vec::new(),
            grant_type: "password".to_string(),
        }
    }

    #[test]
    fn classifies_sensitive_fields_inside_root_fragments() {
        let mut request = Request::new(
            r#"
                query UserAdmin {
                    ...RootFields
                }

                fragment RootFields on Query {
                    user {
                        id
                    }
                }
            "#,
        );

        classify_sensitive_graphql_document(&mut request).expect("query should parse");

        let policy = request
            .data
            .get(&std::any::TypeId::of::<SensitiveGraphqlDocumentPolicy>())
            .and_then(|value| value.downcast_ref::<SensitiveGraphqlDocumentPolicy>())
            .expect("sensitive policy should be attached");

        assert_eq!(policy.0, vec![SensitiveGraphqlField::User]);
    }

    #[test]
    fn classifies_sensitive_fields_for_all_operations_in_document() {
        let mut request = Request::new(
            r#"
                query PublicInfo {
                    ping
                }

                query SensitiveUsers {
                    users
                }
            "#,
        )
        .operation_name("PublicInfo");

        classify_sensitive_graphql_document(&mut request).expect("query should parse");

        let policy = request
            .data
            .get(&std::any::TypeId::of::<SensitiveGraphqlDocumentPolicy>())
            .and_then(|value| value.downcast_ref::<SensitiveGraphqlDocumentPolicy>())
            .expect("sensitive policy should be attached");

        assert_eq!(policy.0, vec![SensitiveGraphqlField::Users]);
    }

    #[test]
    fn collects_sensitive_fields_from_document_without_operation_name_filter() {
        let document = async_graphql::parser::parse_query(
            r#"
                mutation CreateAccount {
                    createUser
                }

                query ListUsers {
                    users
                }
            "#,
        )
        .expect("query should parse");

        assert_eq!(
            sensitive_graphql_fields(&document),
            vec![
                SensitiveGraphqlField::Users,
                SensitiveGraphqlField::CreateUser,
            ]
        );
    }

    #[test]
    fn manage_permission_is_effective_for_user_read_and_list_fields() {
        assert!(SensitiveGraphqlField::User.allows(&[Permission::USERS_MANAGE]));
        assert!(SensitiveGraphqlField::Users.allows(&[Permission::USERS_MANAGE]));
        assert!(SensitiveGraphqlField::CreateUser.allows(&[Permission::USERS_MANAGE]));
        assert!(SensitiveGraphqlField::UpdateUser.allows(&[Permission::USERS_MANAGE]));
        assert!(SensitiveGraphqlField::DisableUser.allows(&[Permission::USERS_MANAGE]));
        assert!(SensitiveGraphqlField::DeleteUser.allows(&[Permission::USERS_MANAGE]));
    }

    #[test]
    fn cyclic_fragments_are_classified_without_unbounded_recursion() {
        let mut request = Request::new(
            r#"
                query Cyclic {
                    ...RootFields
                }

                fragment RootFields on Query {
                    ...RootFields
                    user {
                        id
                    }
                }
            "#,
        );

        classify_sensitive_graphql_document(&mut request).expect("query should parse");

        let policy = request
            .data
            .get(&std::any::TypeId::of::<SensitiveGraphqlDocumentPolicy>())
            .and_then(|value| value.downcast_ref::<SensitiveGraphqlDocumentPolicy>())
            .expect("sensitive policy should be attached");

        assert_eq!(policy.0, vec![SensitiveGraphqlField::User]);
    }

    #[test]
    fn sensitive_user_fields_map_to_auth_module_permission_surface() {
        let auth_permissions = rustok_auth::AUTH_USER_PERMISSIONS;

        for (field, required_permission) in [
            (SensitiveGraphqlField::User, Permission::USERS_READ),
            (SensitiveGraphqlField::Users, Permission::USERS_LIST),
            (SensitiveGraphqlField::CreateUser, Permission::USERS_CREATE),
            (SensitiveGraphqlField::UpdateUser, Permission::USERS_UPDATE),
            (SensitiveGraphqlField::DisableUser, Permission::USERS_MANAGE),
            (SensitiveGraphqlField::DeleteUser, Permission::USERS_MANAGE),
        ] {
            assert!(
                auth_permissions.contains(&required_permission),
                "auth module must publish {} for {:?}",
                required_permission,
                field
            );
            assert!(
                field.allows(&[required_permission]),
                "{:?} should allow {}",
                field,
                required_permission
            );
        }
    }

    #[tokio::test]
    async fn blocks_sensitive_query_without_auth_context() {
        let schema = Schema::build(QueryRoot, MutationRoot, EmptySubscription)
            .extension(GraphqlSecurityPolicy)
            .finish();

        let response = schema.execute(Request::new("query { users }")).await;

        assert_eq!(response.errors.len(), 1);
        assert!(response.errors[0].message.contains("users:list"));
    }

    #[tokio::test]
    async fn allows_sensitive_query_with_matching_permissions() {
        let schema = Schema::build(QueryRoot, MutationRoot, EmptySubscription)
            .extension(GraphqlSecurityPolicy)
            .finish();

        let response = schema
            .execute(
                Request::new("query { users }").data(auth_context(vec![Permission::USERS_LIST])),
            )
            .await;

        assert!(response.errors.is_empty());
        assert_eq!(
            response
                .data
                .into_json()
                .expect("response should serialize"),
            json!({
                "users": [1, 2, 3]
            })
        );
    }

    #[tokio::test]
    async fn rejects_document_even_when_public_operation_is_selected() {
        let schema = Schema::build(QueryRoot, MutationRoot, EmptySubscription)
            .extension(GraphqlSecurityPolicy)
            .finish();

        let response = schema
            .execute(
                Request::new(
                    r#"
                        query PublicInfo {
                            ping
                        }

                        query SensitiveUsers {
                            users
                        }
                    "#,
                )
                .operation_name("PublicInfo"),
            )
            .await;

        assert_eq!(response.errors.len(), 1);
        assert!(response.errors[0].message.contains("users:list"));
    }
}
