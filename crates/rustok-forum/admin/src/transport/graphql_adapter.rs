#[cfg(target_arch = "wasm32")]
use leptos::web_sys;
use rustok_graphql::{execute as execute_graphql, GraphqlRequest};
use rustok_ui_core::normalize_ui_text;
use serde::{Deserialize, Serialize};

use crate::model::{
    CategoryDetail, CategoryDraft, CategoryListItem, ReplyListItem, TopicDetail, TopicDraft,
    TopicListItem,
};

pub type ApiError = String;

const CATEGORIES_QUERY: &str = "query ForumAdminCategories($locale: String, $pagination: PaginationInput) { forumCategories(locale: $locale, pagination: $pagination) { total items { id locale effective_locale: effectiveLocale name slug description icon color topic_count: topicCount reply_count: replyCount } } }";
const CATEGORY_QUERY: &str = "query ForumAdminCategory($id: UUID!, $locale: String) { forumCategory(id: $id, locale: $locale) { id requested_locale: requestedLocale locale effective_locale: effectiveLocale available_locales: availableLocales name slug description icon color parent_id: parentId position topic_count: topicCount reply_count: replyCount moderated } }";
const CREATE_CATEGORY_MUTATION: &str = "mutation ForumAdminCreateCategory($input: CreateForumCategoryInput!) { createForumCategory(input: $input) { id requested_locale: requestedLocale locale effective_locale: effectiveLocale available_locales: availableLocales name slug description icon color parent_id: parentId position topic_count: topicCount reply_count: replyCount moderated } }";
const UPDATE_CATEGORY_MUTATION: &str = "mutation ForumAdminUpdateCategory($id: UUID!, $input: UpdateForumCategoryInput!) { updateForumCategory(id: $id, input: $input) { id requested_locale: requestedLocale locale effective_locale: effectiveLocale available_locales: availableLocales name slug description icon color parent_id: parentId position topic_count: topicCount reply_count: replyCount moderated } }";
const MOVE_CATEGORY_MUTATION: &str = "mutation ForumAdminMoveCategory($categoryId: UUID!, $input: MoveForumCategoryInput!) { moveForumCategory(categoryId: $categoryId, input: $input) { moved { id } } }";
const REORDER_CATEGORY_SIBLINGS_MUTATION: &str = "mutation ForumAdminReorderCategorySiblings($input: ReorderForumCategorySiblingsInput!) { reorderForumCategorySiblings(input: $input) { siblings { id } } }";
const DELETE_CATEGORY_MUTATION: &str =
    "mutation ForumAdminDeleteCategory($id: UUID!) { deleteForumCategory(id: $id) }";
const TOPICS_QUERY: &str = "query ForumAdminTopics($categoryId: UUID, $locale: String, $pagination: PaginationInput) { forumTopics(categoryId: $categoryId, locale: $locale, pagination: $pagination) { total items { id locale effective_locale: effectiveLocale category_id: categoryId author_id: authorId title slug status is_pinned: isPinned is_locked: isLocked reply_count: replyCount created_at: createdAt } } }";
const TOPIC_QUERY: &str = "query ForumAdminTopic($id: UUID!, $locale: String) { forumTopic(id: $id, locale: $locale) { id requested_locale: requestedLocale locale effective_locale: effectiveLocale available_locales: availableLocales category_id: categoryId author_id: authorId title slug body body_format: bodyFormat content_json: contentJson status tags is_pinned: isPinned is_locked: isLocked reply_count: replyCount created_at: createdAt updated_at: updatedAt } }";
const CREATE_TOPIC_MUTATION: &str = "mutation ForumAdminCreateTopic($input: CreateForumTopicInput!) { createForumTopic(input: $input) { id requested_locale: requestedLocale locale effective_locale: effectiveLocale available_locales: availableLocales category_id: categoryId author_id: authorId title slug body body_format: bodyFormat content_json: contentJson status tags is_pinned: isPinned is_locked: isLocked reply_count: replyCount created_at: createdAt updated_at: updatedAt } }";
const UPDATE_TOPIC_MUTATION: &str = "mutation ForumAdminUpdateTopic($id: UUID!, $input: UpdateForumTopicInput!) { updateForumTopic(id: $id, input: $input) { id requested_locale: requestedLocale locale effective_locale: effectiveLocale available_locales: availableLocales category_id: categoryId author_id: authorId title slug body body_format: bodyFormat content_json: contentJson status tags is_pinned: isPinned is_locked: isLocked reply_count: replyCount created_at: createdAt updated_at: updatedAt } }";
const DELETE_TOPIC_MUTATION: &str =
    "mutation ForumAdminDeleteTopic($id: UUID!) { deleteForumTopic(id: $id) }";
const REPLIES_QUERY: &str = "query ForumAdminReplies($topicId: UUID!, $locale: String, $pagination: PaginationInput) { forumReplies(topicId: $topicId, locale: $locale, pagination: $pagination) { total items { id locale effective_locale: effectiveLocale topic_id: topicId author_id: authorId content_preview: content status parent_reply_id: parentReplyId created_at: createdAt } } }";

#[derive(Debug, Deserialize)]
struct CategoriesResponse {
    #[serde(rename = "forumCategories")]
    forum_categories: CategoryConnection,
}

#[derive(Debug, Deserialize)]
struct CategoryResponse {
    #[serde(rename = "forumCategory")]
    forum_category: Option<CategoryDetail>,
}

#[derive(Debug, Deserialize)]
struct CreateCategoryResponse {
    #[serde(rename = "createForumCategory")]
    create_forum_category: CategoryDetail,
}

#[derive(Debug, Deserialize)]
struct UpdateCategoryResponse {
    #[serde(rename = "updateForumCategory")]
    update_forum_category: CategoryDetail,
}

#[derive(Debug, Deserialize)]
struct DeleteCategoryResponse {
    #[serde(rename = "deleteForumCategory")]
    delete_forum_category: bool,
}

#[derive(Debug, Deserialize)]
struct TopicsResponse {
    #[serde(rename = "forumTopics")]
    forum_topics: TopicConnection,
}

#[derive(Debug, Deserialize)]
struct TopicResponse {
    #[serde(rename = "forumTopic")]
    forum_topic: Option<TopicDetail>,
}

#[derive(Debug, Deserialize)]
struct CreateTopicResponse {
    #[serde(rename = "createForumTopic")]
    create_forum_topic: TopicDetail,
}

#[derive(Debug, Deserialize)]
struct UpdateTopicResponse {
    #[serde(rename = "updateForumTopic")]
    update_forum_topic: TopicDetail,
}

#[derive(Debug, Deserialize)]
struct DeleteTopicResponse {
    #[serde(rename = "deleteForumTopic")]
    delete_forum_topic: bool,
}

#[derive(Debug, Deserialize)]
struct RepliesResponse {
    #[serde(rename = "forumReplies")]
    forum_replies: ReplyConnection,
}

#[derive(Debug, Deserialize)]
struct CategoryConnection {
    items: Vec<CategoryListItem>,
}

#[derive(Debug, Deserialize)]
struct TopicConnection {
    items: Vec<TopicListItem>,
}

#[derive(Debug, Deserialize)]
struct ReplyConnection {
    items: Vec<ReplyListItem>,
}

#[derive(Debug, Serialize)]
struct PaginationInput {
    offset: i64,
    limit: i64,
}

#[derive(Debug, Serialize)]
struct CategoriesVariables {
    locale: Option<String>,
    pagination: PaginationInput,
}

#[derive(Debug, Serialize)]
struct CategoryVariables {
    id: String,
    locale: Option<String>,
}

#[derive(Debug, Serialize)]
struct TopicsVariables {
    #[serde(rename = "categoryId")]
    category_id: Option<String>,
    locale: Option<String>,
    pagination: PaginationInput,
}

#[derive(Debug, Serialize)]
struct TopicVariables {
    id: String,
    locale: Option<String>,
}

#[derive(Debug, Serialize)]
struct RepliesVariables {
    #[serde(rename = "topicId")]
    topic_id: String,
    locale: Option<String>,
    pagination: PaginationInput,
}

#[derive(Debug, Serialize)]
struct CategoryMutationVariables<T> {
    input: T,
}

#[derive(Debug, Serialize)]
struct CategoryUpdateVariables<T> {
    id: String,
    input: T,
}

#[derive(Debug, Serialize)]
struct CategoryMoveVariables {
    #[serde(rename = "categoryId")]
    category_id: String,
    input: MoveCategoryInput,
}

#[derive(Debug, Serialize)]
struct CategoryReorderVariables {
    input: ReorderCategorySiblingsInput,
}

#[derive(Debug, Serialize)]
struct TopicMutationVariables<T> {
    input: T,
}

#[derive(Debug, Serialize)]
struct TopicUpdateVariables<T> {
    id: String,
    input: T,
}

#[derive(Debug, Serialize)]
struct IdVariables {
    id: String,
}

#[derive(Debug, Serialize)]
struct CreateCategoryInput {
    locale: String,
    name: String,
    slug: String,
    description: Option<String>,
    icon: Option<String>,
    color: Option<String>,
    #[serde(rename = "parentId")]
    parent_id: Option<String>,
    position: Option<i32>,
    moderated: bool,
}

#[derive(Debug, Serialize)]
struct UpdateCategoryInput {
    locale: String,
    name: Option<String>,
    slug: Option<String>,
    description: Option<String>,
    icon: Option<String>,
    color: Option<String>,
    moderated: Option<bool>,
}

#[derive(Debug, Serialize)]
struct MoveCategoryInput {
    #[serde(rename = "parentId")]
    parent_id: Option<String>,
    position: i32,
}

#[derive(Debug, Serialize)]
struct ReorderCategorySiblingsInput {
    #[serde(rename = "parentId")]
    parent_id: Option<String>,
    #[serde(rename = "orderedCategoryIds")]
    ordered_category_ids: Vec<String>,
}

#[derive(Debug, Serialize)]
struct CreateTopicInput {
    locale: String,
    #[serde(rename = "categoryId")]
    category_id: String,
    title: String,
    slug: Option<String>,
    body: String,
    #[serde(rename = "bodyFormat")]
    body_format: Option<String>,
    #[serde(rename = "contentJson")]
    content_json: Option<serde_json::Value>,
    metadata: Option<serde_json::Value>,
    tags: Vec<String>,
    #[serde(rename = "channelSlugs")]
    channel_slugs: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
struct UpdateTopicInput {
    locale: String,
    title: Option<String>,
    body: Option<String>,
    #[serde(rename = "bodyFormat")]
    body_format: Option<String>,
    #[serde(rename = "contentJson")]
    content_json: Option<serde_json::Value>,
    metadata: Option<serde_json::Value>,
    tags: Option<Vec<String>>,
    #[serde(rename = "channelSlugs")]
    channel_slugs: Option<Vec<String>>,
}

fn graphql_url() -> String {
    if let Some(url) = option_env!("RUSTOK_GRAPHQL_URL") {
        return url.to_string();
    }

    #[cfg(target_arch = "wasm32")]
    {
        let origin = web_sys::window()
            .and_then(|window| window.location().origin().ok())
            .unwrap_or_else(|| "http://localhost:5150".to_string());
        format!("{origin}/api/graphql")
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let base =
            std::env::var("RUSTOK_API_URL").unwrap_or_else(|_| "http://localhost:5150".to_string());
        format!("{base}/api/graphql")
    }
}

async fn request<V, T>(
    query: &str,
    variables: V,
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<T, ApiError>
where
    V: Serialize,
    T: for<'de> Deserialize<'de>,
{
    execute_graphql(
        &graphql_url(),
        GraphqlRequest::new(query, Some(variables)),
        token,
        tenant_slug,
        None,
    )
    .await
    .map_err(|err| err.to_string())
}

pub async fn fetch_categories(
    token: Option<String>,
    tenant_slug: Option<String>,
    locale: String,
) -> Result<Vec<CategoryListItem>, ApiError> {
    let response: CategoriesResponse = request(
        CATEGORIES_QUERY,
        CategoriesVariables {
            locale: Some(locale),
            pagination: PaginationInput {
                offset: 0,
                limit: 50,
            },
        },
        token,
        tenant_slug,
    )
    .await?;
    Ok(response.forum_categories.items)
}

pub async fn fetch_category(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
    locale: String,
) -> Result<CategoryDetail, ApiError> {
    let response: CategoryResponse = request(
        CATEGORY_QUERY,
        CategoryVariables {
            id: id.clone(),
            locale: Some(locale),
        },
        token,
        tenant_slug,
    )
    .await?;
    response
        .forum_category
        .ok_or_else(|| format!("Forum category not found: {id}"))
}

pub async fn create_category(
    token: Option<String>,
    tenant_slug: Option<String>,
    draft: CategoryDraft,
) -> Result<CategoryDetail, ApiError> {
    let locale = draft.locale.clone();
    let requested_position = placement_position(draft.position)?;
    let response: CreateCategoryResponse = request(
        CREATE_CATEGORY_MUTATION,
        CategoryMutationVariables {
            input: create_category_input(draft),
        },
        token.clone(),
        tenant_slug.clone(),
    )
    .await?;
    let category = response.create_forum_category;
    move_category(
        token.clone(),
        tenant_slug.clone(),
        category.id.clone(),
        category.parent_id.clone(),
        requested_position,
    )
    .await?;
    fetch_category(token, tenant_slug, category.id, locale).await
}

pub async fn update_category(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
    draft: CategoryDraft,
) -> Result<CategoryDetail, ApiError> {
    let locale = draft.locale.clone();
    let requested_position = placement_position(draft.position)?;
    let response: UpdateCategoryResponse = request(
        UPDATE_CATEGORY_MUTATION,
        CategoryUpdateVariables {
            id: id.clone(),
            input: update_category_input(draft.clone()),
        },
        token.clone(),
        tenant_slug.clone(),
    )
    .await?;
    let category = response.update_forum_category;
    if category.position != draft.position {
        move_category(
            token.clone(),
            tenant_slug.clone(),
            id.clone(),
            category.parent_id,
            requested_position,
        )
        .await?;
        return fetch_category(token, tenant_slug, id, locale).await;
    }
    Ok(category)
}

pub async fn move_category(
    token: Option<String>,
    tenant_slug: Option<String>,
    category_id: String,
    parent_id: Option<String>,
    position: u32,
) -> Result<(), ApiError> {
    let position = i32::try_from(position)
        .map_err(|_| "Category position exceeds GraphQL integer range".to_string())?;
    let _: serde_json::Value = request(
        MOVE_CATEGORY_MUTATION,
        CategoryMoveVariables {
            category_id,
            input: MoveCategoryInput {
                parent_id,
                position,
            },
        },
        token,
        tenant_slug,
    )
    .await?;
    Ok(())
}

pub async fn reorder_category_siblings(
    token: Option<String>,
    tenant_slug: Option<String>,
    parent_id: Option<String>,
    ordered_category_ids: Vec<String>,
) -> Result<(), ApiError> {
    let _: serde_json::Value = request(
        REORDER_CATEGORY_SIBLINGS_MUTATION,
        CategoryReorderVariables {
            input: ReorderCategorySiblingsInput {
                parent_id,
                ordered_category_ids,
            },
        },
        token,
        tenant_slug,
    )
    .await?;
    Ok(())
}

pub async fn delete_category(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
) -> Result<(), ApiError> {
    let response: DeleteCategoryResponse = request(
        DELETE_CATEGORY_MUTATION,
        IdVariables { id },
        token,
        tenant_slug,
    )
    .await?;
    if response.delete_forum_category {
        Ok(())
    } else {
        Err("Forum category delete returned false".to_string())
    }
}

pub async fn fetch_topics(
    token: Option<String>,
    tenant_slug: Option<String>,
    locale: String,
    category_id: Option<String>,
) -> Result<Vec<TopicListItem>, ApiError> {
    let response: TopicsResponse = request(
        TOPICS_QUERY,
        TopicsVariables {
            category_id: category_id.filter(|value| !value.trim().is_empty()),
            locale: Some(locale),
            pagination: PaginationInput {
                offset: 0,
                limit: 50,
            },
        },
        token,
        tenant_slug,
    )
    .await?;
    Ok(response.forum_topics.items)
}

pub async fn fetch_topic(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
    locale: String,
) -> Result<TopicDetail, ApiError> {
    let response: TopicResponse = request(
        TOPIC_QUERY,
        TopicVariables {
            id: id.clone(),
            locale: Some(locale),
        },
        token,
        tenant_slug,
    )
    .await?;
    response
        .forum_topic
        .ok_or_else(|| format!("Forum topic not found: {id}"))
}

pub async fn create_topic(
    token: Option<String>,
    tenant_slug: Option<String>,
    draft: TopicDraft,
) -> Result<TopicDetail, ApiError> {
    let response: CreateTopicResponse = request(
        CREATE_TOPIC_MUTATION,
        TopicMutationVariables {
            input: create_topic_input(draft),
        },
        token,
        tenant_slug,
    )
    .await?;
    Ok(response.create_forum_topic)
}

pub async fn update_topic(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
    draft: TopicDraft,
) -> Result<TopicDetail, ApiError> {
    let response: UpdateTopicResponse = request(
        UPDATE_TOPIC_MUTATION,
        TopicUpdateVariables {
            id,
            input: update_topic_input(draft),
        },
        token,
        tenant_slug,
    )
    .await?;
    Ok(response.update_forum_topic)
}

pub async fn delete_topic(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
) -> Result<(), ApiError> {
    let response: DeleteTopicResponse = request(
        DELETE_TOPIC_MUTATION,
        IdVariables { id },
        token,
        tenant_slug,
    )
    .await?;
    if response.delete_forum_topic {
        Ok(())
    } else {
        Err("Forum topic delete returned false".to_string())
    }
}

pub async fn fetch_replies(
    token: Option<String>,
    tenant_slug: Option<String>,
    topic_id: String,
    locale: String,
) -> Result<Vec<ReplyListItem>, ApiError> {
    let response: RepliesResponse = request(
        REPLIES_QUERY,
        RepliesVariables {
            topic_id,
            locale: Some(locale),
            pagination: PaginationInput {
                offset: 0,
                limit: 20,
            },
        },
        token,
        tenant_slug,
    )
    .await?;
    Ok(response.forum_replies.items)
}

fn create_category_input(draft: CategoryDraft) -> CreateCategoryInput {
    CreateCategoryInput {
        locale: draft.locale,
        name: draft.name,
        slug: draft.slug,
        description: optional_text(draft.description),
        icon: optional_text(draft.icon),
        color: optional_text(draft.color),
        parent_id: None,
        position: None,
        moderated: draft.moderated,
    }
}

fn update_category_input(draft: CategoryDraft) -> UpdateCategoryInput {
    UpdateCategoryInput {
        locale: draft.locale,
        name: Some(draft.name),
        slug: Some(draft.slug),
        description: optional_text(draft.description),
        icon: optional_text(draft.icon),
        color: optional_text(draft.color),
        moderated: Some(draft.moderated),
    }
}

fn create_topic_input(draft: TopicDraft) -> CreateTopicInput {
    CreateTopicInput {
        locale: draft.locale,
        category_id: draft.category_id,
        title: draft.title,
        slug: optional_text(draft.slug),
        body: draft.body,
        body_format: Some(draft.body_format),
        content_json: None,
        metadata: None,
        tags: draft.tags,
        channel_slugs: None,
    }
}

fn update_topic_input(draft: TopicDraft) -> UpdateTopicInput {
    UpdateTopicInput {
        locale: draft.locale,
        title: Some(draft.title),
        body: Some(draft.body),
        body_format: Some(draft.body_format),
        content_json: None,
        metadata: None,
        tags: Some(draft.tags),
        channel_slugs: None,
    }
}

fn placement_position(position: i32) -> Result<u32, ApiError> {
    u32::try_from(position).map_err(|_| "Category position must be zero or greater".to_string())
}

fn optional_text(value: String) -> Option<String> {
    normalize_ui_text(value.as_str())
}
