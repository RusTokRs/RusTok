use std::sync::Arc;

use async_graphql::{
    ErrorExtensionValues, Response, ServerError,
    extensions::{Extension, ExtensionContext, ExtensionFactory, NextExecute},
};

use crate::error::ForumError;

/// Adds the stable Forum domain error contract to GraphQL resolver failures.
///
/// `async-graphql` preserves errors converted through `?` in
/// `ServerError::source`. This extension uses that source to recover the exact
/// `ForumError::stable_code()` and retryability without requiring every
/// resolver to repeat transport mapping logic. A message fallback covers older
/// resolvers that manually constructed `async_graphql::Error` from the already
/// redacted `ForumError::Display` text.
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
        .or_else(|| contract_from_safe_message(&error.message));

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
    use async_graphql::{Error, ErrorExtensionValues, Pos, ServerError, Value};

    use super::annotate_forum_error;
    use crate::error::ForumError;

    fn extension_value<'a>(error: &'a ServerError, name: &str) -> Option<&'a Value> {
        error
            .extensions
            .as_ref()
            .and_then(|extensions| extensions.get(name))
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
        let mut server_error = ServerError::new("Topic is locked", None);

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
        let mut server_error = ServerError::new("Topic is locked", None);
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
    fn ignores_unrelated_graphql_errors() {
        let mut server_error = ServerError::new("Unrelated module failure", None);

        annotate_forum_error(&mut server_error);

        assert!(server_error.extensions.is_none());
    }
}
