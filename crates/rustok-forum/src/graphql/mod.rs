mod category_command_mutation;
mod category_lifecycle_mutation;
mod category_policy;
mod category_route_query;
mod category_tree_query;
mod connection;
mod content_commands;
mod error_extension;
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

use async_graphql::MergedObject;

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
    GqlForumSubscriptionCursor, GqlForumSubscriptionDrift,
    GqlForumSubscriptionReconciliationReport,
};
pub use topic_fork_mutation::{
    ForkForumTopicReplyBranchGraphqlInput, GqlForumTopicFork,
};
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
    reply_audience_query::ForumReplyAudienceQuery,
    storefront_read_state::ForumStorefrontReadStateQuery,
    storefront_audience_topic::ForumStorefrontAudienceTopicQuery,
    storefront_audience_topics::ForumStorefrontAudienceTopicsQuery,
    topic_route_query::ForumTopicRouteQuery,
);

#[derive(MergedObject, Default)]
pub struct ForumMutation(
    mutation::ForumMutation,
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
