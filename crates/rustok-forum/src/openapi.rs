use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    paths(
        crate::controllers::categories::list_categories,
        crate::controllers::categories::get_category,
        crate::controllers::categories::create_category,
        crate::controllers::categories::update_category,
        crate::controllers::categories::delete_category,
        crate::controllers::topics::list_topics,
        crate::controllers::topics::get_topic,
        crate::controllers::topics::create_topic,
        crate::controllers::topics::update_topic,
        crate::controllers::topics::delete_topic,
        crate::controllers::replies::list_replies,
        crate::controllers::replies::get_reply,
        crate::controllers::replies::create_reply,
        crate::controllers::replies::update_reply,
        crate::controllers::replies::delete_reply,
    ),
    components(
        schemas(
            crate::CreateCategoryInput,
            crate::UpdateCategoryInput,
            crate::CategoryResponse,
            crate::CategoryListItem,
            crate::CreateTopicInput,
            crate::UpdateTopicInput,
            crate::ListTopicsFilter,
            crate::TopicResponse,
            crate::TopicListItem,
            crate::CreateReplyInput,
            crate::UpdateReplyInput,
            crate::ListRepliesFilter,
            crate::ReplyResponse,
            crate::ReplyListItem,
        )
    ),
    tags((name = "forum", description = "Forum endpoints"))
)]
pub struct ForumApiDoc;

pub fn openapi_document() -> utoipa::openapi::OpenApi {
    ForumApiDoc::openapi()
}
