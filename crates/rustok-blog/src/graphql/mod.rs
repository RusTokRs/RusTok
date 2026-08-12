mod mutation;
mod query;
mod rate_limit;
mod runtime_data;
mod types;

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
}
