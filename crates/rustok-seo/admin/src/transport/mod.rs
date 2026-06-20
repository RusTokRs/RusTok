use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

use rustok_seo::{
    SeoBulkApplyInput, SeoBulkApplyMode, SeoBulkExportInput, SeoBulkImportInput, SeoBulkJobRecord,
    SeoBulkJobStatus, SeoBulkListInput, SeoBulkPage, SeoBulkSelectionInput,
    SeoBulkSelectionPreviewRecord, SeoDiagnosticsSummaryRecord, SeoIndexDeliveryStatusRecord,
    SeoIndexRepairReplayInput, SeoIndexRepairReplayResultRecord, SeoModuleSettings,
    SeoRedirectInput, SeoRedirectRecord, SeoRobotsPreviewRecord, SeoSitemapStatusRecord,
    SeoTargetRegistryEntry,
};

mod native_server_adapter;

use native_server_adapter::{
    seo_bulk_items_native, seo_bulk_job_native, seo_bulk_jobs_native,
    seo_bulk_selection_preview_native, seo_bulk_targets_native, seo_diagnostics_native,
    seo_generate_sitemaps_native, seo_index_repair_replay_native, seo_index_tracking_native,
    seo_queue_bulk_apply_native, seo_queue_bulk_export_native, seo_queue_bulk_import_native,
    seo_redirects_native, seo_robots_preview_native, seo_save_settings_native, seo_settings_native,
    seo_sitemap_status_native, seo_upsert_redirect_native,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ApiError {
    ServerFn(String),
}

impl Display for ApiError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ServerFn(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for ApiError {}

impl From<ServerFnError> for ApiError {
    fn from(value: ServerFnError) -> Self {
        Self::ServerFn(value.to_string())
    }
}

pub async fn fetch_redirects() -> Result<Vec<SeoRedirectRecord>, ApiError> {
    seo_redirects_native().await.map_err(Into::into)
}

pub async fn save_redirect(input: SeoRedirectInput) -> Result<SeoRedirectRecord, ApiError> {
    seo_upsert_redirect_native(input).await.map_err(Into::into)
}

pub async fn fetch_sitemap_status() -> Result<SeoSitemapStatusRecord, ApiError> {
    seo_sitemap_status_native().await.map_err(Into::into)
}

pub async fn generate_sitemaps() -> Result<SeoSitemapStatusRecord, ApiError> {
    seo_generate_sitemaps_native().await.map_err(Into::into)
}

pub async fn fetch_settings() -> Result<SeoModuleSettings, ApiError> {
    seo_settings_native().await.map_err(Into::into)
}

pub async fn save_settings(settings: SeoModuleSettings) -> Result<SeoModuleSettings, ApiError> {
    seo_save_settings_native(settings).await.map_err(Into::into)
}

pub async fn fetch_robots_preview() -> Result<SeoRobotsPreviewRecord, ApiError> {
    seo_robots_preview_native().await.map_err(Into::into)
}

pub async fn fetch_diagnostics(
    locale: Option<String>,
) -> Result<SeoDiagnosticsSummaryRecord, ApiError> {
    seo_diagnostics_native(locale).await.map_err(Into::into)
}

pub async fn fetch_bulk_items(input: SeoBulkListInput) -> Result<SeoBulkPage, ApiError> {
    seo_bulk_items_native(input).await.map_err(Into::into)
}

pub async fn fetch_bulk_targets() -> Result<Vec<SeoTargetRegistryEntry>, ApiError> {
    seo_bulk_targets_native().await.map_err(Into::into)
}

pub async fn preview_bulk_selection(
    input: SeoBulkSelectionInput,
) -> Result<SeoBulkSelectionPreviewRecord, ApiError> {
    seo_bulk_selection_preview_native(input)
        .await
        .map_err(Into::into)
}

pub async fn fetch_bulk_jobs(
    limit: Option<i32>,
    status: Option<SeoBulkJobStatus>,
) -> Result<Vec<SeoBulkJobRecord>, ApiError> {
    seo_bulk_jobs_native(limit, status)
        .await
        .map_err(Into::into)
}

#[allow(dead_code)]
pub async fn fetch_bulk_job(job_id: String) -> Result<Option<SeoBulkJobRecord>, ApiError> {
    seo_bulk_job_native(job_id).await.map_err(Into::into)
}

pub async fn fetch_index_delivery_status(
    target_type: Option<String>,
) -> Result<SeoIndexDeliveryStatusRecord, ApiError> {
    let target_type = normalize_index_target_type(target_type).map_err(ApiError::ServerFn)?;
    seo_index_tracking_native(target_type)
        .await
        .map_err(Into::into)
}

pub async fn run_index_repair_replay(
    input: SeoIndexRepairReplayInput,
) -> Result<SeoIndexRepairReplayResultRecord, ApiError> {
    let input = normalize_index_repair_replay_input(input).map_err(ApiError::ServerFn)?;
    seo_index_repair_replay_native(input)
        .await
        .map_err(Into::into)
}

pub async fn queue_bulk_apply(input: SeoBulkApplyInput) -> Result<SeoBulkJobRecord, ApiError> {
    let input = normalize_preview_bulk_apply_input(input);
    seo_queue_bulk_apply_native(input).await.map_err(Into::into)
}

fn normalize_preview_bulk_apply_input(mut input: SeoBulkApplyInput) -> SeoBulkApplyInput {
    if input.apply_mode == SeoBulkApplyMode::PreviewOnly {
        input.publish_after_write = false;
    }
    input
}

fn normalize_index_target_type(target_type: Option<String>) -> Result<Option<String>, String> {
    let Some(value) = target_type else {
        return Ok(None);
    };

    let normalized = value.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return Ok(None);
    }

    match normalized.as_str() {
        "content" | "product" => Ok(Some(normalized)),
        _ => Err("Index target type must be `content` or `product`".to_string()),
    }
}

fn normalize_index_repair_replay_input(
    mut input: SeoIndexRepairReplayInput,
) -> Result<SeoIndexRepairReplayInput, String> {
    input.target_type = normalize_index_target_type(input.target_type)?;
    input.limit = input.limit.clamp(1, 500);
    Ok(input)
}

pub async fn queue_bulk_import(input: SeoBulkImportInput) -> Result<SeoBulkJobRecord, ApiError> {
    seo_queue_bulk_import_native(input)
        .await
        .map_err(Into::into)
}

pub async fn queue_bulk_export(input: SeoBulkExportInput) -> Result<SeoBulkJobRecord, ApiError> {
    seo_queue_bulk_export_native(input)
        .await
        .map_err(Into::into)
}

pub fn bulk_artifact_download_path(job_id: &str, artifact_id: &str) -> String {
    format!("/api/seo/bulk/jobs/{job_id}/artifacts/{artifact_id}")
}
