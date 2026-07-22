use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    paths(
        crate::controllers::posts::list_posts,
        crate::controllers::posts::get_post,
        crate::controllers::posts::create_post,
        crate::controllers::posts::update_post,
        crate::controllers::posts::delete_post,
        crate::controllers::posts::publish_post,
        crate::controllers::posts::unpublish_post,
        crate::controllers::categories::list_categories,
        crate::controllers::categories::get_category,
        crate::controllers::categories::create_category,
        crate::controllers::categories::update_category,
        crate::controllers::categories::delete_category,
        crate::controllers::comments::moderate_comment,
    ),
    components(
        schemas(
            crate::dto::CreatePostInput,
            crate::dto::UpdatePostInput,
            crate::dto::PostResponse,
            crate::dto::PostSummary,
            crate::dto::PostListQuery,
            crate::dto::PostListResponse,
            crate::dto::CreateCategoryInput,
            crate::dto::UpdateCategoryInput,
            crate::dto::CategoryResponse,
            crate::dto::CategoryListItem,
            crate::dto::CategoryListResponse,
            crate::dto::ListCategoriesFilter,
            crate::dto::CommentResponse,
            crate::dto::ModerateCommentInput,
            crate::dto::ModerateCommentStatus,
            crate::state_machine::BlogPostStatus,
        )
    ),
    tags((name = "blog", description = "Blog endpoints"))
)]
pub struct BlogApiDoc;

pub fn openapi_document() -> utoipa::openapi::OpenApi {
    BlogApiDoc::openapi()
}
