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
        let trimmed = url.trim_end_matches('/');
        return trimmed.trim_end_matches("/api/graphql").to_string();
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

pub(crate) fn validate_upload_inputs(
    file_name: &str,
    data_len: usize,
) -> Result<&str, ApiError> {
    let clean = file_name.trim();
    if clean.is_empty() {
        return Err(ApiError::Validation(
            "upload file name cannot be empty".to_string(),
        ));
    }
    if data_len == 0 {
        return Err(ApiError::Validation(
            "upload file content cannot be empty".to_string(),
        ));
    }
    Ok(clean)
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
    let clean_file_name = validate_upload_inputs(&file_name, data.len())?.to_string();
    let part = reqwest::multipart::Part::bytes(data)
        .file_name(clean_file_name)
        .mime_str(&content_type)
        .map_err(|err| ApiError::Rest(format!("invalid content type: {err}")))?;
    let form = reqwest::multipart::Form::new().part("file", part);
    let client = reqwest::Client::new();
    let mut request = client
        .post(format!("{}/api/media", api_base_url()))
        .multipart(form);
    if let Some(token) = token.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        request = request.header(reqwest::header::AUTHORIZATION, format!("Bearer {token}"));
    }
    if let Some(slug) = tenant_slug.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        if let Ok(tenant_uuid) = uuid::Uuid::parse_str(slug)
            && tenant_uuid.is_nil()
        {
            return Err(ApiError::Validation("tenant id cannot be nil".to_string()));
        }
        request = request.header("X-Tenant-ID", slug);
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
    let status = response.status();
    match response.json::<RestApiErrorPayload>().await {
        Ok(payload) => payload
            .message
            .or(payload.error)
            .filter(|msg| !msg.trim().is_empty())
            .unwrap_or_else(|| format!("request failed with status {status}")),
        Err(_) => format!("request failed with status {status}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_upload_inputs_valid() {
        let result = validate_upload_inputs("image.png", 100);
        assert_eq!(result.expect("valid inputs"), "image.png");
    }

    #[test]
    fn validate_upload_inputs_trims_filename() {
        let result = validate_upload_inputs("  photo.jpg \n", 42);
        assert_eq!(result.expect("valid inputs"), "photo.jpg");
    }

    #[test]
    fn validate_upload_inputs_rejects_empty_filename() {
        let err = validate_upload_inputs("", 10).unwrap_err();
        match err {
            ApiError::Validation(msg) => assert!(msg.contains("cannot be empty")),
            other => panic!("expected Validation error, got {other:?}"),
        }

        let err_ws = validate_upload_inputs("   \t", 10).unwrap_err();
        match err_ws {
            ApiError::Validation(msg) => assert!(msg.contains("cannot be empty")),
            other => panic!("expected Validation error, got {other:?}"),
        }
    }

    #[test]
    fn validate_upload_inputs_rejects_empty_content() {
        let err = validate_upload_inputs("photo.jpg", 0).unwrap_err();
        match err {
            ApiError::Validation(msg) => assert!(msg.contains("content cannot be empty")),
            other => panic!("expected Validation error, got {other:?}"),
        }
    }

    #[test]
    fn map_rest_media_item_converts_correctly() {
        let item = RestMediaItem {
            id: "uuid-1".to_string(),
            tenant_id: "tenant-1".to_string(),
            uploaded_by: Some("user-1".to_string()),
            filename: "photo.jpg".to_string(),
            original_name: "orig.jpg".to_string(),
            mime_type: "image/jpeg".to_string(),
            size: 2048,
            storage_driver: "local".to_string(),
            public_url: "/uploads/photo.jpg".to_string(),
            width: Some(1920),
            height: Some(1080),
            created_at: "2026-10-21T00:00:00Z".to_string(),
        };
        let mapped = map_rest_media_item(item);
        assert_eq!(mapped.id, "uuid-1");
        assert_eq!(mapped.tenant_id, "tenant-1");
        assert_eq!(mapped.size, 2048);
        assert_eq!(mapped.width, Some(1920));
        assert_eq!(mapped.height, Some(1080));
    }
}
