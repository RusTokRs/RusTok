mod category_types;
mod mutation;
mod query;
mod rate_limit;
mod runtime_data;
mod types;

pub use category_types::*;
pub use mutation::BlogMutation;
pub use query::BlogQuery;
pub use rate_limit::{
    BlogGraphqlRateLimitError, BlogGraphqlRateLimitExceeded, BlogGraphqlRateLimitPolicy,
    BlogGraphqlRateLimiter, BlogGraphqlRateLimiterHandle,
};
pub use runtime_data::{BlogGraphqlRuntimeData, attach_schema_data};
pub use types::*;

#[cfg(test)]
mod schema_tests {
    use async_graphql::{EmptySubscription, Schema};

    use super::{BlogMutation, BlogQuery};

    #[test]
    fn public_comment_command_uses_the_canonical_unversioned_schema_names() {
        let schema = Schema::build(BlogQuery, BlogMutation, EmptySubscription).finish();
        let sdl = schema.sdl();

        assert!(sdl.contains("createBlogComment("));
        assert!(sdl.contains("input CreateBlogCommentInput"));
        assert!(sdl.contains("content: RichText!"));
        assert!(!sdl.contains("createCommentV"));
        assert!(!sdl.contains("CreateBlogCommentInputV"));
    }

    #[test]
    fn category_schema_keeps_localized_and_structural_commands_separate() {
        let schema = Schema::build(BlogQuery, BlogMutation, EmptySubscription).finish();
        let sdl = schema.sdl();

        assert!(sdl.contains("blogCategoryTree("));
        assert!(sdl.contains("blogCategory("));
        assert!(sdl.contains("createBlogCategory("));
        assert!(sdl.contains("updateBlogCategory("));
        assert!(sdl.contains("moveBlogCategory("));
        assert!(sdl.contains("deleteBlogCategory("));
        assert!(sdl.contains("input UpdateBlogCategoryInput"));
        assert!(sdl.contains("input MoveBlogCategoryInput"));
        let update_input = sdl
            .split("input UpdateBlogCategoryInput")
            .nth(1)
            .and_then(|value| value.split('}').next())
            .expect("update input block");
        assert!(!update_input.contains("position:"));
        assert!(!update_input.contains("parentId:"));
    }
}
