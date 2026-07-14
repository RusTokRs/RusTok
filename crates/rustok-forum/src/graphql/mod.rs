mod category_tree_query;
mod connection;
mod mutation;
mod query;
mod types;

use async_graphql::MergedObject;

pub use category_tree_query::{
    GqlForumCategoryBreadcrumb, GqlForumCategoryTree, GqlForumCategoryTreeNode,
};
pub use connection::*;
pub use mutation::ForumMutation;
pub use types::*;

// Diagnostic branch marker: keeps the focused forum workflow enabled.
#[derive(MergedObject, Default)]
pub struct ForumQuery(query::ForumQuery, category_tree_query::ForumCategoryTreeQuery);
