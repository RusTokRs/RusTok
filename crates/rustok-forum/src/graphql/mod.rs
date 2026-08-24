mod category_command_mutation;
mod category_lifecycle_mutation;
mod category_policy;
mod category_route_query;
mod category_tree_query;
mod connection;
mod content_commands;
mod error_extension;
mod member_card_query;
mod mention_reconciliation_query;
mod mutation;
#[path = "query_runtime.rs"]
mod query;
mod quote_commands;
mod read_state;
mod reconciliation_query;
mod reply_audience_query;
mod runtime_data;
mod storefront_audience_topic;
mod storefront_audience_topics;
mod storefront_read_state;
mod subscription_reconciliation_query;
mod topic_fork_mutation;
mod topic_merge_mutation;
mod topic_reply_range_move_mutation;
mod topic_route_query;
mod topic_slug_rename_mutation;
mod topic_split_mutation;
mod types;

use async_graphql::{ErrorExtensions, MergedObject};

pub use category_command_mutation::{
    GqlForumCategoryMove, GqlForumCategoryPlacement, GqlForumCategorySiblingOrder,
    MoveForumCategoryInput, ReorderForumCategorySiblingsInput,
};
pub use category_lifecycle_mutation::GqlForumCategorySubtreeLifecycle;
pub use category_policy::{GqlForumCategoryTopicPolicy, UpdateForumCategoryTopicPolicyInput};
pub use category_route_query::{
    GqlForumStorefrontCategoryRouteDescriptor, GqlForumStorefrontCategoryRouteDisposition,
    GqlForumStorefrontCategoryRouteResolution,
};
pub use category_tree_query::{
    GqlForumCategoryBreadcrumb, GqlForumCategoryTree, GqlForumCategoryTreeNode,
};
pub use connection::*;
pub use content_commands::{
    CreateForumReplyWithQuotesInput, CreateForumTopicWithQuotesInput,
    UpdateForumReplyWithQuotesInput, UpdateForumTopicWithQuotesInput,
};
pub use error_extension::ForumGraphqlErrorExtension;
pub use member_card_query::{
    GqlForumMemberCard, GqlForumMemberStats, MAX_FORUM_MEMBER_CARD_USER_IDS,
};
pub use mention_reconciliation_query::{GqlForumMentionDrift, GqlForumMentionReconciliationReport};
pub use quote_commands::{
    GqlForumQuoteReferenceInput, GqlForumQuoteTargetKind, GqlForumRelationQuote,
    GqlForumRelationSnapshot, SetForumQuoteRelationsInput,
};
pub use read_state::*;
pub use reconciliation_query::{
    GqlForumCounterDrift, GqlForumCounterReconciliationReport, GqlForumSolutionDrift,
    GqlForumSolutionReconciliationReport,
};
pub use runtime_data::{ForumGraphqlRuntimeData, attach_schema_data};
pub use storefront_read_state::*;
pub use subscription_reconciliation_query::{
    GqlForumSubscriptionCursor, GqlForumSubscriptionDrift, GqlForumSubscriptionReconciliationReport,
};
pub use topic_fork_mutation::{ForkForumTopicReplyBranchGraphqlInput, GqlForumTopicFork};
pub use topic_merge_mutation::{
    GqlForumTopicMerge, GqlForumTopicMergeSolutionResolution, MergeForumTopicGraphqlInput,
    ResolveForumTopicMergeSolutionGraphqlInput,
};
pub use topic_reply_range_move_mutation::{
    GqlForumReplyRangeMove, MoveForumTopicReplyRangeGraphqlInput,
};
pub use topic_route_query::{
    GqlForumStorefrontTopicRouteDecision, GqlForumStorefrontTopicRouteDecisionDisposition,
    GqlForumStorefrontTopicRouteDisposition, GqlForumStorefrontTopicRouteResolution,
};
pub use topic_slug_rename_mutation::{
    GqlForumTopicRouteDescriptor, GqlForumTopicSlugRename, RenameForumTopicSlugGraphqlInput,
};
pub use topic_split_mutation::{GqlForumTopicSplit, SplitForumTopicRepliesGraphqlInput};
pub use types::*;

#[derive(MergedObject, Default)]
pub struct ForumQuery(
    query::ForumContentQuery,
    category_tree_query::ForumCategoryTreeQuery,
    category_policy::ForumCategoryTopicPolicyQuery,
    category_route_query::ForumCategoryRouteQuery,
    read_state::ForumReadStateQuery,
    reconciliation_query::ForumReconciliationQuery,
    subscription_reconciliation_query::ForumSubscriptionReconciliationQuery,
    mention_reconciliation_query::ForumMentionReconciliationQuery,
    member_card_query::ForumMemberCardQuery,
    reply_audience_query::ForumReplyAudienceQuery,
    storefront_read_state::ForumStorefrontReadStateQuery,
    storefront_audience_topic::ForumStorefrontAudienceTopicQuery,
    storefront_audience_topics::ForumStorefrontAudienceTopicsQuery,
    topic_route_query::ForumTopicRouteQuery,
);

#[derive(MergedObject, Default)]
pub struct ForumMutation(
    mutation::ForumContentMutation,
    category_command_mutation::ForumCategoryCommandMutation,
    category_lifecycle_mutation::ForumCategoryLifecycleMutation,
    category_policy::ForumCategoryTopicPolicyMutation,
    quote_commands::ForumQuoteCommandMutation,
    content_commands::ForumContentCommandMutation,
    read_state::ForumReadStateMutation,
    storefront_read_state::ForumStorefrontReadStateMutation,
    topic_fork_mutation::ForumTopicForkMutation,
    topic_merge_mutation::ForumTopicMergeMutation,
    topic_reply_range_move_mutation::ForumTopicReplyRangeMoveMutation,
    topic_slug_rename_mutation::ForumTopicSlugRenameMutation,
    topic_split_mutation::ForumTopicSplitMutation,
);

pub(crate) fn require_forum_permission<'a>(
    ctx: &'a async_graphql::Context<'_>,
    permissions: &[rustok_api::Permission],
    message: &str,
) -> async_graphql::Result<&'a rustok_api::AuthContext> {
    rustok_api::graphql::require_graphql_auth(ctx, permissions, message)
}

pub(crate) fn resolve_tenant_scope(
    tenant: &rustok_api::TenantContext,
    requested_tenant_id: Option<uuid::Uuid>,
) -> async_graphql::Result<uuid::Uuid> {
    match requested_tenant_id {
        Some(requested_tenant_id) if requested_tenant_id != tenant.id => {
            Err(async_graphql::Error::new("Permission denied: tenant scope mismatch")
                .extend_with(|_, ext| ext.set("code", "FORBIDDEN")))
        }
        Some(requested_tenant_id) => Ok(requested_tenant_id),
        None => Ok(tenant.id),
    }
}

pub(crate) async fn require_public_forum_channel_enabled(
    ctx: &async_graphql::Context<'_>,
) -> async_graphql::Result<()> {
    if ctx.data_opt::<rustok_api::AuthContext>().is_some() {
        return Ok(());
    }

    let Some(request) = ctx.data_opt::<rustok_api::RequestContext>() else {
        return Ok(());
    };
    let Some(channel_id) = request.channel_id else {
        return Ok(());
    };

    let db = ctx.data::<sea_orm::DatabaseConnection>()?;
    let enabled = rustok_channel::ChannelService::new(db.clone())
        .is_module_enabled(channel_id, "forum")
        .await
        .map_err(|error| {
            async_graphql::Error::new(format!("Channel module check failed: {error}"))
                .extend_with(|_, extension| extension.set("code", "INTERNAL_SERVER_ERROR"))
        })?;
    if enabled {
        Ok(())
    } else {
        Err(
            async_graphql::Error::new("Forum module is not enabled for this channel")
                .extend_with(|_, extension| extension.set("code", "FORBIDDEN")),
        )
    }
}
