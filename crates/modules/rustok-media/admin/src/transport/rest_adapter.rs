#[cfg(target_arch = "wasm32")]
use leptos::web_sys;
use serde::Deserialize;

use super::ApiError;
use crate::model::MediaListItem;

#[derive(Debug, Deserialize)]
struct RestMediaItem {
    id: String,
    tenant_id: String,
    uploaded_by: Option<String>,
    filename: String,
    original_name: String,
    mime_type: String,
    size: i64,
    storage_driver: String,
    public_url: String,
    width: Option<i32>,
    height: Option<i32>,
    created_at: String,
}

#[derive(Debug, Deserialize)]
struct RestApiErrorPayload {
    error: Option<String>,
    message: Option<String>,
}

fn api_base_url() -> String {
    if let Some(url) = option_env!("RUSTOK_GRAPHQL_URL") {
        return url.trim_end_matches("/api/graphql").to_string();
    }

    #[cfg(target_arch = "wasm32")]
    {
        web_sys::window()
            .and_then(|window| window.location().origin().ok())
            .unwrap_or_else(|| "http://localhost:5150".to_string())
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::env::var("RUSTOK_API_URL").unwrap_or_else(|_| "http://localhost:5150".to_string())
    }
}

fn map_rest_media_item(item: RestMediaItem) -> MediaListItem {
    MediaListItem {
        id: item.id,
        tenant_id: item.tenant_id,
        uploaded_by: item.uploaded_by,
        filename: item.filename,
        original_name: item.original_name,
        mime_type: item.mime_type,
        size: item.size,
        storage_driver: item.storage_driver,
        public_url: item.public_url,
        width: item.width,
        height: item.height,
        created_at: item.created_at,
    }
}

pub(super) async fn upload_media_rest(
    file_name: String,
    content_type: String,
    data: Vec<u8>,
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<MediaListItem, ApiError> {
    let part = reqwest::multipart::Part::bytes(data)
        .file_name(file_name)
        .mime_str(&content_type)
        .map_err(|err| ApiError::Rest(format!("invalid content type: {err}")))?;
    let form = reqwest::multipart::Form::new().part("file", part);
    let client = reqwest::Client::new();
    let mut request = client
        .post(format!("{}/api/media", api_base_url()))
        .multipart(form);
    if let Some(token) = token {
        request = request.header(reqwest::header::AUTHORIZATION, format!("Bearer {token}"));
    }
    if let Some(tenant_slug) = tenant_slug {
        request = request.header("X-Tenant-ID", tenant_slug);
    }

    let response = request
        .send()
        .await
        .map_err(|err| ApiError::Rest(format!("upload failed: {err}")))?;
    if !response.status().is_success() {
        return Err(ApiError::Rest(extract_rest_error(response).await));
    }

    response
        .json::<RestMediaItem>()
        .await
        .map(map_rest_media_item)
        .map_err(|err| ApiError::Rest(format!("invalid upload response payload: {err}")))
}

async fn extract_rest_error(response: reqwest::Response) -> String {
    match response.json::<RestApiErrorPayload>().await {
        Ok(payload) => payload
            .message
            .or(payload.error)
            .unwrap_or_else(|| "request failed".to_string()),
        Err(_) => "request failed".to_string(),
    }
}
