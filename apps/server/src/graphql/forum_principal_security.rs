use std::sync::Arc;

use async_graphql::extensions::{
    Extension, ExtensionContext, ExtensionFactory, NextExecute, NextPrepareRequest,
};
use async_graphql::parser::types::{ExecutableDocument, Selection, SelectionSet};
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
    let human_only = matches!(name, "createForumTopic" | "createForumReply")
        || (name.contains("Forum") && (name.contains("Vote") || name.contains("Subscription")));
    let topic_moderation = matches!(
        name,
        "updateForumTopic"
            | "deleteForumTopic"
            | "markForumTopicSolution"
            | "clearForumTopicSolution"
    );
    let reply_moderation = matches!(name, "updateForumReply" | "deleteForumReply");

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
    inside_forum_response: bool,
) -> ForumOperationPolicy {
    let mut policy = ForumOperationPolicy::default();
    for selection in &selection_set.items {
        match &selection.node {
            Selection::Field(field) => {
                let name = field.node.name.node.as_str();
                policy.merge(field_policy(name));
                let next_inside_forum = inside_forum_response || is_forum_response_root(name);
                if next_inside_forum && matches!(name, "currentUserVote" | "isSubscribed") {
                    policy.personal_projection = true;
                }
                policy.merge(selection_policy(
                    &field.node.selection_set.node,
                    document,
                    next_inside_forum,
                ));
            }
            Selection::FragmentSpread(fragment) => {
                if let Some(definition) = document.fragments.get(&fragment.node.fragment_name.node)
                {
                    policy.merge(selection_policy(
                        &definition.node.selection_set.node,
                        document,
                        inside_forum_response,
                    ));
                }
            }
            Selection::InlineFragment(fragment) => policy.merge(selection_policy(
                &fragment.node.selection_set.node,
                document,
                inside_forum_response,
            )),
        }
    }
    policy
}

fn document_policy(document: &ExecutableDocument) -> ForumOperationPolicy {
    let mut policy = ForumOperationPolicy::default();
    for (_, operation) in document.operations.iter() {
        policy.merge(selection_policy(
            &operation.node.selection_set.node,
            document,
            false,
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
        if !request.query.trim().is_empty() {
            let policy = document_policy(request.parsed_query()?);
            if !policy.is_empty() {
                request.data.insert(policy);
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
                operation_name = ?operation_name,
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
        let document = async_graphql::parser::parse_query(query).expect("query should parse");
        document_policy(&document)
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
    fn category_administration_is_not_misclassified() {
        let policy = policy(r#"mutation { createForumCategory(input: {}) { id } }"#);
        assert!(!policy.human_only);
        assert!(!policy.personal_projection);
        assert!(!policy.topic_moderation);
        assert!(!policy.reply_moderation);
    }
}
