mod category_command_mutation;
mod category_lifecycle_mutation;
mod category_policy;
mod category_tree_query;
mod connection;
mod mutation;
mod query;
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
pub use types::*;

#[derive(MergedObject, Default)]
pub struct ForumQuery(
    query::ForumQuery,
    category_tree_query::ForumCategoryTreeQuery,
    category_policy::ForumCategoryTopicPolicyQuery,
);

#[derive(MergedObject, Default)]
pub struct ForumMutation(
    mutation::ForumMutation,
    category_command_mutation::ForumCategoryCommandMutation,
    category_lifecycle_mutation::ForumCategoryLifecycleMutation,
    category_policy::ForumCategoryTopicPolicyMutation,
);
