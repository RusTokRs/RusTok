mod category_command_mutation;
mod category_lifecycle_mutation;
mod category_policy;
mod category_tree_query;
mod connection;
mod content_commands;
mod error_extension;
mod mutation;
#[path = "query_runtime.rs"]
mod query;
mod quote_commands;
mod read_state;
mod reply_audience_query;
mod runtime_data;
mod storefront_audience_topic;
mod storefront_audience_topics;
mod storefront_read_state;
mod topic_merge_mutation;
mod types;

use async_graphql::MergedObject;

pub use category_command_mutation::{
    GqlForumCategoryMove, GqlForumCategoryPlacement, GqlForumCategorySiblingOrder,
    MoveForumCategoryInput, ReorderForumCategorySiblingsInput,
};
pub use category_lifecycle_mutation::GqlForumCategorySubtreeLifecycle;
pub use category_policy::{GqlForumCategoryTopicPolicy, UpdateForumCategoryTopicPolicyInput};
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
pub use runtime_data::{ForumGraphqlRuntimeData, attach_schema_data};
pub use storefront_read_state::*;
pub use topic_merge_mutation::{GqlForumTopicMerge, MergeForumTopicGraphqlInput};
pub use types::*;

#[derive(MergedObject, Default)]
pub struct ForumQuery(
    query::ForumContentQuery,
    category_tree_query::ForumCategoryTreeQuery,
    category_policy::ForumCategoryTopicPolicyQuery,
    read_state::ForumReadStateQuery,
    reply_audience_query::ForumReplyAudienceQuery,
    storefront_read_state::ForumStorefrontReadStateQuery,
    storefront_audience_topic::ForumStorefrontAudienceTopicQuery,
    storefront_audience_topics::ForumStorefrontAudienceTopicsQuery,
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
    topic_merge_mutation::ForumTopicMergeMutation,
);
