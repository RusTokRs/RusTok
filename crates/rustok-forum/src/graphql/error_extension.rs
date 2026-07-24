use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use async_graphql::{
    ErrorExtensionValues, FieldError, Name, PathSegment, Pos, Request, Response, ServerError,
    ServerResult, Variables,
    extensions::{
        Extension, ExtensionContext, ExtensionFactory, NextExecute, NextPrepareRequest,
    },
    parser::types::{ExecutableDocument, OperationDefinition, OperationType, Selection, SelectionSet},
};
use async_graphql_value::{ConstValue, Value, from_value};
use rustok_api::graphql::{GraphQLError, PaginationInput};
use serde::Deserialize;

use crate::error::ForumError;

const PAGE_BACKED_FORUM_QUERY_FIELDS: [&str; 6] = [
    "forumCategories",
    "forumTopics",
    "forumReplies",
    "forumStorefrontCategories",
    "forumStorefrontTopics",
    "forumStorefrontReplies",
];
const PAGE_BOUNDARY_ERROR: &str = "Forum pagination window must start on a page boundary";

/// Adds the stable Forum domain error contract to GraphQL resolver failures and
/// rejects pagination windows that cannot be represented by the current
/// page-backed Forum services.
///
/// `async-graphql` preserves errors converted through `?` in
/// `ServerError::source`. This extension uses that source to recover the exact
/// `ForumError::stable_code()` and retryability without requiring every
/// resolver to repeat transport mapping logic. A path-scoped message fallback
/// covers older Forum resolvers that manually constructed
/// `async_graphql::Error` from the already redacted `ForumError::Display` text.
///
/// Forum list services currently accept `(page, per_page)`, while the GraphQL
/// contract accepts arbitrary offsets and cursors. An offset that is not a
/// multiple of the normalized limit would otherwise be rounded down by integer
/// division and return rows from the wrong window. The request policy resolves
/// literal, variable and defaulted pagination inputs, delegates normalization
/// to the shared `PaginationInput`, and fails closed before any resolver or
/// database read runs.
#[derive(Default)]
pub struct ForumGraphqlErrorExtension;

impl ExtensionFactory for ForumGraphqlErrorExtension {
    fn create(&self) -> Arc<dyn Extension> {
        Arc::new(ForumGraphqlErrorExtensionInstance)
    }
}

struct ForumGraphqlErrorExtensionInstance;

#[async_trait::async_trait]
impl Extension for ForumGraphqlErrorExtensionInstance {
    async fn prepare_request(
        &self,
        ctx: &ExtensionContext<'_>,
        request: Request,
        next: NextPrepareRequest<'_>,
    ) -> ServerResult<Request> {
        let mut request = next.run(ctx, request).await?;
        validate_forum_pagination_request(&mut request)?;
        Ok(request)
    }

    async fn execute(
        &self,
        ctx: &ExtensionContext<'_>,
        operation_name: Option<&str>,
        next: NextExecute<'_>,
    ) -> Response {
        let mut response = next.run(ctx, operation_name).await;
        for error in &mut response.errors {
            annotate_forum_error(error);
        }
        response
    }
}

#[derive(Debug, Deserialize)]
#[serde(default)]
struct ForumPaginationArguments {
    offset: i64,
    limit: i64,
    first: Option<i64>,
    last: Option<i64>,
    after: Option<String>,
    before: Option<String>,
}

impl Default for ForumPaginationArguments {
    fn default() -> Self {
        Self {
            offset: 0,
            limit: 20,
            first: None,
            last: None,
            after: None,
            before: None,
        }
    }
}

impl From<ForumPaginationArguments> for PaginationInput {
    fn from(value: ForumPaginationArguments) -> Self {
        Self {
            offset: value.offset,
            limit: value.limit,
            first: value.first,
            last: value.last,
            after: value.after,
            before: value.before,
        }
    }
}

fn validate_forum_pagination_request(request: &mut Request) -> ServerResult<()> {
    if request.query.trim().is_empty() {
        return Ok(());
    }

    // Clone variables before borrowing the parsed document from Request.
    let variables = request.variables.clone();
    let error = {
        let document = request.parsed_query()?;
        forum_pagination_error(document, &variables)
    };

    match error {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

fn forum_pagination_error(
    document: &ExecutableDocument,
    variables: &Variables,
) -> Option<ServerError> {
    for (_, operation) in document.operations.iter() {
        if operation.node.ty != OperationType::Query {
            continue;
        }

        let defaults = operation_variable_defaults(&operation.node);
        let mut visited_fragments = HashSet::new();
        if let Some(error) = selection_set_pagination_error(
            &operation.node.selection_set.node,
            document,
            variables,
            &defaults,
            &mut visited_fragments,
        ) {
            return Some(error);
        }
    }

    None
}

fn operation_variable_defaults(operation: &OperationDefinition) -> HashMap<Name, ConstValue> {
    operation
        .variable_definitions
        .iter()
        .filter_map(|definition| {
            definition
                .node
                .default_value()
                .cloned()
                .map(|value| (definition.node.name.node.clone(), value))
        })
        .collect()
}

fn pagination_value_error(
    value: &Value,
    position: Pos,
    variables: &Variables,
    defaults: &HashMap<Name, ConstValue>,
) -> Option<ServerError> {
    let resolved = value.clone().into_const_with(|name| {
        resolve_variable(&name, variables, defaults).cloned().ok_or(())
    });
    let Ok(resolved) = resolved else {
        // GraphQL validation owns missing or incompatible variables.
        return None;
    };
    if resolved == ConstValue::Null {
        return None;
    }

    let Ok(arguments) = from_value::<ForumPaginationArguments>(resolved) else {
        // Schema validation owns malformed input objects and scalar types.
        return None;
    };
    let pagination_input: PaginationInput = arguments.into();
    let (offset, limit) = match pagination_input.normalize() {
        Ok(window) => window,
        Err(error) => return Some(error.into_server_error(position)),
    };

    (offset % limit != 0).then(|| {
        <FieldError as GraphQLError>::bad_user_input(PAGE_BOUNDARY_ERROR)
            .into_server_error(position)
    })
}

fn selection_set_pagination_error(
    selection_set: &SelectionSet,
    document: &ExecutableDocument,
    variables: &Variables,
    defaults: &HashMap<Name, ConstValue>,
    visited_fragments: &mut HashSet<Name>,
) -> Option<ServerError> {
    for selection in &selection_set.items {
        match &selection.node {
            Selection::Field(field) => {
                if !is_page_backed_forum_query(field.node.name.node.as_str()) {
                    continue;
                }

                let pagination = field
                    .node
                    .arguments
                    .iter()
                    .find(|(name, _)| name.node.as_str() == "pagination")
                    .map(|(_, value)| value);
                let Some(pagination) = pagination else {
                    continue;
                };

                if let Some(error) = pagination_value_error(
                    &pagination.node,
                    pagination.pos,
                    variables,
                    defaults,
                ) {
                    return Some(error);
                }
            }
            Selection::FragmentSpread(fragment) => {
                let fragment_name = fragment.node.fragment_name.node.clone();
                if visited_fragments.insert(fragment_name.clone()) {
                    if let Some(definition) = document.fragments.get(&fragment_name) {
                        if let Some(error) = selection_set_pagination_error(
                            &definition.node.selection_set.node,
                            document,
                            variables,
                            defaults,
                            visited_fragments,
                        ) {
                            return Some(error);
                        }
                    }
                }
            }
            Selection::InlineFragment(fragment) => {
                if let Some(error) = selection_set_pagination_error(
                    &fragment.node.selection_set.node,
                    document,
                    variables,
                    defaults,
                    visited_fragments,
                ) {
                    return Some(error);
                }
            }
        }
    }

    None
}

fn resolve_variable<'a>(
    name: &Name,
    variables: &'a Variables,
    defaults: &'a HashMap<Name, ConstValue>,
) -> Option<&'a ConstValue> {
    variables.get(name).or_else(|| defaults.get(name))
}

fn is_page_backed_forum_query(field_name: &str) -> bool {
    PAGE_BACKED_FORUM_QUERY_FIELDS.contains(&field_name)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ForumErrorContract {
    code: &'static str,
    retryable: Option<bool>,
}

fn annotate_forum_error(error: &mut ServerError) {
    if error
        .extensions
        .as_ref()
        .is_some_and(|extensions| extensions.get("code").is_some())
    {
        return;
    }

    let contract = error
        .source::<ForumError>()
        .map(|source| ForumErrorContract {
            code: source.stable_code(),
            retryable: Some(source.is_retryable()),
        })
        .or_else(|| {
            is_forum_graphql_path(error)
                .then(|| contract_from_safe_message(&error.message))
                .flatten()
        });

    let Some(contract) = contract else {
        return;
    };

    let extensions = error
        .extensions
        .get_or_insert_with(ErrorExtensionValues::default);
    extensions.set("code", contract.code);
    if let Some(retryable) = contract.retryable {
        extensions.set("retryable", retryable);
    }
}

fn is_forum_graphql_path(error: &ServerError) -> bool {
    error.path.iter().any(|segment| {
        matches!(
            segment,
            PathSegment::Field(field)
                if field.starts_with("forum") || field.contains("Forum")
        )
    })
}

fn contract_from_safe_message(message: &str) -> Option<ForumErrorContract> {
    let contract = match message {
        "Topic is closed" => ("FORUM_TOPIC_CLOSED", Some(false)),
        "Topic is archived" => ("FORUM_TOPIC_ARCHIVED", Some(false)),
        "Topic is locked" => ("FORUM_TOPIC_LOCKED", Some(false)),
        "Topic is deleted" => ("FORUM_TOPIC_DELETED", Some(false)),
        "Reply is deleted" => ("FORUM_REPLY_DELETED", Some(false)),
        "Forum mention target is unavailable" => {
            ("FORUM_MENTION_TARGET_UNAVAILABLE", Some(false))
        }
        "Forum quote target is unavailable" => {
            ("FORUM_QUOTE_TARGET_UNAVAILABLE", Some(false))
        }
        "Forum relation revision is unavailable" => {
            ("FORUM_RELATION_REVISION_UNAVAILABLE", Some(false))
        }
        "Forum relation revision changed concurrently" => {
            ("FORUM_RELATION_REVISION_CONFLICT", Some(true))
        }
        "Forum persistence operation failed" => ("FORUM_INTERNAL_ERROR", Some(true)),
        "Forum content operation failed" => ("FORUM_INTERNAL_ERROR", Some(false)),
        "Forum internal operation failed" => ("FORUM_INTERNAL_ERROR", Some(true)),
        "Forum capability operation failed" => ("FORUM_CAPABILITY_FAILURE", None),
        _ if message.starts_with("Category not found: ") => {
            ("FORUM_CATEGORY_NOT_FOUND", Some(false))
        }
        _ if message.starts_with("Topic not found: ") => {
            ("FORUM_TOPIC_NOT_FOUND", Some(false))
        }
        _ if message.starts_with("Reply not found: ") => {
            ("FORUM_REPLY_NOT_FOUND", Some(false))
        }
        _ if message.starts_with("Topic solution not found for topic: ") => {
            ("FORUM_SOLUTION_NOT_FOUND", Some(false))
        }
        _ if message.starts_with("Validation error: ") => {
            ("FORUM_VALIDATION_FAILED", Some(false))
        }
        _ if message.starts_with("Forbidden: ") => ("FORUM_FORBIDDEN", Some(false)),
        _ if message.starts_with("Required capability `") => {
            ("FORUM_CAPABILITY_UNAVAILABLE", Some(false))
        }
        _ if message.starts_with("Invalid topic status transition: ") => {
            ("FORUM_TOPIC_TRANSITION_INVALID", Some(false))
        }
        _ if message.starts_with("Invalid reply status transition: ") => {
            ("FORUM_REPLY_TRANSITION_INVALID", Some(false))
        }
        _ => return None,
    };

    Some(ForumErrorContract {
        code: contract.0,
        retryable: contract.1,
    })
}

#[cfg(test)]
mod tests {
    use async_graphql::{
        EmptyMutation, EmptySubscription, Error, ErrorExtensionValues, Object, PathSegment, Pos,
        Request, Schema, ServerError, Value, Variables,
    };
    use rustok_api::graphql::{PaginationInput, encode_cursor};
    use serde_json::json;

    use super::{ForumGraphqlErrorExtension, PAGE_BOUNDARY_ERROR, annotate_forum_error};
    use crate::error::ForumError;

    struct Query;

    #[Object]
    impl Query {
        async fn forum_categories(
            &self,
            #[graphql(default)] pagination: PaginationInput,
        ) -> i64 {
            pagination.offset
        }

        async fn other_items(
            &self,
            #[graphql(default)] pagination: PaginationInput,
        ) -> i64 {
            pagination.offset
        }
    }

    fn schema() -> Schema<Query, EmptyMutation, EmptySubscription> {
        Schema::build(Query, EmptyMutation, EmptySubscription)
            .extension(ForumGraphqlErrorExtension)
            .finish()
    }

    fn extension_value<'a>(error: &'a ServerError, name: &str) -> Option<&'a Value> {
        error
            .extensions
            .as_ref()
            .and_then(|extensions| extensions.get(name))
    }

    fn forum_path(error: ServerError) -> ServerError {
        error.with_path(vec![PathSegment::Field("forumTopic".to_string())])
    }

    #[tokio::test]
    async fn rejects_unaligned_literal_forum_pagination_before_resolver_execution() {
        let response = schema()
            .execute("{ forumCategories(pagination: { offset: 5, limit: 25 }) }")
            .await;

        assert_eq!(response.errors.len(), 1);
        assert_eq!(response.errors[0].message, PAGE_BOUNDARY_ERROR);
        assert_eq!(
            extension_value(&response.errors[0], "code"),
            Some(&Value::from("BAD_USER_INPUT"))
        );
    }

    #[tokio::test]
    async fn rejects_unaligned_cursor_pagination_from_variables() {
        let response = schema()
            .execute(
                Request::new(
                    "query Forum($pagination: PaginationInput) { forumCategories(pagination: $pagination) }",
                )
                .variables(Variables::from_json(json!({
                    "pagination": {
                        "first": 25,
                        "after": encode_cursor(4)
                    }
                }))),
            )
            .await;

        assert_eq!(response.errors.len(), 1);
        assert_eq!(response.errors[0].message, PAGE_BOUNDARY_ERROR);
        assert_eq!(
            extension_value(&response.errors[0], "code"),
            Some(&Value::from("BAD_USER_INPUT"))
        );
    }

    #[tokio::test]
    async fn rejects_unaligned_pagination_inside_root_fragment() {
        let response = schema()
            .execute(
                "query Forum { ...ForumPage } fragment ForumPage on Query { forumCategories(pagination: { offset: 3, limit: 20 }) }",
            )
            .await;

        assert_eq!(response.errors.len(), 1);
        assert_eq!(response.errors[0].message, PAGE_BOUNDARY_ERROR);
    }

    #[tokio::test]
    async fn accepts_aligned_forum_pagination_and_ignores_unrelated_fields() {
        let aligned = schema()
            .execute("{ forumCategories(pagination: { offset: 25, limit: 25 }) }")
            .await;
        assert!(aligned.errors.is_empty());
        assert_eq!(
            aligned.data.into_json().expect("data should serialize")["forumCategories"],
            25
        );

        let unrelated = schema()
            .execute("{ otherItems(pagination: { offset: 5, limit: 25 }) }")
            .await;
        assert!(unrelated.errors.is_empty());
        assert_eq!(
            unrelated.data.into_json().expect("data should serialize")["otherItems"],
            5
        );
    }

    #[tokio::test]
    async fn honors_defaulted_pagination_variables() {
        let response = schema()
            .execute(Request::new(format!(
                "query Forum($pagination: PaginationInput = {{ first: 25, after: \"{}\" }}) {{ forumCategories(pagination: $pagination) }}",
                encode_cursor(24)
            )))
            .await;

        assert!(response.errors.is_empty());
        assert_eq!(
            response.data.into_json().expect("data should serialize")["forumCategories"],
            0
        );
    }

    #[test]
    fn annotates_exact_domain_code_and_retryability_from_source() {
        let graphql_error: Error = ForumError::capability_unavailable(
            "profiles",
            "FORUM_PROFILES_CAPABILITY_UNAVAILABLE",
        )
        .into();
        let mut server_error = graphql_error.into_server_error(Pos::default());

        annotate_forum_error(&mut server_error);

        assert_eq!(
            extension_value(&server_error, "code"),
            Some(&Value::from("FORUM_PROFILES_CAPABILITY_UNAVAILABLE"))
        );
        assert_eq!(
            extension_value(&server_error, "retryable"),
            Some(&Value::from(false))
        );
    }

    #[test]
    fn annotates_source_less_legacy_forum_messages() {
        let mut server_error = forum_path(ServerError::new("Topic is locked", None));

        annotate_forum_error(&mut server_error);

        assert_eq!(
            extension_value(&server_error, "code"),
            Some(&Value::from("FORUM_TOPIC_LOCKED"))
        );
        assert_eq!(
            extension_value(&server_error, "retryable"),
            Some(&Value::from(false))
        );
    }

    #[test]
    fn preserves_an_existing_graphql_error_code() {
        let mut server_error = forum_path(ServerError::new("Topic is locked", None));
        let mut extensions = ErrorExtensionValues::default();
        extensions.set("code", "EXISTING_CONTRACT");
        server_error.extensions = Some(extensions);

        annotate_forum_error(&mut server_error);

        assert_eq!(
            extension_value(&server_error, "code"),
            Some(&Value::from("EXISTING_CONTRACT"))
        );
        assert_eq!(extension_value(&server_error, "retryable"), None);
    }

    #[test]
    fn ignores_unrelated_graphql_errors_with_matching_text() {
        let mut server_error = ServerError::new("Topic is locked", None)
            .with_path(vec![PathSegment::Field("updatePage".to_string())]);

        annotate_forum_error(&mut server_error);

        assert!(server_error.extensions.is_none());
    }
}
