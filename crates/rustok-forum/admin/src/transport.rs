mod category_tree_graphql_adapter;
mod category_tree_rest_adapter;
mod graphql_adapter;
mod rest_adapter;

use crate::model::{
    CategoryDetail, CategoryDraft, CategoryListItem, ReplyListItem, TopicDetail, TopicDraft,
    TopicListItem,
};

pub type ApiError = String;

pub async fn fetch_category_tree(
    token: Option<String>,
    tenant_slug: Option<String>,
    locale: String,
) -> Result<Vec<CategoryListItem>, ApiError> {
    match category_tree_graphql_adapter::fetch_category_tree(
        token.clone(),
        tenant_slug.clone(),
        locale.clone(),
    )
    .await
    {
        Ok(tree) => Ok(tree.into_flat_items()),
        Err(_) => category_tree_rest_adapter::fetch_category_tree(token, tenant_slug, locale)
            .await
            .map(|tree| tree.into_flat_items()),
    }
}

pub async fn fetch_categories(
    token: Option<String>,
    tenant_slug: Option<String>,
    locale: String,
) -> Result<Vec<CategoryListItem>, ApiError> {
    match graphql_adapter::fetch_categories(token.clone(), tenant_slug.clone(), locale.clone())
        .await
    {
        Ok(categories) => Ok(categories),
        Err(_) => rest_adapter::fetch_categories(token, tenant_slug, locale).await,
    }
}

pub async fn fetch_category(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
    locale: String,
) -> Result<CategoryDetail, ApiError> {
    match graphql_adapter::fetch_category(
        token.clone(),
        tenant_slug.clone(),
        id.clone(),
        locale.clone(),
    )
    .await
    {
        Ok(category) => Ok(category),
        Err(_) => rest_adapter::fetch_category(token, tenant_slug, id, locale).await,
    }
}

pub async fn create_category(
    token: Option<String>,
    tenant_slug: Option<String>,
    draft: CategoryDraft,
) -> Result<CategoryDetail, ApiError> {
    let locale = draft.locale.clone();
    let requested_position = placement_position(draft.position)?;

    match graphql_adapter::create_category(token.clone(), tenant_slug.clone(), draft.clone()).await {
        Ok(category) => {
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
        Err(_) => rest_adapter::create_category(token, tenant_slug, draft).await,
    }
}

pub async fn update_category(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
    draft: CategoryDraft,
) -> Result<CategoryDetail, ApiError> {
    let locale = draft.locale.clone();
    let requested_position = placement_position(draft.position)?;

    match graphql_adapter::update_category(
        token.clone(),
        tenant_slug.clone(),
        id.clone(),
        draft.clone(),
    )
    .await
    {
        Ok(category) if category.position != draft.position => {
            move_category(
                token.clone(),
                tenant_slug.clone(),
                id.clone(),
                category.parent_id,
                requested_position,
            )
            .await?;
            fetch_category(token, tenant_slug, id, locale).await
        }
        Ok(category) => Ok(category),
        Err(_) => rest_adapter::update_category(token, tenant_slug, id, draft).await,
    }
}

pub async fn move_category(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
    parent_id: Option<String>,
    position: u32,
) -> Result<(), ApiError> {
    match graphql_adapter::move_category(
        token.clone(),
        tenant_slug.clone(),
        id.clone(),
        parent_id.clone(),
        position,
    )
    .await
    {
        Ok(()) => Ok(()),
        Err(_) => rest_adapter::move_category(token, tenant_slug, id, parent_id, position).await,
    }
}

pub async fn reorder_category_siblings(
    token: Option<String>,
    tenant_slug: Option<String>,
    parent_id: Option<String>,
    ordered_category_ids: Vec<String>,
) -> Result<(), ApiError> {
    match graphql_adapter::reorder_category_siblings(
        token.clone(),
        tenant_slug.clone(),
        parent_id.clone(),
        ordered_category_ids.clone(),
    )
    .await
    {
        Ok(()) => Ok(()),
        Err(_) => {
            rest_adapter::reorder_category_siblings(
                token,
                tenant_slug,
                parent_id,
                ordered_category_ids,
            )
            .await
        }
    }
}

pub async fn delete_category(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
) -> Result<(), ApiError> {
    match graphql_adapter::delete_category(token.clone(), tenant_slug.clone(), id.clone()).await {
        Ok(()) => Ok(()),
        Err(_) => rest_adapter::delete_category(token, tenant_slug, id).await,
    }
}

pub async fn fetch_topics(
    token: Option<String>,
    tenant_slug: Option<String>,
    locale: String,
    category_id: Option<String>,
) -> Result<Vec<TopicListItem>, ApiError> {
    match graphql_adapter::fetch_topics(
        token.clone(),
        tenant_slug.clone(),
        locale.clone(),
        category_id.clone(),
    )
    .await
    {
        Ok(topics) => Ok(topics),
        Err(_) => rest_adapter::fetch_topics(token, tenant_slug, locale, category_id).await,
    }
}

pub async fn fetch_topic(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
    locale: String,
) -> Result<TopicDetail, ApiError> {
    match graphql_adapter::fetch_topic(
        token.clone(),
        tenant_slug.clone(),
        id.clone(),
        locale.clone(),
    )
    .await
    {
        Ok(topic) => Ok(topic),
        Err(_) => rest_adapter::fetch_topic(token, tenant_slug, id, locale).await,
    }
}

pub async fn create_topic(
    token: Option<String>,
    tenant_slug: Option<String>,
    draft: TopicDraft,
) -> Result<TopicDetail, ApiError> {
    match graphql_adapter::create_topic(token.clone(), tenant_slug.clone(), draft.clone()).await {
        Ok(topic) => Ok(topic),
        Err(_) => rest_adapter::create_topic(token, tenant_slug, draft).await,
    }
}

pub async fn update_topic(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
    draft: TopicDraft,
) -> Result<TopicDetail, ApiError> {
    match graphql_adapter::update_topic(
        token.clone(),
        tenant_slug.clone(),
        id.clone(),
        draft.clone(),
    )
    .await
    {
        Ok(topic) => Ok(topic),
        Err(_) => rest_adapter::update_topic(token, tenant_slug, id, draft).await,
    }
}

pub async fn delete_topic(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
) -> Result<(), ApiError> {
    match graphql_adapter::delete_topic(token.clone(), tenant_slug.clone(), id.clone()).await {
        Ok(()) => Ok(()),
        Err(_) => rest_adapter::delete_topic(token, tenant_slug, id).await,
    }
}

pub async fn fetch_replies(
    token: Option<String>,
    tenant_slug: Option<String>,
    topic_id: String,
    locale: String,
) -> Result<Vec<ReplyListItem>, ApiError> {
    match graphql_adapter::fetch_replies(
        token.clone(),
        tenant_slug.clone(),
        topic_id.clone(),
        locale.clone(),
    )
    .await
    {
        Ok(replies) => Ok(replies),
        Err(_) => rest_adapter::fetch_replies(token, tenant_slug, topic_id, locale).await,
    }
}

fn placement_position(position: i32) -> Result<u32, ApiError> {
    u32::try_from(position).map_err(|_| "Category position must be zero or greater".to_string())
}
