use std::sync::Arc;

use async_graphql::extensions::{
    Extension, ExtensionContext, ExtensionFactory, NextExecute, NextPrepareRequest,
};
use async_graphql::parser::types::{ExecutableDocument, SelectionSet};
use async_graphql::{FieldError, Pos, Request, Response, ServerResult};
use rustok_api::{AuthContext, Permission, graphql::GraphQLError, has_effective_permission};

#[derive(Clone, Copy, Debug, Default)]
struct ForumOperationPolicy {
    human_only: bool,
    personal_projection: bool,
    topic_moderation: bool,
    reply_moderation: bool,
}

impl ForumOperationPolicy {
    fn merge(&mut self, other: Self) {
        self.human_only |= other.human_only;
        self.personal_projection |= other.personal_projection;
        self.topic_moderation |= other.topic_moderation;
        self.reply_moderation |= other.reply_moderation;
    }

    fn is_empty(self) -> bool {
        !self.human_only
            && !self.personal_projection
            && !self.topic_moderation
            && !self.reply_moderation
    }
}

fn field_policy(name: &str) -> ForumOperationPolicy {
    let human_only = matches!(
        name,
        "createForumTopic"
            | "createForumReply"
            | "createForumTopicWithQuotes"
            | "createForumReplyWithQuotes"
    ) || (name.contains("Forum")
        && ["Vote", "Subscription", "Report"]
            .iter()
            .any(|part| name.contains(part)));
    let topic_moderation = matches!(
        name,
        "updateForumTopic"
            | "deleteForumTopic"
            | "updateForumTopicWithQuotes"
            | "setForumTopicQuotes"
            | "markForumTopicSolution"
            | "clearForumTopicSolution"
    );
    let reply_moderation = matches!(
        name,
        "updateForumReply"
            | "deleteForumReply"
            | "updateForumReplyWithQuotes"
            | "setForumReplyQuotes"
    );

    ForumOperationPolicy {
        human_only,
        personal_projection: false,
        topic_moderation,
        reply_moderation,
    }
}

fn is_forum_response_root(name: &str) -> bool {
    matches!(
        name,
        "forumCategories"
            | "forumCategory"
            | "forumTopics"
            | "forumTopic"
            | "forumReplies"
            | "forumReply"
    )
}

fn selection_policy(
    selection_set: &SelectionSet,
    document: &ExecutableDocument,
) -> ForumOperationPolicy {
    let mut policy = ForumOperationPolicy::default();
    let _completed = super::visit_selection_fields(
        document,
        selection_set,
        false,
        |field, inside_forum_response| {
            let name = field.name.node.as_str();
            policy.merge(field_policy(name));
            let next_inside_forum = inside_forum_response || is_forum_response_root(name);
            if next_inside_forum && matches!(name, "currentUserVote" | "isSubscribed") {
                policy.personal_projection = true;
            }
            std::ops::ControlFlow::Continue(Some(next_inside_forum))
        },
    );
    policy
}

fn document_policy(
    document: &ExecutableDocument,
    operation_name: Option<&str>,
) -> ForumOperationPolicy {
    let mut policy = ForumOperationPolicy::default();
    for operation in super::selected_operations(document, operation_name) {
        policy.merge(selection_policy(
            &operation.node.selection_set.node,
            document,
        ));
    }
    policy
}

#[derive(Default)]
pub struct ForumPrincipalPolicy;

impl ExtensionFactory for ForumPrincipalPolicy {
    fn create(&self) -> Arc<dyn Extension> {
        Arc::new(ForumPrincipalPolicyExtension)
    }
}

struct ForumPrincipalPolicyExtension;

#[async_trait::async_trait]
impl Extension for ForumPrincipalPolicyExtension {
    async fn prepare_request(
        &self,
        ctx: &ExtensionContext<'_>,
        request: Request,
        next: NextPrepareRequest<'_>,
    ) -> ServerResult<Request> {
        let mut request = next.run(ctx, request).await?;
        let operation_name = request.operation_name.clone();
        let policy = document_policy(request.parsed_query()?, operation_name.as_deref());
        if !policy.is_empty() {
            request.data.insert(policy);
        }
        Ok(request)
    }

    async fn execute(
        &self,
        ctx: &ExtensionContext<'_>,
        operation_name: Option<&str>,
        next: NextExecute<'_>,
    ) -> Response {
        let Some(auth) = ctx.data_opt::<AuthContext>() else {
            return next.run(ctx, operation_name).await;
        };
        if !auth.is_service_principal() {
            return next.run(ctx, operation_name).await;
        }
        let Some(policy) = ctx.data_opt::<ForumOperationPolicy>() else {
            return next.run(ctx, operation_name).await;
        };

        let denial = if policy.human_only {
            Some(
                "Forum authorship, voting, and personal subscriptions require human-user credentials",
            )
        } else if policy.personal_projection {
            Some(
                "Forum current-user vote and subscription projections require human-user credentials",
            )
        } else if policy.topic_moderation
            && !has_effective_permission(&auth.permissions, &Permission::FORUM_TOPICS_MODERATE)
        {
            Some("Service credentials require forum_topics:moderate for this operation")
        } else if policy.reply_moderation
            && !has_effective_permission(&auth.permissions, &Permission::FORUM_REPLIES_MODERATE)
        {
            Some("Service credentials require forum_replies:moderate for this operation")
        } else {
            None
        };

        if let Some(message) = denial {
            tracing::warn!(
                operation_name_present = operation_name.is_some(),
                "Rejected GraphQL forum operation authenticated as an insufficient service principal"
            );
            return Response::from_errors(vec![
                <FieldError as GraphQLError>::permission_denied(message)
                    .into_server_error(Pos::default()),
            ]);
        }

        next.run(ctx, operation_name).await
    }
}

#[cfg(test)]
mod tests {
    use super::document_policy;

    fn policy(query: &str) -> super::ForumOperationPolicy {
        policy_for(query, None)
    }

    fn policy_for(query: &str, operation_name: Option<&str>) -> super::ForumOperationPolicy {
        let document = async_graphql::parser::parse_query(query).expect("query should parse");
        document_policy(&document, operation_name)
    }

    #[test]
    fn ignores_forum_actions_in_unselected_operations() {
        let query = r#"
            query PublicInfo { ping }
            mutation CreateTopic { createForumTopic(input: {}) { id } }
        "#;

        assert!(
            policy_for(query, Some("PublicInfo")).is_empty(),
            "unselected forum mutation must not impose service-principal restrictions"
        );
        assert!(
            policy_for(query, Some("CreateTopic")).human_only,
            "selected forum mutation must retain the human-principal restriction"
        );
    }

    #[test]
    fn classifies_human_owned_forum_actions_inside_fragments() {
        let forum_policy = policy(
            r#"
                mutation {
                    ...ForumActions
                }
                fragment ForumActions on Mutation {
                    createForumTopic(input: {}) { id }
                    setForumReplyVote(
                        tenantId: "00000000-0000-0000-0000-000000000001"
                        replyId: "00000000-0000-0000-0000-000000000002"
                        value: 1
                    ) { id }
                }
            "#,
        );
        assert!(forum_policy.human_only);
    }

    #[test]
    fn quote_aware_content_mutations_keep_human_and_moderation_boundaries() {
        assert!(policy(r#"mutation { createForumTopicWithQuotes { id } }"#).human_only);
        assert!(policy(r#"mutation { createForumReplyWithQuotes { id } }"#).human_only);

        let update_topic = policy(r#"mutation { updateForumTopicWithQuotes { id } }"#);
        assert!(update_topic.topic_moderation);
        assert!(!update_topic.human_only);

        let set_topic_quotes = policy(r#"mutation { setForumTopicQuotes { id } }"#);
        assert!(set_topic_quotes.topic_moderation);

        let update_reply = policy(r#"mutation { updateForumReplyWithQuotes { id } }"#);
        assert!(update_reply.reply_moderation);
        assert!(!update_reply.human_only);

        let set_reply_quotes = policy(r#"mutation { setForumReplyQuotes { id } }"#);
        assert!(set_reply_quotes.reply_moderation);
    }

    #[test]
    fn classifies_personal_forum_projections_only_inside_forum_results() {
        let forum_policy = policy(
            r#"
                query {
                    forumTopics {
                        nodes {
                            currentUserVote
                            isSubscribed
                        }
                    }
                }
            "#,
        );
        assert!(forum_policy.personal_projection);

        let unrelated = policy(
            r#"
                query {
                    someOtherModule {
                        isSubscribed
                    }
                }
            "#,
        );
        assert!(!unrelated.personal_projection);
    }

    #[test]
    fn classifies_topic_and_reply_moderation_separately() {
        let topic =
            policy(r#"mutation { deleteForumTopic(id: "00000000-0000-0000-0000-000000000001") }"#);
        assert!(topic.topic_moderation);
        assert!(!topic.reply_moderation);

        let reply =
            policy(r#"mutation { deleteForumReply(id: "00000000-0000-0000-0000-000000000001") }"#);
        assert!(reply.reply_moderation);
        assert!(!reply.topic_moderation);
    }

    #[test]
    fn reused_fragment_is_reclassified_for_forum_context() {
        let forum_policy = policy(
            r#"
                query {
                    someOtherModule {
                        ...SharedFields
                    }
                    forumTopics {
                        nodes {
                            ...SharedFields
                        }
                    }
                }
                fragment SharedFields on Query {
                    isSubscribed
                }
            "#,
        );

        assert!(forum_policy.personal_projection);
    }

    #[test]
    fn cyclic_fragments_are_bounded_and_classified() {
        let forum_policy = policy(
            r#"
                query {
                    forumTopics {
                        nodes {
                            ...A
                        }
                    }
                }
                fragment A on Query {
                    ...B
                    isSubscribed
                }
                fragment B on Query {
                    ...A
                }
            "#,
        );

        assert!(forum_policy.personal_projection);
    }

    #[test]
    fn category_administration_is_not_misclassified() {
        let policy = policy(r#"mutation { createForumCategory(input: {}) { id } }"#);
        assert!(!policy.human_only);
        assert!(!policy.personal_projection);
        assert!(!policy.topic_moderation);
        assert!(!policy.reply_moderation);
    }
}
