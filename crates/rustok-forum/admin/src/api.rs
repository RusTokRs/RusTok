#[cfg(target_arch = "wasm32")]
use leptos::web_sys;
use reqwest::Method;
use serde::{de::DeserializeOwned, Serialize};

use crate::model::{
    CategoryDetail, CategoryDraft, CategoryListItem, ReplyListItem, TopicDetail, TopicDraft,
    TopicListItem,
};

const AUTH_HEADER: &str = "Authorization";
const TENANT_HEADER: &str = "X-Tenant-Slug";
const ACCEPT_LANGUAGE_HEADER: &str = "Accept-Language";

pub type ApiError = String;

#[derive(Debug, Serialize)]
struct CreateCategoryInput<'a> {
    locale: &'a str,
    name: &'a str,
    slug: &'a str,
    description: Option<&'a str>,
    icon: Option<&'a str>,
    color: Option<&'a str>,
    parent_id: Option<String>,
    position: Option<i32>,
    moderated: bool,
}

#[derive(Debug, Serialize)]
struct UpdateCategoryInput<'a> {
    locale: &'a str,
    name: Option<&'a str>,
    slug: Option<&'a str>,
    description: Option<&'a str>,
    icon: Option<&'a str>,
    color: Option<&'a str>,
    position: Option<i32>,
    moderated: Option<bool>,
}

#[derive(Debug, Serialize)]
struct CreateTopicInput<'a> {
    locale: &'a str,
    category_id: &'a str,
    title: &'a str,
    slug: Option<&'a str>,
    body: &'a str,
    body_format: &'a str,
    content_json: Option<serde_json::Value>,
    metadata: Option<serde_json::Value>,
    tags: &'a [String],
}

#[derive(Debug, Serialize)]
struct UpdateTopicInput<'a> {
    locale: &'a str,
    title: Option<&'a str>,
    body: Option<&'a str>,
    body_format: Option<&'a str>,
    content_json: Option<serde_json::Value>,
    metadata: Option<serde_json::Value>,
    tags: Option<&'a [String]>,
}

fn api_base_url() -> String {
    if let Some(url) = option_env!("RUSTOK_API_URL") {
        return format!("{url}/api/forum");
    }

    #[cfg(target_arch = "wasm32")]
    {
        let origin = web_sys::window()
            .and_then(|window| window.location().origin().ok())
            .unwrap_or_else(|| "http://localhost:5150".to_string());
        format!("{origin}/api/forum")
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let base =
            std::env::var("RUSTOK_API_URL").unwrap_or_else(|_| "http://localhost:5150".to_string());
        format!("{base}/api/forum")
    }
}

async fn request_json<T, B>(
    method: Method,
    path: &str,
    token: Option<String>,
    tenant_slug: Option<String>,
    locale: Option<String>,
    body: Option<B>,
) -> Result<T, ApiError>
where
    T: DeserializeOwned,
    B: Serialize,
{
    let client = reqwest::Client::new();
    let mut request = client.request(method, format!("{}{}", api_base_url(), path));

    if let Some(value) = token {
        request = request.header(AUTH_HEADER, format!("Bearer {value}"));
    }
    if let Some(value) = tenant_slug {
        request = request.header(TENANT_HEADER, value);
    }
    if let Some(value) = locale {
        request = request.header(ACCEPT_LANGUAGE_HEADER, value);
    }
    if let Some(payload) = body {
        request = request.json(&payload);
    }

    let response = request
        .send()
        .await
        .map_err(|err| format!("Network error: {err}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        let suffix = if body.trim().is_empty() {
            String::new()
        } else {
            format!(": {}", body.trim())
        };
        return Err(format!("HTTP {}{}", status, suffix));
    }

    response
        .json::<T>()
        .await
        .map_err(|err| format!("Invalid JSON response: {err}"))
}

async fn request_empty<B>(
    method: Method,
    path: &str,
    token: Option<String>,
    tenant_slug: Option<String>,
    locale: Option<String>,
    body: Option<B>,
) -> Result<(), ApiError>
where
    B: Serialize,
{
    let client = reqwest::Client::new();
    let mut request = client.request(method, format!("{}{}", api_base_url(), path));

    if let Some(value) = token {
        request = request.header(AUTH_HEADER, format!("Bearer {value}"));
    }
    if let Some(value) = tenant_slug {
        request = request.header(TENANT_HEADER, value);
    }
    if let Some(value) = locale {
        request = request.header(ACCEPT_LANGUAGE_HEADER, value);
    }
    if let Some(payload) = body {
        request = request.json(&payload);
    }

    let response = request
        .send()
        .await
        .map_err(|err| format!("Network error: {err}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        let suffix = if body.trim().is_empty() {
            String::new()
        } else {
            format!(": {}", body.trim())
        };
        return Err(format!("HTTP {}{}", status, suffix));
    }

    Ok(())
}

pub async fn fetch_categories(
    token: Option<String>,
    tenant_slug: Option<String>,
    locale: String,
) -> Result<Vec<CategoryListItem>, ApiError> {
    request_json(
        Method::GET,
        format!("/categories?locale={locale}&page=1&per_page=50").as_str(),
        token,
        tenant_slug,
        Some(locale),
        None::<()>,
    )
    .await
}

pub async fn fetch_category(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
    locale: String,
) -> Result<CategoryDetail, ApiError> {
    request_json(
        Method::GET,
        format!("/categories/{id}?locale={locale}").as_str(),
        token,
        tenant_slug,
        Some(locale),
        None::<()>,
    )
    .await
}

pub async fn create_category(
    token: Option<String>,
    tenant_slug: Option<String>,
    draft: CategoryDraft,
) -> Result<CategoryDetail, ApiError> {
    request_json(
        Method::POST,
        "/categories",
        token,
        tenant_slug,
        Some(draft.locale.clone()),
        Some(CreateCategoryInput {
            locale: draft.locale.as_str(),
            name: draft.name.as_str(),
            slug: draft.slug.as_str(),
            description: optional_text(draft.description.as_str()),
            icon: optional_text(draft.icon.as_str()),
            color: optional_text(draft.color.as_str()),
            parent_id: None,
            position: Some(draft.position),
            moderated: draft.moderated,
        }),
    )
    .await
}

pub async fn update_category(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
    draft: CategoryDraft,
) -> Result<CategoryDetail, ApiError> {
    request_json(
        Method::PUT,
        format!("/categories/{id}").as_str(),
        token,
        tenant_slug,
        Some(draft.locale.clone()),
        Some(UpdateCategoryInput {
            locale: draft.locale.as_str(),
            name: Some(draft.name.as_str()),
            slug: Some(draft.slug.as_str()),
            description: optional_text(draft.description.as_str()),
            icon: optional_text(draft.icon.as_str()),
            color: optional_text(draft.color.as_str()),
            position: Some(draft.position),
            moderated: Some(draft.moderated),
        }),
    )
    .await
}

pub async fn delete_category(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
) -> Result<(), ApiError> {
    request_empty(
        Method::DELETE,
        format!("/categories/{id}").as_str(),
        token,
        tenant_slug,
        None,
        None::<()>,
    )
    .await
}

pub async fn fetch_topics(
    token: Option<String>,
    tenant_slug: Option<String>,
    locale: String,
    category_id: Option<String>,
) -> Result<Vec<TopicListItem>, ApiError> {
    let mut path = format!("/topics?locale={locale}&page=1&per_page=50");
    if let Some(value) = category_id.filter(|value| !value.trim().is_empty()) {
        path.push_str("&category_id=");
        path.push_str(value.as_str());
    }
    request_json(
        Method::GET,
        path.as_str(),
        token,
        tenant_slug,
        Some(locale),
        None::<()>,
    )
    .await
}

pub async fn fetch_topic(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
    locale: String,
) -> Result<TopicDetail, ApiError> {
    request_json(
        Method::GET,
        format!("/topics/{id}?locale={locale}").as_str(),
        token,
        tenant_slug,
        Some(locale),
        None::<()>,
    )
    .await
}

pub async fn create_topic(
    token: Option<String>,
    tenant_slug: Option<String>,
    draft: TopicDraft,
) -> Result<TopicDetail, ApiError> {
    request_json(
        Method::POST,
        "/topics",
        token,
        tenant_slug,
        Some(draft.locale.clone()),
        Some(CreateTopicInput {
            locale: draft.locale.as_str(),
            category_id: draft.category_id.as_str(),
            title: draft.title.as_str(),
            slug: optional_text(draft.slug.as_str()),
            body: draft.body.as_str(),
            body_format: draft.body_format.as_str(),
            content_json: None,
            metadata: None,
            tags: draft.tags.as_slice(),
        }),
    )
    .await
}

pub async fn update_topic(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
    draft: TopicDraft,
) -> Result<TopicDetail, ApiError> {
    request_json(
        Method::PUT,
        format!("/topics/{id}").as_str(),
        token,
        tenant_slug,
        Some(draft.locale.clone()),
        Some(UpdateTopicInput {
            locale: draft.locale.as_str(),
            title: Some(draft.title.as_str()),
            body: Some(draft.body.as_str()),
            body_format: Some(draft.body_format.as_str()),
            content_json: None,
            metadata: None,
            tags: Some(draft.tags.as_slice()),
        }),
    )
    .await
}

pub async fn delete_topic(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
) -> Result<(), ApiError> {
    request_empty(
        Method::DELETE,
        format!("/topics/{id}").as_str(),
        token,
        tenant_slug,
        None,
        None::<()>,
    )
    .await
}

pub async fn fetch_replies(
    token: Option<String>,
    tenant_slug: Option<String>,
    topic_id: String,
    locale: String,
) -> Result<Vec<ReplyListItem>, ApiError> {
    request_json(
        Method::GET,
        format!("/topics/{topic_id}/replies?locale={locale}&page=1&per_page=20").as_str(),
        token,
        tenant_slug,
        Some(locale),
        None::<()>,
    )
    .await
}

fn optional_text(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}
