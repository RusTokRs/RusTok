use async_graphql::parser::types::{ExecutableDocument, OperationDefinition};
use async_graphql::Positioned;

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
    let operations = document
        .operations
        .iter()
        .map(|(_, operation)| operation)
        .collect::<Vec<_>>();

    match operation_name {
        Some(selected) => operations
            .into_iter()
            .filter(|operation| {
                operation
                    .node
                    .name
                    .as_ref()
                    .is_some_and(|name| name.node.as_str() == selected)
            })
            .collect(),
        None if operations.len() == 1 => operations,
        None => Vec::new(),
    }
}

mod artifact_lifecycle;
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
        let document = async_graphql::parser::parse_query(
            "query Public { ping } query Admin { users }",
        )
        .expect("document should parse");

        let public = selected_operations(&document, Some("Public"));
        assert_eq!(public.len(), 1);
        assert_eq!(
            public[0]
                .node
                .name
                .as_ref()
                .map(|name| name.node.as_str()),
            Some("Public")
        );

        let admin = selected_operations(&document, Some("Admin"));
        assert_eq!(admin.len(), 1);
        assert_eq!(
            admin[0]
                .node
                .name
                .as_ref()
                .map(|name| name.node.as_str()),
            Some("Admin")
        );
    }

    #[test]
    fn only_anonymous_or_single_operation_documents_need_no_name() {
        let single = async_graphql::parser::parse_query("query Public { ping }")
            .expect("document should parse");
        assert_eq!(selected_operations(&single, None).len(), 1);
        assert_eq!(selected_operations(&single, Some("Public")).len(), 1);

        let anonymous = async_graphql::parser::parse_query("{ ping }")
            .expect("document should parse");
        assert_eq!(selected_operations(&anonymous, None).len(), 1);

        let multiple = async_graphql::parser::parse_query(
            "query Public { ping } query Admin { users }",
        )
        .expect("document should parse");
        assert!(selected_operations(&multiple, None).is_empty());
    }
}

