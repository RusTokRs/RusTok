use async_graphql::Positioned;
use async_graphql::parser::types::{ExecutableDocument, OperationDefinition};

/// Return only the GraphQL operation selected by the request.
///
/// Security extensions must not authorize or reject operations that the
/// GraphQL executor will not run. A missing operation name selects the sole
/// operation only; multi-operation documents without an operation name are
/// invalid at execution and therefore have no selected operation here.
pub(crate) fn selected_operations<'a>(
    document: &'a ExecutableDocument,
    operation_name: Option<&str>,
) -> Vec<&'a Positioned<OperationDefinition>> {
    let mut operations = document.operations.iter();

    match operation_name {
        // OperationDefinition has no name field; the parser stores operation
        // names as keys of DocumentOperations::Multiple.
        Some(selected) => operations
            .filter_map(|(name, operation)| {
                name.is_some_and(|name| name.as_str() == selected)
                    .then_some(operation)
            })
            .collect(),
        None => match (operations.next(), operations.next()) {
            (Some((_, only)), None) => vec![only],
            _ => Vec::new(),
        },
    }
}

mod artifact_lifecycle;
pub mod auth_rate_limit;
#[cfg(feature = "mod-blog")]
pub mod blog_rate_limit;
pub mod dashboard_security;
pub mod forum_principal_security;
pub mod index_drift_diagnosis;
pub mod index_drift_source_page_diagnosis;
pub mod index_replay;
#[cfg(test)]
mod index_replay_locale_tests;
#[cfg(test)]
mod index_replay_shutdown_tests;
pub mod legacy_disable_user;
pub mod loaders;
#[cfg(feature = "mod-moderation")]
pub mod moderation_recovery;
pub mod module_rollback;
pub mod module_security;
pub mod module_settings_cas;
pub mod mutations;
pub mod observability;
pub mod persisted;
pub mod principal_tenant_security;
#[cfg(feature = "mod-profiles")]
pub mod profile_summary_policy;
pub mod queries;
pub mod rbac_runtime;
pub mod schema;
pub mod search_rate_limit;
pub mod security;
pub mod settings;
pub mod starter;
pub mod storefront_principal_security;
pub mod subscriptions;
pub mod system;
pub mod tenant_security;
pub mod transition_lifecycle;
pub mod types;

pub use schema::{AppSchema, GraphqlSchemaDependencies, SharedGraphqlSchema, build_schema};

#[cfg(test)]
mod operation_selection_tests {
    use super::selected_operations;

    #[test]
    fn selects_only_the_named_operation() {
        use async_graphql::parser::types::Selection;

        let document =
            async_graphql::parser::parse_query("query Public { ping } query Admin { users }")
                .expect("document should parse");

        let public = selected_operations(&document, Some("Public"));
        assert_eq!(public.len(), 1);
        assert!(matches!(
            &public[0].node.selection_set.node.items[0].node,
            Selection::Field(field) if field.node.name.node.as_str() == "ping"
        ));

        let admin = selected_operations(&document, Some("Admin"));
        assert_eq!(admin.len(), 1);
        assert!(matches!(
            &admin[0].node.selection_set.node.items[0].node,
            Selection::Field(field) if field.node.name.node.as_str() == "users"
        ));

        assert!(selected_operations(&document, Some("Unknown")).is_empty());
    }

    #[test]
    fn only_anonymous_or_single_operation_documents_need_no_name() {
        let single = async_graphql::parser::parse_query("query Public { ping }")
            .expect("document should parse");
        assert_eq!(selected_operations(&single, None).len(), 1);
        assert_eq!(selected_operations(&single, Some("Public")).len(), 1);

        let anonymous =
            async_graphql::parser::parse_query("{ ping }").expect("document should parse");
        assert_eq!(selected_operations(&anonymous, None).len(), 1);

        let multiple =
            async_graphql::parser::parse_query("query Public { ping } query Admin { users }")
                .expect("document should parse");
        assert!(selected_operations(&multiple, None).is_empty());
    }
}

#[cfg(test)]
mod parsed_document_policy_tests {
    use super::{
        dashboard_security::GraphqlDashboardSecurityPolicy,
        forum_principal_security::ForumPrincipalPolicy,
        legacy_disable_user::LegacyDisableUserPolicy,
        module_security::GraphqlModuleSecurityPolicy,
        security::GraphqlSecurityPolicy,
        storefront_principal_security::StorefrontPrincipalPolicy,
        tenant_security::GraphqlTenantPolicy,
    };
    use async_graphql::{EmptyMutation, EmptySubscription, Object, Request, Schema};
    use rustok_api::{AuthContext, TenantContext};
    use uuid::Uuid;

    struct Query;

    #[Object]
    impl Query {
        async fn ping(&self) -> bool {
            true
        }

        async fn users(&self) -> bool {
            true
        }

        async fn dashboard_stats(&self) -> bool {
            true
        }

        async fn module_registry(&self) -> bool {
            true
        }

        async fn echo_tenant(&self, tenant_id: Uuid) -> Uuid {
            tenant_id
        }

        async fn storefront_me(&self) -> bool {
            true
        }
    }

    struct Mutation;

    #[Object]
    impl Mutation {
        async fn disable_user(&self) -> bool {
            true
        }

        async fn create_forum_topic(&self) -> bool {
            true
        }
    }

    fn parsed_request(query: &str) -> Request {
        let mut request = Request::new("");
        request.set_parsed_query(async_graphql::parser::parse_query(query).unwrap());
        request
    }

    fn service_auth(tenant_id: Uuid) -> AuthContext {
        AuthContext {
            user_id: Uuid::new_v4(),
            session_id: Uuid::nil(),
            tenant_id,
            permissions: Vec::new(),
            client_id: Some(Uuid::new_v4()),
            scopes: Vec::new(),
            grant_type: "client_credentials".to_owned(),
        }
    }

    #[tokio::test]
    async fn parsed_documents_without_source_still_pass_every_security_policy() {
        let security = Schema::build(Query, EmptyMutation, EmptySubscription)
            .extension(GraphqlSecurityPolicy)
            .finish();
        let dashboard = Schema::build(Query, EmptyMutation, EmptySubscription)
            .extension(GraphqlDashboardSecurityPolicy)
            .finish();
        let modules = Schema::build(Query, EmptyMutation, EmptySubscription)
            .extension(GraphqlModuleSecurityPolicy)
            .finish();
        for (schema, query) in [
            (&security, "{ users }"),
            (&dashboard, "{ dashboardStats }"),
            (&modules, "{ moduleRegistry }"),
        ] {
            let response = schema.execute(parsed_request(query)).await;
            assert_eq!(response.errors.len(), 1, "{query}: {response:?}");
            assert!(schema.execute(parsed_request("{ ping }")).await.errors.is_empty());
        }

        let tenant_id = Uuid::new_v4();
        let tenant = TenantContext {
            id: tenant_id,
            name: "Tenant".to_owned(),
            slug: "tenant".to_owned(),
            domain: None,
            settings: serde_json::json!({}),
            default_locale: "en".to_owned(),
            is_active: true,
        };
        let tenant_schema = Schema::build(Query, EmptyMutation, EmptySubscription)
            .extension(GraphqlTenantPolicy)
            .finish();
        let foreign = Uuid::new_v4();
        let denied = tenant_schema
            .execute(parsed_request(&format!("{{ echoTenant(tenantId: \"{foreign}\") }}")).data(tenant.clone()))
            .await;
        assert_eq!(denied.errors.len(), 1);
        let allowed = tenant_schema
            .execute(parsed_request(&format!("{{ echoTenant(tenantId: \"{tenant_id}\") }}")).data(tenant))
            .await;
        assert!(allowed.errors.is_empty());

        let storefront = Schema::build(Query, Mutation, EmptySubscription)
            .extension(StorefrontPrincipalPolicy)
            .finish();
        let forum = Schema::build(Query, Mutation, EmptySubscription)
            .extension(ForumPrincipalPolicy)
            .finish();
        let legacy = Schema::build(Query, Mutation, EmptySubscription)
            .extension(LegacyDisableUserPolicy)
            .finish();
        for (schema, query) in [
            (&storefront, "{ storefrontMe }"),
            (&forum, "mutation { createForumTopic }"),
            (&legacy, "mutation { disableUser }"),
        ] {
            let response = schema
                .execute(parsed_request(query).data(service_auth(tenant_id)))
                .await;
            assert_eq!(response.errors.len(), 1, "{query}: {response:?}");
            assert!(schema.execute(parsed_request("{ ping }")).await.errors.is_empty());
        }
    }
}
