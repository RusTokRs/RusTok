use std::collections::{HashMap, HashSet};

use async_graphql::Json;
use chrono::{DateTime, Utc};
use csv::{ReaderBuilder, StringRecord, WriterBuilder};
use sea_orm::ActiveValue::Set;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect,
    TransactionTrait,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;

use rustok_api::TenantContext;
use rustok_content::resolve_by_locale_with_fallback;
use rustok_seo_targets::{SeoTargetBulkListRequest, SeoTargetSlug};

use crate::dto::{
    SeoBulkApplyInput, SeoBulkApplyMode, SeoBulkArtifactRecord, SeoBulkBoolFieldPatch,
    SeoBulkExportInput, SeoBulkFieldPatchMode, SeoBulkImportInput, SeoBulkJobOperationKind,
    SeoBulkJobRecord, SeoBulkJobStatus, SeoBulkJsonFieldPatch, SeoBulkListInput,
    SeoBulkMetaPatchInput, SeoBulkSelectionInput, SeoBulkSelectionMode,
    SeoBulkSelectionPreviewRecord, SeoBulkSource, SeoBulkStringFieldPatch, SeoFieldSource,
    SeoMetaInput, SeoMetaRecord, SeoMetaTranslationInput,
};
use crate::entities::{seo_bulk_job, seo_bulk_job_artifact, seo_bulk_job_item};
use crate::{SeoError, SeoResult};

use super::robots::is_valid_structured_data_payload;
use super::{SeoService, trimmed_option};

const CSV_MIME_TYPE: &str = "text/csv; charset=utf-8";
const CSV_HEADERS: [&str; 13] = [
    "target_kind",
    "target_id",
    "locale",
    "title",
    "description",
    "keywords",
    "canonical_url",
    "og_title",
    "og_description",
    "og_image",
    "structured_data",
    "noindex",
    "nofollow",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
struct NormalizedBulkListFilter {
    target_kind: SeoTargetSlug,
    locale: String,
    query: Option<String>,
    source: SeoBulkSource,
}

/// Normalizes the scope a bulk job runs over. Jobs cover the whole scope, so the
/// page window of the editor list is not part of the job filter.
fn normalize_bulk_list_input(
    input: SeoBulkListInput,
    fallback_locale: &str,
) -> SeoResult<NormalizedBulkListFilter> {
    Ok(NormalizedBulkListFilter {
        target_kind: input.target_kind,
        locale: super::normalize_effective_locale(input.locale.as_str(), fallback_locale)?,
        query: input
            .query
            .map(|value| value.trim().to_ascii_lowercase())
            .filter(|value| !value.is_empty()),
        source: input.source.unwrap_or(SeoBulkSource::Any),
    })
}

#[cfg(test)]
fn validate_bulk_patch(patch: &SeoBulkMetaPatchInput) -> SeoResult<()> {
    validate_bulk_patch_shape(patch)?;
    validate_bulk_patch_has_change(patch)
}

fn validate_bulk_apply(input: &SeoBulkApplyInput) -> SeoResult<()> {
    validate_bulk_patch_shape(&input.patch)?;
    match input.apply_mode {
        SeoBulkApplyMode::PreviewOnly
        | SeoBulkApplyMode::ApplyMissingOnly
        | SeoBulkApplyMode::OverwriteGeneratedOnly => Ok(()),
        SeoBulkApplyMode::ApplyMissingSchemaOnly => validate_schema_only_patch(&input.patch),
        SeoBulkApplyMode::ForceOverwriteExplicit => validate_bulk_patch_has_change(&input.patch),
    }
}

fn validate_schema_only_patch(patch: &SeoBulkMetaPatchInput) -> SeoResult<()> {
    let has_other_change = [
        patch.title.as_ref().map(|p| p.mode),
        patch.description.as_ref().map(|p| p.mode),
        patch.keywords.as_ref().map(|p| p.mode),
        patch.canonical_url.as_ref().map(|p| p.mode),
        patch.og_title.as_ref().map(|p| p.mode),
        patch.og_description.as_ref().map(|p| p.mode),
        patch.og_image.as_ref().map(|p| p.mode),
        patch.noindex.as_ref().map(|p| p.mode),
        patch.nofollow.as_ref().map(|p| p.mode),
    ]
    .into_iter()
    .flatten()
    .any(|mode| mode != SeoBulkFieldPatchMode::Keep);

    if has_other_change {
        return Err(SeoError::validation(
            "bulk apply mode `apply_missing_schema_only` may only modify structured_data",
        ));
    }

    let has_schema_change = patch
        .structured_data
        .as_ref()
        .map(|p| p.mode != SeoBulkFieldPatchMode::Keep)
        .unwrap_or(false);

    if !has_schema_change {
        return Err(SeoError::validation(
            "bulk apply mode `apply_missing_schema_only` requires a structured_data patch",
        ));
    }

    Ok(())
}

fn validate_bulk_patch_shape(patch: &SeoBulkMetaPatchInput) -> SeoResult<()> {
    validate_string_field_patch(patch.title.as_ref(), "title")?;
    validate_string_field_patch(patch.description.as_ref(), "description")?;
    validate_string_field_patch(patch.keywords.as_ref(), "keywords")?;
    validate_string_field_patch(patch.canonical_url.as_ref(), "canonical_url")?;
    validate_string_field_patch(patch.og_title.as_ref(), "og_title")?;
    validate_string_field_patch(patch.og_description.as_ref(), "og_description")?;
    validate_string_field_patch(patch.og_image.as_ref(), "og_image")?;
    validate_bool_field_patch(patch.noindex.as_ref(), "noindex")?;
    validate_bool_field_patch(patch.nofollow.as_ref(), "nofollow")?;
    validate_json_field_patch(patch.structured_data.as_ref(), "structured_data")
}

fn validate_bulk_patch_has_change(patch: &SeoBulkMetaPatchInput) -> SeoResult<()> {
    let has_change = [
        patch.title.as_ref().map(|value| value.mode),
        patch.description.as_ref().map(|value| value.mode),
        patch.keywords.as_ref().map(|value| value.mode),
        patch.canonical_url.as_ref().map(|value| value.mode),
        patch.og_title.as_ref().map(|value| value.mode),
        patch.og_description.as_ref().map(|value| value.mode),
        patch.og_image.as_ref().map(|value| value.mode),
        patch.noindex.as_ref().map(|value| value.mode),
        patch.nofollow.as_ref().map(|value| value.mode),
        patch.structured_data.as_ref().map(|value| value.mode),
    ]
    .into_iter()
    .flatten()
    .any(|mode| mode != SeoBulkFieldPatchMode::Keep);

    if !has_change {
        return Err(SeoError::validation(
            "bulk patch must change at least one field",
        ));
    }
    Ok(())
}

fn validate_string_field_patch(
    patch: Option<&SeoBulkStringFieldPatch>,
    field: &str,
) -> SeoResult<()> {
    if let Some(patch) = patch {
        match patch.mode {
            SeoBulkFieldPatchMode::Keep | SeoBulkFieldPatchMode::Clear => {}
            SeoBulkFieldPatchMode::Set => {
                let value = patch
                    .value
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .ok_or_else(|| {
                        SeoError::validation(format!(
                            "bulk patch field `{field}` requires a non-empty `value`"
                        ))
                    })?;
                if value.is_empty() {
                    return Err(SeoError::validation(format!(
                        "bulk patch field `{field}` requires a non-empty `value`"
                    )));
                }
            }
        }
    }
    Ok(())
}

fn validate_bool_field_patch(patch: Option<&SeoBulkBoolFieldPatch>, field: &str) -> SeoResult<()> {
    if let Some(patch) = patch
        && matches!(patch.mode, SeoBulkFieldPatchMode::Set)
        && patch.value.is_none()
    {
        return Err(SeoError::validation(format!(
            "bulk patch field `{field}` requires a boolean `value`"
        )));
    }
    Ok(())
}

fn validate_json_field_patch(patch: Option<&SeoBulkJsonFieldPatch>, field: &str) -> SeoResult<()> {
    if let Some(patch) = patch
        && matches!(patch.mode, SeoBulkFieldPatchMode::Set)
    {
        let value = patch.value.as_ref().ok_or_else(|| {
            SeoError::validation(format!(
                "bulk patch field `{field}` requires a JSON `value`"
            ))
        })?;
        if !is_valid_structured_data_payload(&value.0) {
            return Err(SeoError::validation(format!(
                "bulk patch field `{field}` must be a JSON-LD object, array, or @graph with at least one non-empty @type"
            )));
        }
    }
    Ok(())
}

fn apply_string_patch(
    current: Option<String>,
    patch: Option<&SeoBulkStringFieldPatch>,
) -> SeoResult<Option<String>> {
    match patch {
        None => Ok(current),
        Some(patch) => match patch.mode {
            SeoBulkFieldPatchMode::Keep => Ok(current),
            SeoBulkFieldPatchMode::Clear => Ok(None),
            SeoBulkFieldPatchMode::Set => Ok(Some(
                patch
                    .value
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .ok_or_else(|| SeoError::validation("bulk patch string value is required"))?
                    .to_string(),
            )),
        },
    }
}

fn apply_bool_patch(current: bool, patch: Option<&SeoBulkBoolFieldPatch>) -> SeoResult<bool> {
    match patch {
        None => Ok(current),
        Some(patch) => match patch.mode {
            SeoBulkFieldPatchMode::Keep => Ok(current),
            SeoBulkFieldPatchMode::Clear => Ok(false),
            SeoBulkFieldPatchMode::Set => patch
                .value
                .ok_or_else(|| SeoError::validation("bulk patch boolean value is required")),
        },
    }
}

fn apply_json_patch(
    current: Option<Value>,
    patch: Option<&SeoBulkJsonFieldPatch>,
) -> SeoResult<Option<Value>> {
    match patch {
        None => Ok(current),
        Some(patch) => match patch.mode {
            SeoBulkFieldPatchMode::Keep => Ok(current),
            SeoBulkFieldPatchMode::Clear => Ok(None),
            SeoBulkFieldPatchMode::Set => patch
                .value
                .clone()
                .map(|value| {
                    if is_valid_structured_data_payload(&value.0) {
                        Ok(Some(value.0))
                    } else {
                        Err(SeoError::validation(
                            "bulk patch JSON value must be a JSON-LD object, array, or @graph with at least one non-empty @type",
                        ))
                    }
                })
                .transpose()?
                .ok_or_else(|| SeoError::validation("bulk patch JSON value is required")),
        },
    }
}

fn map_bulk_source(value: &str) -> Option<SeoBulkSource> {
    match value {
        "explicit" => Some(SeoBulkSource::Explicit),
        "generated" => Some(SeoBulkSource::Generated),
        "fallback" => Some(SeoBulkSource::Fallback),
        "any" => Some(SeoBulkSource::Any),
        _ => None,
    }
}

fn map_bulk_job_model(
    model: seo_bulk_job::Model,
    artifacts_map: &HashMap<Uuid, Vec<SeoBulkArtifactRecord>>,
) -> SeoResult<SeoBulkJobRecord> {
    Ok(SeoBulkJobRecord {
        id: model.id,
        operation_kind: SeoBulkJobOperationKind::parse(model.operation_kind.as_str())
            .ok_or_else(|| SeoError::validation("invalid bulk operation kind"))?,
        status: SeoBulkJobStatus::parse(model.status.as_str())
            .ok_or_else(|| SeoError::validation("invalid bulk job status"))?,
        target_kind: SeoTargetSlug::new(model.target_kind.as_str())
            .map_err(|_| SeoError::validation("invalid bulk target kind"))?,
        locale: model.locale,
        publish_after_write: model.publish_after_write,
        matched_count: model.matched_count,
        processed_count: model.processed_count,
        succeeded_count: model.succeeded_count,
        failed_count: model.failed_count,
        skipped_count: model.skipped_count,
        artifact_count: model.artifact_count,
        last_error: model.last_error,
        created_at: DateTime::<Utc>::from(model.created_at),
        started_at: model.started_at.map(DateTime::<Utc>::from),
        completed_at: model.completed_at.map(DateTime::<Utc>::from),
        artifacts: artifacts_map.get(&model.id).cloned().unwrap_or_default(),
    })
}

fn limit_job_message(value: String) -> String {
    rustok_core::truncate(value.trim(), 2048)
}

#[allow(dead_code)]
fn parse_bulk_csv(
    expected_kind: SeoTargetSlug,
    expected_locale: &str,
    csv_utf8: &str,
) -> SeoResult<Vec<BulkImportRow>> {
    let mut reader = ReaderBuilder::new()
        .has_headers(true)
        .flexible(false)
        .from_reader(csv_utf8.as_bytes());
    let headers = reader
        .headers()
        .map_err(|err| SeoError::validation(format!("failed to read CSV headers: {err}")))?
        .clone();
    validate_csv_headers(&headers)?;

    let mut rows = Vec::new();
    for (index, result) in reader.records().enumerate() {
        let record = result.map_err(|err| {
            SeoError::validation(format!("failed to read CSV row {}: {err}", index + 2))
        })?;
        rows.push(parse_bulk_csv_record(
            &record,
            &expected_kind,
            expected_locale,
            index + 2,
        )?);
    }
    Ok(rows)
}

fn validate_csv_headers(headers: &StringRecord) -> SeoResult<()> {
    if headers.len() != CSV_HEADERS.len() {
        return Err(SeoError::validation(format!(
            "invalid CSV header length: expected {}, got {}",
            CSV_HEADERS.len(),
            headers.len()
        )));
    }

    for (index, expected) in CSV_HEADERS.iter().enumerate() {
        let actual = headers.get(index).unwrap_or_default();
        if actual != *expected {
            return Err(SeoError::validation(format!(
                "invalid CSV header at column {}: expected `{}`, got `{}`",
                index + 1,
                expected,
                actual
            )));
        }
    }
    Ok(())
}

fn parse_bulk_csv_record(
    record: &StringRecord,
    expected_kind: &SeoTargetSlug,
    expected_locale: &str,
    row_number: usize,
) -> SeoResult<BulkImportRow> {
    let target_kind = record.get(0).unwrap_or_default().trim().to_string();
    if target_kind != expected_kind.as_str() {
        return Err(SeoError::validation(format!(
            "row {row_number} has target_kind `{target_kind}` but current import scope is `{}`",
            expected_kind.as_str()
        )));
    }

    let target_id = Uuid::parse_str(record.get(1).unwrap_or_default().trim()).map_err(|err| {
        SeoError::validation(format!(
            "row {row_number} contains invalid target_id: {err}"
        ))
    })?;
    let locale =
        super::normalize_effective_locale(record.get(2).unwrap_or_default(), expected_locale)?;
    if locale != expected_locale {
        return Err(SeoError::validation(format!(
            "row {row_number} has locale `{locale}` but current import scope is `{expected_locale}`"
        )));
    }

    Ok(BulkImportRow {
        row_number,
        target_id,
        title: trimmed_string(record.get(3)),
        description: trimmed_string(record.get(4)),
        keywords: trimmed_string(record.get(5)),
        canonical_url: trimmed_string(record.get(6)),
        og_title: trimmed_string(record.get(7)),
        og_description: trimmed_string(record.get(8)),
        og_image: trimmed_string(record.get(9)),
        structured_data: parse_optional_json(record.get(10), row_number)?,
        noindex: parse_csv_bool(record.get(11), row_number, "noindex")?,
        nofollow: parse_csv_bool(record.get(12), row_number, "nofollow")?,
    })
}

fn parse_optional_json(value: Option<&str>, row_number: usize) -> SeoResult<Option<Value>> {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };

    serde_json::from_str::<Value>(value)
        .map(Some)
        .map_err(|err| {
            SeoError::validation(format!(
                "row {row_number} contains invalid structured_data JSON: {err}"
            ))
        })
}

fn parse_csv_bool(value: Option<&str>, row_number: usize, field: &str) -> SeoResult<bool> {
    let value = value.unwrap_or_default().trim().to_ascii_lowercase();
    match value.as_str() {
        "" | "false" | "0" | "no" => Ok(false),
        "true" | "1" | "yes" => Ok(true),
        _ => Err(SeoError::validation(format!(
            "row {row_number} contains invalid boolean `{field}` value `{value}`"
        ))),
    }
}

fn trimmed_string(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|s| {
            if let Some(stripped) = s.strip_prefix('\'')
                && stripped.starts_with(['=', '+', '-', '@', '\t', '\r'])
            {
                return stripped.to_string();
            }
            s.to_string()
        })
}

fn sanitize_csv_cell(value: String) -> String {
    if value.starts_with(['=', '+', '-', '@', '\t', '\r']) {
        format!("'{value}")
    } else {
        value
    }
}

#[allow(dead_code)]
fn export_csv_row(
    target_kind: SeoTargetSlug,
    target_id: Uuid,
    locale: &str,
    record: &SeoMetaRecord,
) -> Vec<String> {
    vec![
        target_kind.as_str().to_string(),
        target_id.to_string(),
        locale.to_string(),
        sanitize_csv_cell(record.translation.title.clone().unwrap_or_default()),
        sanitize_csv_cell(record.translation.description.clone().unwrap_or_default()),
        sanitize_csv_cell(record.translation.keywords.clone().unwrap_or_default()),
        sanitize_csv_cell(record.canonical_url.clone().unwrap_or_default()),
        sanitize_csv_cell(record.translation.og_title.clone().unwrap_or_default()),
        sanitize_csv_cell(
            record
                .translation
                .og_description
                .clone()
                .unwrap_or_default(),
        ),
        sanitize_csv_cell(record.translation.og_image.clone().unwrap_or_default()),
        record
            .structured_data
            .as_ref()
            .map(|value| value.0.to_string())
            .unwrap_or_default(),
        record.noindex.to_string(),
        record.nofollow.to_string(),
    ]
}

fn export_csv_row_values(
    target_kind: SeoTargetSlug,
    locale: &str,
    row: &BulkImportRow,
) -> Vec<String> {
    vec![
        target_kind.as_str().to_string(),
        row.target_id.to_string(),
        locale.to_string(),
        sanitize_csv_cell(row.title.clone().unwrap_or_default()),
        sanitize_csv_cell(row.description.clone().unwrap_or_default()),
        sanitize_csv_cell(row.keywords.clone().unwrap_or_default()),
        sanitize_csv_cell(row.canonical_url.clone().unwrap_or_default()),
        sanitize_csv_cell(row.og_title.clone().unwrap_or_default()),
        sanitize_csv_cell(row.og_description.clone().unwrap_or_default()),
        sanitize_csv_cell(row.og_image.clone().unwrap_or_default()),
        row.structured_data
            .as_ref()
            .map(Value::to_string)
            .unwrap_or_default(),
        row.noindex.to_string(),
        row.nofollow.to_string(),
    ]
}

fn build_failure_csv(rows: &[(Vec<String>, String)]) -> SeoResult<String> {
    let mut writer = WriterBuilder::new()
        .has_headers(false)
        .from_writer(Vec::<u8>::new());
    let mut headers = CSV_HEADERS
        .iter()
        .map(|item| (*item).to_string())
        .collect::<Vec<_>>();
    headers.push("error_message".to_string());
    writer.write_record(headers).map_err(|err| {
        SeoError::validation(format!("failed to write failure CSV header: {err}"))
    })?;
    for (row, message) in rows {
        let mut data = row.clone();
        data.push(message.clone());
        writer.write_record(data).map_err(|err| {
            SeoError::validation(format!("failed to write failure CSV row: {err}"))
        })?;
    }

    let bytes = writer.into_inner().map_err(|err| {
        SeoError::validation(format!("failed to finalize failure CSV writer: {err}"))
    })?;
    String::from_utf8(bytes)
        .map_err(|err| SeoError::validation(format!("failure CSV is not valid UTF-8: {err}")))
}

fn build_preview_csv(rows: &[Vec<String>]) -> SeoResult<String> {
    let mut writer = WriterBuilder::new()
        .has_headers(false)
        .from_writer(Vec::<u8>::new());
    writer.write_record(CSV_HEADERS).map_err(|err| {
        SeoError::validation(format!("failed to write preview CSV header: {err}"))
    })?;
    for row in rows {
        writer.write_record(row).map_err(|err| {
            SeoError::validation(format!("failed to write preview CSV row: {err}"))
        })?;
    }

    let bytes = writer.into_inner().map_err(|err| {
        SeoError::validation(format!("failed to finalize preview CSV writer: {err}"))
    })?;
    String::from_utf8(bytes)
        .map_err(|err| SeoError::validation(format!("preview CSV is not valid UTF-8: {err}")))
}

fn empty_csv_row(target_kind: &str, target_id: Uuid, locale: &str) -> Vec<String> {
    vec![
        target_kind.to_string(),
        target_id.to_string(),
        locale.to_string(),
        String::new(),
        String::new(),
        String::new(),
        String::new(),
        String::new(),
        String::new(),
        String::new(),
        String::new(),
        String::new(),
        String::new(),
    ]
}

fn preview_failure_row(target_kind: &str, target_id: Uuid, locale: &str) -> Vec<String> {
    empty_csv_row(target_kind, target_id, locale)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        SeoBulkFieldPatchMode, SeoMetaRecord, SeoMetaTranslationRecord, SeoTargetSlug,
        seo_builtin_slug,
    };

    fn page_slug() -> SeoTargetSlug {
        SeoTargetSlug::new(seo_builtin_slug::PAGE).expect("builtin SEO target slug must stay valid")
    }

    fn noop_patch() -> SeoBulkMetaPatchInput {
        SeoBulkMetaPatchInput {
            title: None,
            description: None,
            keywords: None,
            canonical_url: None,
            og_title: None,
            og_description: None,
            og_image: None,
            structured_data: None,
            noindex: None,
            nofollow: None,
        }
    }

    fn apply_input(apply_mode: SeoBulkApplyMode) -> SeoBulkApplyInput {
        SeoBulkApplyInput {
            selection: SeoBulkSelectionInput {
                mode: SeoBulkSelectionMode::CurrentFilterScope,
                selected_ids: Vec::new(),
                filter: Some(SeoBulkListInput {
                    target_kind: page_slug(),
                    locale: "en-US".to_string(),
                    query: None,
                    source: None,
                    after: None,
                    per_page: 20,
                }),
            },
            patch: noop_patch(),
            apply_mode,
            publish_after_write: true,
        }
    }

    #[test]
    fn normalize_bulk_list_input_canonicalizes_locale_and_query() {
        let filter = normalize_bulk_list_input(
            SeoBulkListInput {
                target_kind: page_slug(),
                locale: "en-us".to_string(),
                query: Some("  Sale  ".to_string()),
                source: None,
                after: None,
                per_page: 999,
            },
            "ru-RU",
        )
        .expect("normalize filter");

        assert_eq!(filter.locale, "en-US");
        assert_eq!(filter.query.as_deref(), Some("sale"));
        assert_eq!(filter.source, SeoBulkSource::Any);
    }

    #[test]
    fn validate_bulk_patch_rejects_empty_set_value() {
        let result = validate_bulk_patch(&SeoBulkMetaPatchInput {
            title: Some(SeoBulkStringFieldPatch {
                mode: SeoBulkFieldPatchMode::Set,
                value: Some("   ".to_string()),
            }),
            description: None,
            keywords: None,
            canonical_url: None,
            og_title: None,
            og_description: None,
            og_image: None,
            structured_data: None,
            noindex: None,
            nofollow: None,
        });

        assert!(
            result
                .expect_err("empty set should be rejected")
                .to_string()
                .contains("title"),
            "validation error should mention title"
        );
    }

    #[test]
    fn validate_bulk_patch_rejects_untyped_structured_data() {
        let result = validate_bulk_patch(&SeoBulkMetaPatchInput {
            title: None,
            description: None,
            keywords: None,
            canonical_url: None,
            og_title: None,
            og_description: None,
            og_image: None,
            structured_data: Some(SeoBulkJsonFieldPatch {
                mode: SeoBulkFieldPatchMode::Set,
                value: Some(Json(json!({"name": "Missing type"}))),
            }),
            noindex: None,
            nofollow: None,
        });

        assert!(
            result
                .expect_err("untyped structured data should be rejected")
                .to_string()
                .contains("structured_data"),
            "validation error should mention structured_data"
        );
    }

    #[test]
    fn validate_bulk_apply_allows_safe_noop_materialization_modes() {
        validate_bulk_apply(&apply_input(SeoBulkApplyMode::PreviewOnly))
            .expect("preview can run without patch changes");
        validate_bulk_apply(&apply_input(SeoBulkApplyMode::ApplyMissingOnly))
            .expect("missing-only materialization can run without patch changes");
        validate_bulk_apply(&apply_input(SeoBulkApplyMode::OverwriteGeneratedOnly))
            .expect("generated materialization can run without patch changes");

        let err = validate_bulk_apply(&apply_input(SeoBulkApplyMode::ForceOverwriteExplicit))
            .expect_err("force overwrite must require an explicit patch delta");
        assert!(err.to_string().contains("at least one field"));
    }

    #[test]
    fn validate_bulk_apply_allows_apply_missing_schema_only_with_structured_data_only() {
        let mut input = apply_input(SeoBulkApplyMode::ApplyMissingSchemaOnly);
        input.patch.structured_data = Some(SeoBulkJsonFieldPatch {
            mode: SeoBulkFieldPatchMode::Set,
            value: Some(Json(json!({"@type":"Product"}))),
        });
        validate_bulk_apply(&input).expect("schema-only patch should be valid");
    }

    #[test]
    fn validate_bulk_apply_rejects_apply_missing_schema_only_with_other_changes() {
        let mut input = apply_input(SeoBulkApplyMode::ApplyMissingSchemaOnly);
        input.patch.structured_data = Some(SeoBulkJsonFieldPatch {
            mode: SeoBulkFieldPatchMode::Set,
            value: Some(Json(json!({"@type":"Product"}))),
        });
        input.patch.title = Some(SeoBulkStringFieldPatch {
            mode: SeoBulkFieldPatchMode::Set,
            value: Some("Title".to_string()),
        });
        let err = validate_bulk_apply(&input).expect_err("mixed patch should fail");
        assert!(err.to_string().contains("apply_missing_schema_only"));
    }

    #[test]
    fn validate_bulk_apply_rejects_apply_missing_schema_only_without_structured_data_change() {
        let input = apply_input(SeoBulkApplyMode::ApplyMissingSchemaOnly);
        let err = validate_bulk_apply(&input).expect_err("no schema change should fail");
        assert!(err.to_string().contains("structured_data"));
    }

    #[test]
    fn parse_bulk_csv_accepts_valid_scope_bound_rows() {
        let csv = "target_kind,target_id,locale,title,description,keywords,canonical_url,og_title,og_description,og_image,structured_data,noindex,nofollow\npage,11111111-1111-1111-1111-111111111111,en-US,Title,Desc,kw,/canonical,OG Title,OG Desc,https://img.test/x.jpg,\"{\"\"@type\"\":\"\"Thing\"\"}\",true,false\n";
        let rows = parse_bulk_csv(page_slug(), "en-US", csv).expect("parse csv");

        assert_eq!(rows.len(), 1);
        assert_eq!(
            rows[0].target_id.to_string(),
            "11111111-1111-1111-1111-111111111111"
        );
        assert_eq!(rows[0].title.as_deref(), Some("Title"));
        assert_eq!(rows[0].structured_data, Some(json!({"@type":"Thing"})));
        assert!(rows[0].noindex);
        assert!(!rows[0].nofollow);
    }

    #[test]
    fn parse_bulk_csv_rejects_mismatched_scope() {
        let csv = "target_kind,target_id,locale,title,description,keywords,canonical_url,og_title,og_description,og_image,structured_data,noindex,nofollow\nproduct,11111111-1111-1111-1111-111111111111,en-US,Title,Desc,kw,/canonical,OG Title,OG Desc,https://img.test/x.jpg,,false,false\n";
        let err = parse_bulk_csv(page_slug(), "en-US", csv).expect_err("scope mismatch must fail");

        assert!(err.to_string().contains("target_kind"));
    }

    #[test]
    fn build_failure_csv_appends_error_column() {
        let csv = build_failure_csv(&[(
            empty_csv_row(
                "page",
                Uuid::parse_str("11111111-1111-1111-1111-111111111111").expect("uuid"),
                "en-US",
            ),
            "boom".to_string(),
        )])
        .expect("build failure csv");

        assert!(
            csv.lines()
                .next()
                .unwrap_or_default()
                .ends_with("error_message")
        );
        assert!(csv.contains("boom"));
    }

    #[test]
    fn build_preview_csv_uses_public_bulk_headers() {
        let csv = build_preview_csv(&[empty_csv_row(
            "page",
            Uuid::parse_str("11111111-1111-1111-1111-111111111111").expect("uuid"),
            "en-US",
        )])
        .expect("build preview csv");

        assert!(
            csv.lines()
                .next()
                .unwrap_or_default()
                .starts_with("target_kind,target_id,locale")
        );
        assert!(csv.contains("11111111-1111-1111-1111-111111111111"));
    }

    #[test]
    fn export_csv_row_serializes_structured_data_json() {
        let row = export_csv_row(
            page_slug(),
            Uuid::parse_str("11111111-1111-1111-1111-111111111111").expect("uuid"),
            "en-US",
            &SeoMetaRecord {
                target_kind: page_slug(),
                target_id: Uuid::parse_str("11111111-1111-1111-1111-111111111111").expect("uuid"),
                requested_locale: Some("en-US".to_string()),
                effective_locale: "en-US".to_string(),
                available_locales: vec!["en-US".to_string()],
                noindex: false,
                nofollow: true,
                canonical_url: Some("/canonical".to_string()),
                translation: SeoMetaTranslationRecord {
                    locale: "en-US".to_string(),
                    title: Some("Title".to_string()),
                    description: Some("Desc".to_string()),
                    keywords: Some("kw".to_string()),
                    og_title: Some("OG".to_string()),
                    og_description: Some("OG Desc".to_string()),
                    og_image: Some("https://img.test/x.jpg".to_string()),
                },
                source: "explicit".to_string(),
                open_graph: None,
                structured_data: Some(Json(json!({"@type":"Thing"}))),
                effective_state: crate::dto::SeoDocumentEffectiveState::default(),
            },
        );

        assert_eq!(row[10], "{\"@type\":\"Thing\"}");
        assert_eq!(row[11], "false");
        assert_eq!(row[12], "true");
    }

    #[test]
    fn export_csv_row_sanitizes_formula_injection() {
        let row = export_csv_row(
            page_slug(),
            Uuid::parse_str("11111111-1111-1111-1111-111111111111").expect("uuid"),
            "en-US",
            &SeoMetaRecord {
                target_kind: page_slug(),
                target_id: Uuid::parse_str("11111111-1111-1111-1111-111111111111").expect("uuid"),
                requested_locale: Some("en-US".to_string()),
                effective_locale: "en-US".to_string(),
                available_locales: vec!["en-US".to_string()],
                noindex: false,
                nofollow: false,
                canonical_url: Some("=HYPERLINK(\"http://evil.com\")".to_string()),
                translation: SeoMetaTranslationRecord {
                    locale: "en-US".to_string(),
                    title: Some("=SUM(1,2)".to_string()),
                    description: Some("+12345".to_string()),
                    keywords: Some("-keyword".to_string()),
                    og_title: Some("@special".to_string()),
                    og_description: Some("\tindented".to_string()),
                    og_image: Some("https://img.test/x.jpg".to_string()),
                },
                source: "explicit".to_string(),
                open_graph: None,
                structured_data: None,
                effective_state: crate::dto::SeoDocumentEffectiveState::default(),
            },
        );

        assert_eq!(row[3], "'=SUM(1,2)");
        assert_eq!(row[4], "'+12345");
        assert_eq!(row[5], "'-keyword");
        assert_eq!(row[6], "'=HYPERLINK(\"http://evil.com\")");
        assert_eq!(row[7], "'@special");
        assert_eq!(row[8], "'\tindented");

        let csv = "target_kind,target_id,locale,title,description,keywords,canonical_url,og_title,og_description,og_image,structured_data,noindex,nofollow\npage,11111111-1111-1111-1111-111111111111,en-US,\"'=SUM(1,2)\",\"'+12345\",\"'-keyword\",/canon,OG,OG,https://img.test/x.jpg,,false,false\n";
        let parsed = parse_bulk_csv(page_slug(), "en-US", csv).expect("parse csv");
        assert_eq!(parsed[0].title.as_deref(), Some("=SUM(1,2)"));
        assert_eq!(parsed[0].description.as_deref(), Some("+12345"));
        assert_eq!(parsed[0].keywords.as_deref(), Some("-keyword"));
    }

    fn string_mode(mode: SeoBulkFieldPatchMode) -> Option<SeoBulkFieldPatchMode> {
        Some(mode)
    }

    #[test]
    fn plan_bulk_field_preserves_explicit_fields_in_safe_modes() {
        use crate::dto::SeoFieldSource;
        // A Set targeting an operator-authored field is preserved per field instead of
        // failing the row, and Keep never disturbs it either.
        assert_eq!(
            plan_bulk_field(
                SeoBulkApplyMode::ApplyMissingOnly,
                SeoFieldSource::Explicit,
                true,
                BulkFieldClass::TemplateText,
                string_mode(SeoBulkFieldPatchMode::Set),
                "title",
            )
            .expect("explicit field must not fail the row"),
            BulkFieldPlan::Preserve
        );
        assert_eq!(
            plan_bulk_field(
                SeoBulkApplyMode::ApplyMissingOnly,
                SeoFieldSource::Explicit,
                true,
                BulkFieldClass::TemplateText,
                string_mode(SeoBulkFieldPatchMode::Keep),
                "title",
            )
            .expect("explicit field must stay untouched"),
            BulkFieldPlan::Preserve
        );
        assert_eq!(
            plan_bulk_field(
                SeoBulkApplyMode::ApplyMissingSchemaOnly,
                SeoFieldSource::Explicit,
                true,
                BulkFieldClass::CanonicalOrSchema,
                string_mode(SeoBulkFieldPatchMode::Set),
                "structured_data",
            )
            .expect("explicit schema must not fail the row"),
            BulkFieldPlan::Preserve
        );
    }

    #[test]
    fn plan_bulk_field_materializes_only_template_fields_on_keep() {
        use crate::dto::SeoFieldSource;
        // Missing-only materializes template text (its purpose) ...
        assert_eq!(
            plan_bulk_field(
                SeoBulkApplyMode::ApplyMissingOnly,
                SeoFieldSource::Generated,
                false,
                BulkFieldClass::TemplateText,
                string_mode(SeoBulkFieldPatchMode::Keep),
                "title",
            )
            .expect("template text materializes"),
            BulkFieldPlan::Materialize
        );
        // ... but never canonical or structured data without an explicit patch, so
        // materialization can no longer freeze derived routes or legacy payloads.
        assert_eq!(
            plan_bulk_field(
                SeoBulkApplyMode::ApplyMissingOnly,
                SeoFieldSource::Generated,
                false,
                BulkFieldClass::CanonicalOrSchema,
                string_mode(SeoBulkFieldPatchMode::Keep),
                "canonical_url",
            )
            .expect("canonical stays raw-absent"),
            BulkFieldPlan::Preserve
        );
        assert_eq!(
            plan_bulk_field(
                SeoBulkApplyMode::OverwriteGeneratedOnly,
                SeoFieldSource::Fallback,
                false,
                BulkFieldClass::TemplateText,
                string_mode(SeoBulkFieldPatchMode::Keep),
                "title",
            )
            .expect("generated-only leaves fallback text alone"),
            BulkFieldPlan::Preserve
        );
        assert_eq!(
            plan_bulk_field(
                SeoBulkApplyMode::OverwriteGeneratedOnly,
                SeoFieldSource::Generated,
                false,
                BulkFieldClass::TemplateText,
                string_mode(SeoBulkFieldPatchMode::Keep),
                "title",
            )
            .expect("generated template text materializes"),
            BulkFieldPlan::Materialize
        );
    }

    #[test]
    fn plan_bulk_field_applies_operator_intent_for_absent_fields() {
        use crate::dto::SeoFieldSource;
        // Set/Clear are operator intent: they apply to non-explicit fields in safe modes.
        assert_eq!(
            plan_bulk_field(
                SeoBulkApplyMode::ApplyMissingOnly,
                SeoFieldSource::Fallback,
                false,
                BulkFieldClass::CanonicalOrSchema,
                string_mode(SeoBulkFieldPatchMode::Set),
                "canonical_url",
            )
            .expect("explicit Set applies"),
            BulkFieldPlan::Applied
        );
        assert_eq!(
            plan_bulk_field(
                SeoBulkApplyMode::ApplyMissingOnly,
                SeoFieldSource::Generated,
                false,
                BulkFieldClass::TemplateText,
                string_mode(SeoBulkFieldPatchMode::Clear),
                "description",
            )
            .expect("explicit Clear applies"),
            BulkFieldPlan::Applied
        );
        // ForceOverwriteExplicit keeps whole-record semantics: every field writes.
        assert_eq!(
            plan_bulk_field(
                SeoBulkApplyMode::ForceOverwriteExplicit,
                SeoFieldSource::Explicit,
                true,
                BulkFieldClass::TemplateText,
                string_mode(SeoBulkFieldPatchMode::Keep),
                "title",
            )
            .expect("force materializes"),
            BulkFieldPlan::Materialize
        );
        assert_eq!(
            plan_bulk_field(
                SeoBulkApplyMode::ForceOverwriteExplicit,
                SeoFieldSource::Explicit,
                true,
                BulkFieldClass::CanonicalOrSchema,
                string_mode(SeoBulkFieldPatchMode::Set),
                "canonical_url",
            )
            .expect("force applies"),
            BulkFieldPlan::Applied
        );
    }

    #[test]
    fn plan_bulk_field_generated_only_still_loud_for_non_generated_targets() {
        use crate::dto::SeoFieldSource;
        let error = plan_bulk_field(
            SeoBulkApplyMode::OverwriteGeneratedOnly,
            SeoFieldSource::Fallback,
            false,
            BulkFieldClass::TemplateText,
            string_mode(SeoBulkFieldPatchMode::Set),
            "title",
        )
        .expect_err("setting a fallback field in generated-only mode must fail loudly");
        assert!(
            error.to_string().contains("requires generated SEO"),
            "unexpected error: {error}"
        );
        // Preview never writes.
        plan_bulk_field(
            SeoBulkApplyMode::PreviewOnly,
            SeoFieldSource::Generated,
            false,
            BulkFieldClass::TemplateText,
            None,
            "title",
        )
        .expect_err("preview mode must not write SEO records");
    }

    #[test]
    fn resolve_bulk_json_preserves_raw_absence_over_invalid_effective() {
        // A Keep/preserve decision keeps raw absence, healing legacy JSON `null` payloads
        // instead of re-persisting the unrenderable serving value.
        let effective = Some(Value::Null);
        let resolved = resolve_bulk_json(BulkFieldPlan::Preserve, None, effective, None)
            .expect("preserve resolves");
        assert_eq!(resolved, None);
        let materialized = resolve_bulk_json(
            BulkFieldPlan::Materialize,
            None,
            Some(json!({"@type": "WebPage"})),
            None,
        )
        .expect("materialize resolves");
        assert_eq!(materialized, Some(json!({"@type": "WebPage"})));
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
struct BulkTargetSummary {
    target_id: Uuid,
    label: String,
    route: String,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
struct BulkScopedSummary {
    summary: BulkTargetSummary,
    source: SeoBulkSource,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
struct BulkSelectionResolution {
    filter: NormalizedBulkListFilter,
    target_ids: Vec<Uuid>,
}

#[derive(Debug, Clone)]
struct BulkImportRow {
    row_number: usize,
    target_id: Uuid,
    title: Option<String>,
    description: Option<String>,
    keywords: Option<String>,
    canonical_url: Option<String>,
    og_title: Option<String>,
    og_description: Option<String>,
    og_image: Option<String>,
    structured_data: Option<Value>,
    noindex: bool,
    nofollow: bool,
}

/// Which class of SEO field a bulk field patch is deciding over.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BulkFieldClass {
    TemplateText,
    Robots,
    CanonicalOrSchema,
}

/// The decision for one field of one bulk apply row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BulkFieldPlan {
    /// Keep the stored operator value exactly as it is (absent stays absent).
    Preserve,
    /// Materialize the merged serving value into the explicit row.
    Materialize,
    /// Apply the operator's Set/Clear patch on top of the merged serving value.
    Applied,
}

/// Result of applying one bulk patch row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BulkApplyOutcome {
    Written { published_revision: Option<i32> },
    Skipped,
}

/// Decides one field of one bulk apply row under the active apply mode.
///
/// Safe modes never overwrite operator-authored values: a Set/Clear targeting an already
/// explicit field preserves just that field instead of failing the whole row, while every
/// other field of the row still receives the patch. Keep decisions only materialize template
/// fields (title/description/keywords/og_* and robots defaults); canonical_url and
/// structured_data stay raw-absent unless the operator supplied an explicit patch, so safe
/// materialization can never freeze derived routes or invalid legacy payloads.
fn plan_bulk_field(
    mode: SeoBulkApplyMode,
    source: SeoFieldSource,
    raw_present: bool,
    class: BulkFieldClass,
    patch_mode: Option<SeoBulkFieldPatchMode>,
    field_name: &str,
) -> SeoResult<BulkFieldPlan> {
    if mode == SeoBulkApplyMode::PreviewOnly {
        return Err(SeoError::validation(
            "bulk apply mode `preview_only` does not write SEO records",
        ));
    }
    let patch_mode = patch_mode.unwrap_or(SeoBulkFieldPatchMode::Keep);
    if patch_mode != SeoBulkFieldPatchMode::Keep {
        if mode == SeoBulkApplyMode::ForceOverwriteExplicit {
            return Ok(BulkFieldPlan::Applied);
        }
        if raw_present {
            // Operator-authored values survive every safe mode: preserve this field only.
            return Ok(BulkFieldPlan::Preserve);
        }
        if mode == SeoBulkApplyMode::OverwriteGeneratedOnly && source != SeoFieldSource::Generated {
            return Err(SeoError::validation(format!(
                "bulk apply mode `overwrite_generated_only` field `{field_name}` requires generated SEO, got `{}`",
                source.as_str()
            )));
        }
        return Ok(BulkFieldPlan::Applied);
    }
    if mode == SeoBulkApplyMode::ForceOverwriteExplicit {
        return Ok(BulkFieldPlan::Materialize);
    }
    if raw_present {
        return Ok(BulkFieldPlan::Preserve);
    }
    let materializes = match (mode, class) {
        (
            SeoBulkApplyMode::ApplyMissingOnly,
            BulkFieldClass::TemplateText | BulkFieldClass::Robots,
        ) => true,
        (
            SeoBulkApplyMode::OverwriteGeneratedOnly,
            BulkFieldClass::TemplateText | BulkFieldClass::Robots,
        ) => source == SeoFieldSource::Generated,
        _ => false,
    };
    Ok(if materializes {
        BulkFieldPlan::Materialize
    } else {
        BulkFieldPlan::Preserve
    })
}

/// Resolves a string field of one bulk apply row from its plan.
fn resolve_bulk_string(
    plan: BulkFieldPlan,
    raw: Option<String>,
    effective: Option<String>,
    patch: Option<&SeoBulkStringFieldPatch>,
) -> SeoResult<Option<String>> {
    match plan {
        BulkFieldPlan::Preserve => Ok(raw),
        BulkFieldPlan::Materialize => Ok(effective),
        BulkFieldPlan::Applied => apply_string_patch(effective, patch),
    }
}

/// Resolves the structured data field of one bulk apply row from its plan.
fn resolve_bulk_json(
    plan: BulkFieldPlan,
    raw: Option<Value>,
    effective: Option<Value>,
    patch: Option<&SeoBulkJsonFieldPatch>,
) -> SeoResult<Option<Value>> {
    match plan {
        BulkFieldPlan::Preserve => Ok(raw),
        BulkFieldPlan::Materialize => Ok(effective),
        BulkFieldPlan::Applied => apply_json_patch(effective, patch),
    }
}

#[allow(dead_code)]
impl SeoService {
    pub async fn preview_bulk_selection_count(
        &self,
        tenant: &TenantContext,
        selection: SeoBulkSelectionInput,
    ) -> SeoResult<SeoBulkSelectionPreviewRecord> {
        let resolution = self.resolve_bulk_selection(tenant, selection).await?;
        Ok(SeoBulkSelectionPreviewRecord {
            count: resolution.target_ids.len() as i32,
        })
    }

    pub async fn queue_bulk_apply(
        &self,
        tenant: &TenantContext,
        created_by: Option<Uuid>,
        input: SeoBulkApplyInput,
    ) -> SeoResult<SeoBulkJobRecord> {
        validate_bulk_apply(&input)?;
        let resolution = self
            .resolve_bulk_selection(tenant, input.selection.clone())
            .await?;
        let now = Utc::now().fixed_offset();
        let model = seo_bulk_job::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant.id),
            operation_kind: Set(SeoBulkJobOperationKind::Apply.as_str().to_string()),
            status: Set(SeoBulkJobStatus::Queued.as_str().to_string()),
            target_kind: Set(resolution.filter.target_kind.as_str().to_string()),
            locale: Set(resolution.filter.locale.clone()),
            filter_payload: Set(serde_json::to_value(&resolution.filter).map_err(|err| {
                SeoError::validation(format!("failed to serialize bulk filter: {err}"))
            })?),
            input_payload: Set(serde_json::to_value(&input).map_err(|err| {
                SeoError::validation(format!("failed to serialize bulk apply input: {err}"))
            })?),
            publish_after_write: Set(input.publish_after_write),
            matched_count: Set(resolution.target_ids.len() as i32),
            processed_count: Set(0),
            succeeded_count: Set(0),
            failed_count: Set(0),
            skipped_count: Set(0),
            artifact_count: Set(0),
            last_error: Set(None),
            created_by: Set(created_by),
            started_at: Set(None),
            completed_at: Set(None),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(&self.db)
        .await?;

        self.bulk_job(tenant.id, model.id)
            .await?
            .ok_or(SeoError::NotFound)
    }

    pub async fn queue_bulk_export(
        &self,
        tenant: &TenantContext,
        created_by: Option<Uuid>,
        input: SeoBulkExportInput,
    ) -> SeoResult<SeoBulkJobRecord> {
        let filter =
            normalize_bulk_list_input(input.filter.clone(), tenant.default_locale.as_str())?;
        let scoped = self.collect_bulk_scope(tenant, &filter).await?;
        let now = Utc::now().fixed_offset();
        let model = seo_bulk_job::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant.id),
            operation_kind: Set(SeoBulkJobOperationKind::ExportCsv.as_str().to_string()),
            status: Set(SeoBulkJobStatus::Queued.as_str().to_string()),
            target_kind: Set(filter.target_kind.as_str().to_string()),
            locale: Set(filter.locale.clone()),
            filter_payload: Set(serde_json::to_value(&filter).map_err(|err| {
                SeoError::validation(format!("failed to serialize bulk filter: {err}"))
            })?),
            input_payload: Set(serde_json::to_value(&input).map_err(|err| {
                SeoError::validation(format!("failed to serialize bulk export input: {err}"))
            })?),
            publish_after_write: Set(false),
            matched_count: Set(scoped.len() as i32),
            processed_count: Set(0),
            succeeded_count: Set(0),
            failed_count: Set(0),
            skipped_count: Set(0),
            artifact_count: Set(0),
            last_error: Set(None),
            created_by: Set(created_by),
            started_at: Set(None),
            completed_at: Set(None),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(&self.db)
        .await?;

        self.bulk_job(tenant.id, model.id)
            .await?
            .ok_or(SeoError::NotFound)
    }

    pub async fn queue_bulk_import(
        &self,
        tenant: &TenantContext,
        created_by: Option<Uuid>,
        input: SeoBulkImportInput,
    ) -> SeoResult<SeoBulkJobRecord> {
        let locale = super::normalize_effective_locale(
            input.locale.as_str(),
            tenant.default_locale.as_str(),
        )?;
        let rows = parse_bulk_csv(
            input.target_kind.clone(),
            locale.as_str(),
            input.csv_utf8.as_str(),
        )?;
        let now = Utc::now().fixed_offset();
        let model = seo_bulk_job::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant.id),
            operation_kind: Set(SeoBulkJobOperationKind::ImportCsv.as_str().to_string()),
            status: Set(SeoBulkJobStatus::Queued.as_str().to_string()),
            target_kind: Set(input.target_kind.as_str().to_string()),
            locale: Set(locale.clone()),
            filter_payload: Set(json!({
                "target_kind": input.target_kind.as_str(),
                "locale": locale,
            })),
            input_payload: Set(serde_json::to_value(&input).map_err(|err| {
                SeoError::validation(format!("failed to serialize bulk import input: {err}"))
            })?),
            publish_after_write: Set(input.publish_after_write),
            matched_count: Set(rows.len() as i32),
            processed_count: Set(0),
            succeeded_count: Set(0),
            failed_count: Set(0),
            skipped_count: Set(0),
            artifact_count: Set(0),
            last_error: Set(None),
            created_by: Set(created_by),
            started_at: Set(None),
            completed_at: Set(None),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(&self.db)
        .await?;

        self.bulk_job(tenant.id, model.id)
            .await?
            .ok_or(SeoError::NotFound)
    }

    pub async fn list_bulk_jobs(
        &self,
        tenant_id: Uuid,
        limit: usize,
        status: Option<SeoBulkJobStatus>,
    ) -> SeoResult<Vec<SeoBulkJobRecord>> {
        let mut query = seo_bulk_job::Entity::find()
            .filter(seo_bulk_job::Column::TenantId.eq(tenant_id))
            .order_by_desc(seo_bulk_job::Column::CreatedAt);
        if let Some(status) = status {
            query = query.filter(seo_bulk_job::Column::Status.eq(status.as_str()));
        }
        let jobs = query.limit(limit.max(1) as u64).all(&self.db).await?;
        let jobs = jobs.into_iter().take(limit.max(1)).collect::<Vec<_>>();
        let job_ids = jobs.iter().map(|job| job.id).collect::<Vec<_>>();
        let artifacts = self.load_bulk_artifacts_map(&job_ids).await?;

        jobs.into_iter()
            .map(|job| map_bulk_job_model(job, &artifacts))
            .collect()
    }

    pub async fn bulk_job(
        &self,
        tenant_id: Uuid,
        job_id: Uuid,
    ) -> SeoResult<Option<SeoBulkJobRecord>> {
        let Some(job) = seo_bulk_job::Entity::find()
            .filter(seo_bulk_job::Column::TenantId.eq(tenant_id))
            .filter(seo_bulk_job::Column::Id.eq(job_id))
            .one(&self.db)
            .await?
        else {
            return Ok(None);
        };
        let artifacts = self.load_bulk_artifacts_map(&[job.id]).await?;
        Ok(Some(map_bulk_job_model(job, &artifacts)?))
    }

    pub async fn bulk_artifact(
        &self,
        tenant_id: Uuid,
        job_id: Uuid,
        artifact_id: Uuid,
    ) -> SeoResult<Option<seo_bulk_job_artifact::Model>> {
        seo_bulk_job_artifact::Entity::find()
            .filter(seo_bulk_job_artifact::Column::TenantId.eq(tenant_id))
            .filter(seo_bulk_job_artifact::Column::JobId.eq(job_id))
            .filter(seo_bulk_job_artifact::Column::Id.eq(artifact_id))
            .one(&self.db)
            .await
            .map_err(Into::into)
    }

    pub async fn execute_next_bulk_job(&self) -> SeoResult<Option<SeoBulkJobRecord>> {
        let Some(job) = seo_bulk_job::Entity::find()
            .filter(seo_bulk_job::Column::Status.eq(SeoBulkJobStatus::Queued.as_str()))
            .order_by_asc(seo_bulk_job::Column::CreatedAt)
            .one(&self.db)
            .await?
        else {
            return Ok(None);
        };

        let now = Utc::now().fixed_offset();
        let mut active: seo_bulk_job::ActiveModel = job.clone().into();
        active.status = Set(SeoBulkJobStatus::Running.as_str().to_string());
        active.started_at = Set(Some(now));
        active.updated_at = Set(now);
        active.last_error = Set(None);
        let running = active.update(&self.db).await?;

        let result = match SeoBulkJobOperationKind::parse(running.operation_kind.as_str()) {
            Some(SeoBulkJobOperationKind::Apply) => self.execute_apply_job(&running).await,
            Some(SeoBulkJobOperationKind::ExportCsv) => self.execute_export_job(&running).await,
            Some(SeoBulkJobOperationKind::ImportCsv) => self.execute_import_job(&running).await,
            None => Err(SeoError::validation(format!(
                "unknown bulk operation kind `{}`",
                running.operation_kind
            ))),
        };

        if let Err(error) = result {
            self.fail_bulk_job(&running, error.to_string()).await?;
        }

        self.bulk_job(running.tenant_id, running.id).await
    }

    async fn load_tenant_context(&self, tenant_id: Uuid) -> SeoResult<TenantContext> {
        let tenant = rustok_tenant::TenantService::new(self.db.clone())
            .get_tenant(tenant_id)
            .await
            .map_err(|err| match err {
                rustok_tenant::TenantError::NotFound => SeoError::NotFound,
                rustok_tenant::TenantError::Database(db_err) => SeoError::Database(db_err),
                rustok_tenant::TenantError::Validation(msg) => SeoError::validation(msg),
                other => SeoError::configuration(other.to_string()),
            })?;
        Ok(TenantContext {
            id: tenant.id,
            name: tenant.name,
            slug: tenant.slug,
            domain: tenant.domain,
            settings: tenant.settings,
            default_locale: tenant.default_locale,
            is_active: tenant.is_active,
        })
    }

    async fn resolve_bulk_selection(
        &self,
        tenant: &TenantContext,
        selection: SeoBulkSelectionInput,
    ) -> SeoResult<BulkSelectionResolution> {
        let filter = selection
            .filter
            .ok_or_else(|| SeoError::validation("bulk selection filter is required"))?;
        let filter = normalize_bulk_list_input(filter, tenant.default_locale.as_str())?;
        let scoped = self.collect_bulk_scope(tenant, &filter).await?;
        let scoped_ids = scoped
            .iter()
            .map(|item| item.summary.target_id)
            .collect::<Vec<_>>();
        let target_ids = match selection.mode {
            SeoBulkSelectionMode::CurrentFilterScope => scoped_ids,
            SeoBulkSelectionMode::SelectedIds => {
                let selected = selection.selected_ids.into_iter().collect::<HashSet<_>>();
                scoped_ids
                    .into_iter()
                    .filter(|id| selected.contains(id))
                    .collect::<Vec<_>>()
            }
        };

        Ok(BulkSelectionResolution { filter, target_ids })
    }

    async fn collect_bulk_scope(
        &self,
        tenant: &TenantContext,
        filter: &NormalizedBulkListFilter,
    ) -> SeoResult<Vec<BulkScopedSummary>> {
        let summaries = self.collect_bulk_summaries(tenant, filter).await?;
        let mut scoped = Vec::new();
        for summary in summaries {
            let source = self
                .seo_meta(
                    tenant,
                    filter.target_kind.clone(),
                    summary.target_id,
                    Some(filter.locale.as_str()),
                )
                .await?
                .as_ref()
                .and_then(|record| map_bulk_source(record.source.as_str()))
                .unwrap_or(SeoBulkSource::Fallback);
            if filter.source != SeoBulkSource::Any && filter.source != source {
                continue;
            }
            if let Some(query) = filter.query.as_deref() {
                let haystacks = [
                    summary.label.to_ascii_lowercase(),
                    summary.route.to_ascii_lowercase(),
                    summary.target_id.to_string().to_ascii_lowercase(),
                ];
                if !haystacks.iter().any(|value| value.contains(query)) {
                    continue;
                }
            }
            scoped.push(BulkScopedSummary { summary, source });
        }

        Ok(scoped)
    }

    async fn collect_bulk_summaries(
        &self,
        tenant: &TenantContext,
        filter: &NormalizedBulkListFilter,
    ) -> SeoResult<Vec<BulkTargetSummary>> {
        let Some(provider) = self.registry.get(&filter.target_kind) else {
            return Ok(Vec::new());
        };
        let summaries = provider
            .list_bulk_summaries(
                &self.target_runtime(),
                SeoTargetBulkListRequest {
                    tenant_id: tenant.id,
                    default_locale: tenant.default_locale.as_str(),
                    locale: filter.locale.as_str(),
                },
            )
            .await
            .map_err(|error| {
                SeoError::validation(format!(
                    "SEO target provider `{}` failed to collect bulk summaries: {error}",
                    filter.target_kind.as_str()
                ))
            })?;

        Ok(summaries
            .into_iter()
            .map(|summary| BulkTargetSummary {
                target_id: summary.target_id,
                label: summary.label,
                route: summary.route,
            })
            .collect())
    }

    async fn execute_apply_job(&self, job: &seo_bulk_job::Model) -> SeoResult<()> {
        let tenant = self.load_tenant_context(job.tenant_id).await?;
        let input = serde_json::from_value::<SeoBulkApplyInput>(job.input_payload.clone())
            .map_err(|err| {
                SeoError::validation(format!("failed to decode bulk apply payload: {err}"))
            })?;
        let resolution = self
            .resolve_bulk_selection(&tenant, input.selection)
            .await?;
        let mut succeeded = 0_i32;
        let mut failed = 0_i32;
        let mut skipped = 0_i32;
        let mut preview_rows = Vec::<Vec<String>>::new();
        let mut failure_rows = Vec::new();

        for target_id in resolution.target_ids {
            if input.apply_mode == SeoBulkApplyMode::PreviewOnly {
                match self
                    .seo_meta(
                        &tenant,
                        resolution.filter.target_kind.clone(),
                        target_id,
                        Some(resolution.filter.locale.as_str()),
                    )
                    .await
                {
                    Ok(Some(record)) => {
                        preview_rows.push(export_csv_row(
                            resolution.filter.target_kind.clone(),
                            target_id,
                            resolution.filter.locale.as_str(),
                            &record,
                        ));
                        succeeded += 1;
                        self.insert_bulk_job_item(job, target_id, None, None)
                            .await?;
                    }
                    Ok(None) => {
                        failed += 1;
                        let message = "SEO target not found".to_string();
                        self.insert_bulk_job_item(job, target_id, Some(message.clone()), None)
                            .await?;
                        failure_rows.push((
                            preview_failure_row(
                                resolution.filter.target_kind.as_str(),
                                target_id,
                                resolution.filter.locale.as_str(),
                            ),
                            message,
                        ));
                    }
                    Err(error) => {
                        failed += 1;
                        let message = error.to_string();
                        self.insert_bulk_job_item(job, target_id, Some(message.clone()), None)
                            .await?;
                        failure_rows.push((
                            preview_failure_row(
                                resolution.filter.target_kind.as_str(),
                                target_id,
                                resolution.filter.locale.as_str(),
                            ),
                            message,
                        ));
                    }
                }
                continue;
            }

            match self
                .apply_bulk_patch_to_target(
                    &tenant,
                    job.id,
                    resolution.filter.target_kind.clone(),
                    resolution.filter.locale.as_str(),
                    target_id,
                    &input.patch,
                    input.apply_mode,
                    job.publish_after_write,
                )
                .await
            {
                Ok(BulkApplyOutcome::Written { published_revision }) => {
                    succeeded += 1;
                    self.insert_bulk_job_item(job, target_id, None, published_revision)
                        .await?;
                }
                Ok(BulkApplyOutcome::Skipped) => {
                    skipped += 1;
                    self.insert_bulk_skipped_job_item(job, target_id).await?;
                }
                Err(error) => {
                    failed += 1;
                    let message = error.to_string();
                    self.insert_bulk_job_item(job, target_id, Some(message.clone()), None)
                        .await?;
                    failure_rows.push((
                        empty_csv_row(
                            resolution.filter.target_kind.as_str(),
                            target_id,
                            resolution.filter.locale.as_str(),
                        ),
                        message,
                    ));
                }
            }
        }

        let mut artifacts = 0_i32;
        if !preview_rows.is_empty() {
            let content = build_preview_csv(&preview_rows)?;
            self.insert_bulk_job_artifact(
                job,
                "preview_report",
                format!("seo-bulk-preview-{}.csv", job.id),
                CSV_MIME_TYPE,
                content,
            )
            .await?;
            artifacts += 1;
        }
        if !failure_rows.is_empty() {
            let content = build_failure_csv(&failure_rows)?;
            self.insert_bulk_job_artifact(
                job,
                "failure_report",
                format!("seo-bulk-apply-failures-{}.csv", job.id),
                CSV_MIME_TYPE,
                content,
            )
            .await?;
            artifacts += 1;
        }

        self.finish_bulk_job(
            job,
            succeeded + failed + skipped,
            succeeded,
            failed,
            skipped,
            artifacts,
            None,
        )
        .await
    }

    async fn execute_export_job(&self, job: &seo_bulk_job::Model) -> SeoResult<()> {
        let tenant = self.load_tenant_context(job.tenant_id).await?;
        let input = serde_json::from_value::<SeoBulkExportInput>(job.input_payload.clone())
            .map_err(|err| {
                SeoError::validation(format!("failed to decode bulk export payload: {err}"))
            })?;
        let filter = normalize_bulk_list_input(input.filter, tenant.default_locale.as_str())?;
        let scoped = self.collect_bulk_scope(&tenant, &filter).await?;
        let mut writer = WriterBuilder::new()
            .has_headers(false)
            .from_writer(Vec::<u8>::new());
        writer.write_record(CSV_HEADERS).map_err(|err| {
            SeoError::validation(format!("failed to write export CSV header: {err}"))
        })?;

        let mut succeeded = 0_i32;
        let mut failed = 0_i32;
        let skipped = 0_i32;
        let mut artifacts = 0_i32;
        let mut failure_rows = Vec::new();

        for item in scoped {
            match self
                .seo_meta(
                    &tenant,
                    filter.target_kind.clone(),
                    item.summary.target_id,
                    Some(filter.locale.as_str()),
                )
                .await
            {
                Ok(Some(record)) => {
                    writer
                        .write_record(export_csv_row(
                            filter.target_kind.clone(),
                            item.summary.target_id,
                            filter.locale.as_str(),
                            &record,
                        ))
                        .map_err(|err| {
                            SeoError::validation(format!(
                                "failed to serialize export row for {}: {err}",
                                item.summary.target_id
                            ))
                        })?;
                    succeeded += 1;
                    self.insert_bulk_job_item(job, item.summary.target_id, None, None)
                        .await?;
                }
                Ok(None) => {
                    failed += 1;
                    let message = "SEO target not found".to_string();
                    self.insert_bulk_job_item(
                        job,
                        item.summary.target_id,
                        Some(message.clone()),
                        None,
                    )
                    .await?;
                    failure_rows.push((
                        empty_csv_row(
                            filter.target_kind.as_str(),
                            item.summary.target_id,
                            filter.locale.as_str(),
                        ),
                        message,
                    ));
                }
                Err(error) => {
                    failed += 1;
                    let message = error.to_string();
                    self.insert_bulk_job_item(
                        job,
                        item.summary.target_id,
                        Some(message.clone()),
                        None,
                    )
                    .await?;
                    failure_rows.push((
                        empty_csv_row(
                            filter.target_kind.as_str(),
                            item.summary.target_id,
                            filter.locale.as_str(),
                        ),
                        message,
                    ));
                }
            }
        }

        let bytes = writer.into_inner().map_err(|err| {
            SeoError::validation(format!("failed to finalize export CSV writer: {err}"))
        })?;
        let content = String::from_utf8(bytes)
            .map_err(|err| SeoError::validation(format!("export CSV is not valid UTF-8: {err}")))?;
        self.insert_bulk_job_artifact(
            job,
            "export_csv",
            format!(
                "seo-bulk-export-{}-{}-{}.csv",
                filter.target_kind.as_str(),
                filter.locale,
                job.id
            ),
            CSV_MIME_TYPE,
            content,
        )
        .await?;
        artifacts += 1;

        if !failure_rows.is_empty() {
            let failure_csv = build_failure_csv(&failure_rows)?;
            self.insert_bulk_job_artifact(
                job,
                "failure_report",
                format!("seo-bulk-export-failures-{}.csv", job.id),
                CSV_MIME_TYPE,
                failure_csv,
            )
            .await?;
            artifacts += 1;
        }

        self.finish_bulk_job(
            job,
            succeeded + failed + skipped,
            succeeded,
            failed,
            skipped,
            artifacts,
            None,
        )
        .await
    }

    async fn execute_import_job(&self, job: &seo_bulk_job::Model) -> SeoResult<()> {
        let tenant = self.load_tenant_context(job.tenant_id).await?;
        let input = serde_json::from_value::<SeoBulkImportInput>(job.input_payload.clone())
            .map_err(|err| {
                SeoError::validation(format!("failed to decode bulk import payload: {err}"))
            })?;
        let locale = super::normalize_effective_locale(
            input.locale.as_str(),
            tenant.default_locale.as_str(),
        )?;
        let rows = parse_bulk_csv(
            input.target_kind.clone(),
            locale.as_str(),
            input.csv_utf8.as_str(),
        )?;
        let mut succeeded = 0_i32;
        let mut failed = 0_i32;
        let skipped = 0_i32;
        let mut artifacts = 0_i32;
        let mut failure_rows = Vec::new();

        for row in rows {
            match self
                .import_bulk_row(
                    &tenant,
                    job.id,
                    input.target_kind.clone(),
                    locale.as_str(),
                    &row,
                    job.publish_after_write,
                )
                .await
            {
                Ok(revision) => {
                    succeeded += 1;
                    self.insert_bulk_job_item(job, row.target_id, None, revision)
                        .await?;
                }
                Err(error) => {
                    failed += 1;
                    let message = error.to_string();
                    self.insert_bulk_job_item(job, row.target_id, Some(message.clone()), None)
                        .await?;
                    failure_rows.push((
                        export_csv_row_values(input.target_kind.clone(), locale.as_str(), &row),
                        format!("row {}: {}", row.row_number, message),
                    ));
                }
            }
        }

        if !failure_rows.is_empty() {
            let content = build_failure_csv(&failure_rows)?;
            self.insert_bulk_job_artifact(
                job,
                "failure_report",
                format!("seo-bulk-import-failures-{}.csv", job.id),
                CSV_MIME_TYPE,
                content,
            )
            .await?;
            artifacts += 1;
        }

        self.finish_bulk_job(
            job,
            succeeded + failed + skipped,
            succeeded,
            failed,
            skipped,
            artifacts,
            None,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    async fn apply_bulk_patch_to_target(
        &self,
        tenant: &TenantContext,
        job_id: Uuid,
        target_kind: SeoTargetSlug,
        locale: &str,
        target_id: Uuid,
        patch: &SeoBulkMetaPatchInput,
        apply_mode: SeoBulkApplyMode,
        publish_after_write: bool,
    ) -> SeoResult<BulkApplyOutcome> {
        if apply_mode == SeoBulkApplyMode::PreviewOnly {
            return Err(SeoError::validation(
                "bulk apply mode `preview_only` does not write SEO records",
            ));
        }
        let current = self
            .seo_meta(tenant, target_kind.clone(), target_id, Some(locale))
            .await?
            .ok_or(SeoError::NotFound)?;

        // Raw operator-authored values drive the per-field preservation rules. They are read
        // from the stored record so a missing (NULL/absent) column stays absent even when the
        // merged serving value carries generated or fallback content for that field.
        let explicit = self
            .load_explicit_meta(tenant.id, target_kind.clone(), target_id)
            .await?;
        let explicit_row = explicit.is_some();
        let raw_translation = explicit.as_ref().and_then(|loaded| {
            resolve_by_locale_with_fallback(
                loaded.translations.as_slice(),
                locale,
                Some(tenant.default_locale.as_str()),
                |item| item.locale.as_str(),
            )
            .item
            .cloned()
        });
        let raw_title = raw_translation
            .as_ref()
            .and_then(|item| trimmed_option(item.title.clone()));
        let raw_description = raw_translation
            .as_ref()
            .and_then(|item| trimmed_option(item.description.clone()));
        let raw_keywords = raw_translation
            .as_ref()
            .and_then(|item| trimmed_option(item.keywords.clone()));
        let raw_og_title = raw_translation
            .as_ref()
            .and_then(|item| trimmed_option(item.og_title.clone()));
        let raw_og_description = raw_translation
            .as_ref()
            .and_then(|item| trimmed_option(item.og_description.clone()));
        let raw_og_image = raw_translation
            .as_ref()
            .and_then(|item| trimmed_option(item.og_image.clone()));
        let raw_canonical = explicit
            .as_ref()
            .and_then(|loaded| trimmed_option(loaded.meta.canonical_url.clone()));
        // Invalid legacy payloads (for example JSON `null`) count as absent so safe modes
        // heal them to NULL instead of re-persisting an unrenderable value.
        let raw_structured_data = explicit
            .as_ref()
            .and_then(|loaded| loaded.meta.structured_data.clone())
            .filter(is_valid_structured_data_payload);
        let effective_structured_data = current
            .structured_data
            .as_ref()
            .map(|value| value.0.clone())
            .filter(is_valid_structured_data_payload);

        let title_plan = plan_bulk_field(
            apply_mode,
            current.effective_state.title.source,
            raw_title.is_some(),
            BulkFieldClass::TemplateText,
            patch.title.as_ref().map(|value| value.mode),
            "title",
        )?;
        let description_plan = plan_bulk_field(
            apply_mode,
            current.effective_state.description.source,
            raw_description.is_some(),
            BulkFieldClass::TemplateText,
            patch.description.as_ref().map(|value| value.mode),
            "description",
        )?;
        let keywords_plan = plan_bulk_field(
            apply_mode,
            current.effective_state.keywords.source,
            raw_keywords.is_some(),
            BulkFieldClass::TemplateText,
            patch.keywords.as_ref().map(|value| value.mode),
            "keywords",
        )?;
        let og_title_plan = plan_bulk_field(
            apply_mode,
            current.effective_state.open_graph.source,
            raw_og_title.is_some(),
            BulkFieldClass::TemplateText,
            patch.og_title.as_ref().map(|value| value.mode),
            "og_title",
        )?;
        let og_description_plan = plan_bulk_field(
            apply_mode,
            current.effective_state.open_graph.source,
            raw_og_description.is_some(),
            BulkFieldClass::TemplateText,
            patch.og_description.as_ref().map(|value| value.mode),
            "og_description",
        )?;
        let og_image_plan = plan_bulk_field(
            apply_mode,
            current.effective_state.open_graph.source,
            raw_og_image.is_some(),
            BulkFieldClass::TemplateText,
            patch.og_image.as_ref().map(|value| value.mode),
            "og_image",
        )?;
        let canonical_plan = plan_bulk_field(
            apply_mode,
            current.effective_state.canonical_url.source,
            raw_canonical.is_some(),
            BulkFieldClass::CanonicalOrSchema,
            patch.canonical_url.as_ref().map(|value| value.mode),
            "canonical_url",
        )?;
        let structured_data_plan = plan_bulk_field(
            apply_mode,
            current.effective_state.structured_data.source,
            raw_structured_data.is_some(),
            BulkFieldClass::CanonicalOrSchema,
            patch.structured_data.as_ref().map(|value| value.mode),
            "structured_data",
        )?;
        // Robots are row-level booleans: raw presence means the explicit row exists.
        let noindex_plan = plan_bulk_field(
            apply_mode,
            current.effective_state.robots.source,
            explicit_row,
            BulkFieldClass::Robots,
            patch.noindex.as_ref().map(|value| value.mode),
            "noindex",
        )?;
        let nofollow_plan = plan_bulk_field(
            apply_mode,
            current.effective_state.robots.source,
            explicit_row,
            BulkFieldClass::Robots,
            patch.nofollow.as_ref().map(|value| value.mode),
            "nofollow",
        )?;

        // A row is written only when at least one field is patched or materialized. Robots
        // defaults alone never create a row; they ride along with another written field.
        let mut row_written = [
            title_plan,
            description_plan,
            keywords_plan,
            og_title_plan,
            og_description_plan,
            og_image_plan,
            canonical_plan,
            structured_data_plan,
        ]
        .iter()
        .any(|plan| *plan != BulkFieldPlan::Preserve);
        row_written = row_written
            || noindex_plan == BulkFieldPlan::Applied
            || nofollow_plan == BulkFieldPlan::Applied;
        if !row_written {
            return Ok(BulkApplyOutcome::Skipped);
        }

        let input = SeoMetaInput {
            target_kind: target_kind.clone(),
            target_id,
            noindex: match noindex_plan {
                BulkFieldPlan::Applied => {
                    apply_bool_patch(current.noindex, patch.noindex.as_ref())?
                }
                BulkFieldPlan::Preserve | BulkFieldPlan::Materialize => current.noindex,
            },
            nofollow: match nofollow_plan {
                BulkFieldPlan::Applied => {
                    apply_bool_patch(current.nofollow, patch.nofollow.as_ref())?
                }
                BulkFieldPlan::Preserve | BulkFieldPlan::Materialize => current.nofollow,
            },
            canonical_url: resolve_bulk_string(
                canonical_plan,
                raw_canonical,
                current.canonical_url.clone(),
                patch.canonical_url.as_ref(),
            )?,
            structured_data: resolve_bulk_json(
                structured_data_plan,
                raw_structured_data,
                effective_structured_data,
                patch.structured_data.as_ref(),
            )?
            .map(Json),
            translations: vec![SeoMetaTranslationInput {
                locale: locale.to_string(),
                title: resolve_bulk_string(
                    title_plan,
                    raw_title,
                    current.translation.title.clone(),
                    patch.title.as_ref(),
                )?,
                description: resolve_bulk_string(
                    description_plan,
                    raw_description,
                    current.translation.description.clone(),
                    patch.description.as_ref(),
                )?,
                keywords: resolve_bulk_string(
                    keywords_plan,
                    raw_keywords,
                    current.translation.keywords.clone(),
                    patch.keywords.as_ref(),
                )?,
                og_title: resolve_bulk_string(
                    og_title_plan,
                    raw_og_title,
                    current.translation.og_title.clone(),
                    patch.og_title.as_ref(),
                )?,
                og_description: resolve_bulk_string(
                    og_description_plan,
                    raw_og_description,
                    current.translation.og_description.clone(),
                    patch.og_description.as_ref(),
                )?,
                og_image: resolve_bulk_string(
                    og_image_plan,
                    raw_og_image,
                    current.translation.og_image.clone(),
                    patch.og_image.as_ref(),
                )?,
            }],
        };

        self.upsert_meta_with_transition(tenant, input, Some(format!("job:{job_id}")))
            .await?;
        if publish_after_write {
            let revision = self
                .publish_revision(
                    tenant,
                    target_kind,
                    target_id,
                    Some(format!("bulk job {} apply", job_id)),
                )
                .await?;
            return Ok(BulkApplyOutcome::Written {
                published_revision: Some(revision.revision),
            });
        }

        Ok(BulkApplyOutcome::Written {
            published_revision: None,
        })
    }

    async fn import_bulk_row(
        &self,
        tenant: &TenantContext,
        job_id: Uuid,
        target_kind: SeoTargetSlug,
        locale: &str,
        row: &BulkImportRow,
        publish_after_write: bool,
    ) -> SeoResult<Option<i32>> {
        let input = SeoMetaInput {
            target_kind: target_kind.clone(),
            target_id: row.target_id,
            noindex: row.noindex,
            nofollow: row.nofollow,
            canonical_url: row.canonical_url.clone(),
            structured_data: row.structured_data.clone().map(Json),
            translations: vec![SeoMetaTranslationInput {
                locale: locale.to_string(),
                title: row.title.clone(),
                description: row.description.clone(),
                keywords: row.keywords.clone(),
                og_title: row.og_title.clone(),
                og_description: row.og_description.clone(),
                og_image: row.og_image.clone(),
            }],
        };

        self.upsert_meta_with_transition(tenant, input, Some(format!("job:{job_id}")))
            .await?;
        if publish_after_write {
            let revision = self
                .publish_revision(
                    tenant,
                    target_kind,
                    row.target_id,
                    Some(format!("bulk job {} import_csv", job_id)),
                )
                .await?;
            return Ok(Some(revision.revision));
        }

        Ok(None)
    }

    async fn fail_bulk_job(&self, job: &seo_bulk_job::Model, message: String) -> SeoResult<()> {
        let now = Utc::now().fixed_offset();
        let txn = self.db.begin().await?;
        let mut active: seo_bulk_job::ActiveModel = job.clone().into();
        active.status = Set(SeoBulkJobStatus::Failed.as_str().to_string());
        active.last_error = Set(Some(limit_job_message(message)));
        active.completed_at = Set(Some(now));
        active.updated_at = Set(now);
        let updated = active.update(&txn).await?;

        let terminal_dispatch = self
            .publish_seo_bulk_terminal_event_in_tx(
                &txn,
                updated.tenant_id,
                updated.id,
                updated.target_kind.as_str(),
                updated.locale.as_str(),
                updated.status.as_str(),
                updated.processed_count,
                updated.succeeded_count,
                updated.failed_count,
            )
            .await?;
        txn.commit().await?;
        self.dispatch_seo_bulk_terminal_reindex(updated.tenant_id, terminal_dispatch)
            .await;

        Ok(())
    }

    async fn finish_bulk_job(
        &self,
        job: &seo_bulk_job::Model,
        processed_count: i32,
        succeeded_count: i32,
        failed_count: i32,
        skipped_count: i32,
        artifact_count: i32,
        last_error: Option<String>,
    ) -> SeoResult<()> {
        // Skipped rows are processed but untouched: they never count as failures, so a run over
        // already-explicit targets finishes `completed`, not `failed`.
        let status = if failed_count == 0 {
            SeoBulkJobStatus::Completed
        } else if succeeded_count == 0 {
            SeoBulkJobStatus::Failed
        } else {
            SeoBulkJobStatus::Partial
        };
        let now = Utc::now().fixed_offset();
        let txn = self.db.begin().await?;
        let mut active: seo_bulk_job::ActiveModel = job.clone().into();
        active.status = Set(status.as_str().to_string());
        active.processed_count = Set(processed_count);
        active.succeeded_count = Set(succeeded_count);
        active.failed_count = Set(failed_count);
        active.skipped_count = Set(skipped_count);
        active.artifact_count = Set(artifact_count);
        active.last_error = Set(last_error.map(limit_job_message));
        active.completed_at = Set(Some(now));
        active.updated_at = Set(now);
        let updated = active.update(&txn).await?;

        let terminal_dispatch = self
            .publish_seo_bulk_terminal_event_in_tx(
                &txn,
                updated.tenant_id,
                updated.id,
                updated.target_kind.as_str(),
                updated.locale.as_str(),
                updated.status.as_str(),
                updated.processed_count,
                updated.succeeded_count,
                updated.failed_count,
            )
            .await?;
        txn.commit().await?;
        self.dispatch_seo_bulk_terminal_reindex(updated.tenant_id, terminal_dispatch)
            .await;

        Ok(())
    }

    async fn insert_bulk_job_item(
        &self,
        job: &seo_bulk_job::Model,
        target_id: Uuid,
        error_message: Option<String>,
        published_revision: Option<i32>,
    ) -> SeoResult<()> {
        let status = if error_message.is_some() {
            "failed"
        } else {
            "completed"
        };
        self.insert_bulk_job_item_with_status(
            job,
            target_id,
            status,
            error_message,
            published_revision,
        )
        .await
    }

    async fn insert_bulk_skipped_job_item(
        &self,
        job: &seo_bulk_job::Model,
        target_id: Uuid,
    ) -> SeoResult<()> {
        self.insert_bulk_job_item_with_status(job, target_id, "skipped", None, None)
            .await
    }

    async fn insert_bulk_job_item_with_status(
        &self,
        job: &seo_bulk_job::Model,
        target_id: Uuid,
        status: &str,
        error_message: Option<String>,
        published_revision: Option<i32>,
    ) -> SeoResult<()> {
        let now = Utc::now().fixed_offset();
        seo_bulk_job_item::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(job.tenant_id),
            job_id: Set(job.id),
            target_id: Set(target_id),
            status: Set(status.to_string()),
            error_message: Set(error_message.map(limit_job_message)),
            published_revision: Set(published_revision),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(&self.db)
        .await?;
        Ok(())
    }

    async fn insert_bulk_job_artifact(
        &self,
        job: &seo_bulk_job::Model,
        kind: impl Into<String>,
        file_name: impl Into<String>,
        mime_type: impl Into<String>,
        content: String,
    ) -> SeoResult<seo_bulk_job_artifact::Model> {
        let now = Utc::now().fixed_offset();
        seo_bulk_job_artifact::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(job.tenant_id),
            job_id: Set(job.id),
            kind: Set(kind.into()),
            file_name: Set(file_name.into()),
            mime_type: Set(mime_type.into()),
            content: Set(content),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(&self.db)
        .await
        .map_err(Into::into)
    }

    async fn load_bulk_artifacts_map(
        &self,
        job_ids: &[Uuid],
    ) -> SeoResult<HashMap<Uuid, Vec<SeoBulkArtifactRecord>>> {
        if job_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let artifacts = seo_bulk_job_artifact::Entity::find()
            .filter(seo_bulk_job_artifact::Column::JobId.is_in(job_ids.to_vec()))
            .order_by_asc(seo_bulk_job_artifact::Column::CreatedAt)
            .all(&self.db)
            .await?;
        let mut map = HashMap::<Uuid, Vec<SeoBulkArtifactRecord>>::new();
        for artifact in artifacts {
            map.entry(artifact.job_id)
                .or_default()
                .push(SeoBulkArtifactRecord {
                    id: artifact.id,
                    job_id: artifact.job_id,
                    kind: artifact.kind,
                    file_name: artifact.file_name,
                    mime_type: artifact.mime_type,
                    created_at: DateTime::<Utc>::from(artifact.created_at),
                });
        }
        Ok(map)
    }
}
const BULK_APPLY_CHUNK_SIZE: usize = 50;
const MAX_BULK_TARGETS: usize = 100_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct QueuedBulkApplyPayload {
    input: SeoBulkApplyInput,
    target_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, Copy)]
struct BulkJobProgress {
    processed: i32,
    succeeded: i32,
    failed: i32,
    skipped: i32,
    artifacts: i32,
}

#[derive(Debug, Clone)]
struct BatchedBulkSelectionResolution {
    filter: NormalizedBulkListFilter,
    rows: Vec<super::bulk_read_model::BulkReadRow>,
}

impl SeoService {
    pub(super) async fn preview_bulk_selection_count_batched(
        &self,
        tenant: &TenantContext,
        selection: SeoBulkSelectionInput,
    ) -> SeoResult<SeoBulkSelectionPreviewRecord> {
        let resolution = self
            .resolve_bulk_selection_batched(tenant, selection)
            .await?;
        Ok(SeoBulkSelectionPreviewRecord {
            count: resolution.rows.len() as i32,
        })
    }

    pub(super) async fn queue_bulk_apply_batched(
        &self,
        tenant: &TenantContext,
        created_by: Option<Uuid>,
        input: SeoBulkApplyInput,
    ) -> SeoResult<SeoBulkJobRecord> {
        validate_bulk_apply(&input)?;
        let resolution = self
            .resolve_bulk_selection_batched(tenant, input.selection.clone())
            .await?;
        let target_ids = resolution
            .rows
            .iter()
            .map(|row| row.target_id)
            .collect::<Vec<_>>();
        if target_ids.len() > MAX_BULK_TARGETS {
            return Err(SeoError::validation(format!(
                "bulk apply selection exceeds the {MAX_BULK_TARGETS} target limit"
            )));
        }
        let queued_payload = QueuedBulkApplyPayload {
            input: input.clone(),
            target_ids: target_ids.clone(),
        };
        let now = Utc::now().fixed_offset();
        let model = seo_bulk_job::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant.id),
            operation_kind: Set(SeoBulkJobOperationKind::Apply.as_str().to_string()),
            status: Set(SeoBulkJobStatus::Queued.as_str().to_string()),
            target_kind: Set(resolution.filter.target_kind.as_str().to_string()),
            locale: Set(resolution.filter.locale.clone()),
            filter_payload: Set(serde_json::to_value(&resolution.filter).map_err(|err| {
                SeoError::validation(format!("failed to serialize bulk filter: {err}"))
            })?),
            input_payload: Set(serde_json::to_value(&queued_payload).map_err(|err| {
                SeoError::validation(format!(
                    "failed to serialize bounded bulk apply payload: {err}"
                ))
            })?),
            publish_after_write: Set(input.publish_after_write),
            matched_count: Set(target_ids.len() as i32),
            processed_count: Set(0),
            succeeded_count: Set(0),
            failed_count: Set(0),
            skipped_count: Set(0),
            artifact_count: Set(0),
            last_error: Set(None),
            created_by: Set(created_by),
            started_at: Set(None),
            completed_at: Set(None),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(&self.db)
        .await?;

        self.bulk_job(tenant.id, model.id)
            .await?
            .ok_or(SeoError::NotFound)
    }

    #[allow(dead_code)]
    pub(super) async fn queue_bulk_export_batched(
        &self,
        tenant: &TenantContext,
        created_by: Option<Uuid>,
        input: SeoBulkExportInput,
    ) -> SeoResult<SeoBulkJobRecord> {
        let filter =
            normalize_bulk_list_input(input.filter.clone(), tenant.default_locale.as_str())?;
        let rows = self.collect_bulk_rows_for_filter(tenant, &filter).await?;
        let now = Utc::now().fixed_offset();
        let model = seo_bulk_job::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant.id),
            operation_kind: Set(SeoBulkJobOperationKind::ExportCsv.as_str().to_string()),
            status: Set(SeoBulkJobStatus::Queued.as_str().to_string()),
            target_kind: Set(filter.target_kind.as_str().to_string()),
            locale: Set(filter.locale.clone()),
            filter_payload: Set(serde_json::to_value(&filter).map_err(|err| {
                SeoError::validation(format!("failed to serialize bulk filter: {err}"))
            })?),
            input_payload: Set(serde_json::to_value(&input).map_err(|err| {
                SeoError::validation(format!("failed to serialize bulk export input: {err}"))
            })?),
            publish_after_write: Set(false),
            matched_count: Set(rows.len() as i32),
            processed_count: Set(0),
            succeeded_count: Set(0),
            failed_count: Set(0),
            skipped_count: Set(0),
            artifact_count: Set(0),
            last_error: Set(None),
            created_by: Set(created_by),
            started_at: Set(None),
            completed_at: Set(None),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(&self.db)
        .await?;

        self.bulk_job(tenant.id, model.id)
            .await?
            .ok_or(SeoError::NotFound)
    }

    #[allow(dead_code)]
    pub(super) async fn execute_next_bulk_job_batched(
        &self,
    ) -> SeoResult<Option<SeoBulkJobRecord>> {
        let running_apply = seo_bulk_job::Entity::find()
            .filter(seo_bulk_job::Column::Status.eq(SeoBulkJobStatus::Running.as_str()))
            .filter(seo_bulk_job::Column::OperationKind.eq(SeoBulkJobOperationKind::Apply.as_str()))
            .order_by_asc(seo_bulk_job::Column::UpdatedAt)
            .one(&self.db)
            .await?;

        let running = if let Some(job) = running_apply {
            job
        } else {
            let Some(job) = seo_bulk_job::Entity::find()
                .filter(seo_bulk_job::Column::Status.eq(SeoBulkJobStatus::Queued.as_str()))
                .order_by_asc(seo_bulk_job::Column::CreatedAt)
                .one(&self.db)
                .await?
            else {
                return Ok(None);
            };

            let now = Utc::now().fixed_offset();
            let mut active: seo_bulk_job::ActiveModel = job.into();
            active.status = Set(SeoBulkJobStatus::Running.as_str().to_string());
            active.started_at = Set(Some(now));
            active.updated_at = Set(now);
            active.last_error = Set(None);
            active.update(&self.db).await?
        };

        let result = match SeoBulkJobOperationKind::parse(running.operation_kind.as_str()) {
            Some(SeoBulkJobOperationKind::Apply) => self.execute_apply_job_chunk(&running).await,
            Some(SeoBulkJobOperationKind::ExportCsv) => {
                self.execute_export_job_batched(&running).await
            }
            Some(SeoBulkJobOperationKind::ImportCsv) => self.execute_import_job(&running).await,
            None => Err(SeoError::validation(format!(
                "unknown bulk operation kind `{}`",
                running.operation_kind
            ))),
        };

        if let Err(error) = result {
            self.fail_bulk_job(&running, error.to_string()).await?;
        }

        self.bulk_job(running.tenant_id, running.id).await
    }

    async fn resolve_bulk_selection_batched(
        &self,
        tenant: &TenantContext,
        selection: SeoBulkSelectionInput,
    ) -> SeoResult<BatchedBulkSelectionResolution> {
        let filter = selection
            .filter
            .ok_or_else(|| SeoError::validation("bulk selection filter is required"))?;
        let filter = normalize_bulk_list_input(filter, tenant.default_locale.as_str())?;
        let mut rows = self.collect_bulk_rows_for_filter(tenant, &filter).await?;
        if selection.mode == SeoBulkSelectionMode::SelectedIds {
            let selected = selection.selected_ids.into_iter().collect::<HashSet<_>>();
            rows.retain(|row| selected.contains(&row.target_id));
        }

        Ok(BatchedBulkSelectionResolution { filter, rows })
    }

    async fn collect_bulk_rows_for_filter(
        &self,
        tenant: &TenantContext,
        filter: &NormalizedBulkListFilter,
    ) -> SeoResult<Vec<super::bulk_read_model::BulkReadRow>> {
        self.collect_bulk_read_rows(
            tenant,
            &super::bulk_read_model::BulkReadFilter {
                target_kind: filter.target_kind.clone(),
                locale: filter.locale.clone(),
                query: filter.query.clone(),
                source: filter.source,
            },
        )
        .await
    }

    async fn execute_apply_job_chunk(&self, job: &seo_bulk_job::Model) -> SeoResult<()> {
        let tenant = self.load_tenant_context(job.tenant_id).await?;
        let payload = self.decode_bounded_apply_payload(&tenant, job).await?;
        let processed_items = seo_bulk_job_item::Entity::find()
            .filter(seo_bulk_job_item::Column::JobId.eq(job.id))
            .order_by_asc(seo_bulk_job_item::Column::CreatedAt)
            .all(&self.db)
            .await?;
        let processed_ids = processed_items
            .iter()
            .map(|item| item.target_id)
            .collect::<HashSet<_>>();
        let chunk = payload
            .target_ids
            .iter()
            .copied()
            .filter(|target_id| !processed_ids.contains(target_id))
            .take(BULK_APPLY_CHUNK_SIZE)
            .collect::<Vec<_>>();

        if chunk.is_empty() {
            let progress = self.load_bulk_job_progress(job.id).await?;
            return self
                .finish_bulk_job(
                    job,
                    progress.processed,
                    progress.succeeded,
                    progress.failed,
                    progress.skipped,
                    progress.artifacts,
                    None,
                )
                .await;
        }

        let chunk_start = processed_ids.len() + 1;
        let chunk_end = chunk_start + chunk.len() - 1;
        let mut preview_rows = Vec::<Vec<String>>::new();
        let mut failure_rows = Vec::new();

        if payload.input.apply_mode == SeoBulkApplyMode::PreviewOnly {
            let resolution = self
                .resolve_bulk_selection_batched(&tenant, payload.input.selection.clone())
                .await?;
            let projections = resolution
                .rows
                .into_iter()
                .map(|row| (row.target_id, row.projection))
                .collect::<HashMap<_, _>>();

            for target_id in chunk {
                if let Some(projection) = projections.get(&target_id) {
                    preview_rows.push(export_bulk_projection_row(
                        resolution.filter.target_kind.clone(),
                        target_id,
                        resolution.filter.locale.as_str(),
                        projection,
                    ));
                    self.insert_bulk_job_item(job, target_id, None, None)
                        .await?;
                } else {
                    let message = "SEO target not found".to_string();
                    self.insert_bulk_job_item(job, target_id, Some(message.clone()), None)
                        .await?;
                    failure_rows.push((
                        preview_failure_row(
                            resolution.filter.target_kind.as_str(),
                            target_id,
                            resolution.filter.locale.as_str(),
                        ),
                        message,
                    ));
                }
            }
        } else {
            let filter = normalize_bulk_list_input(
                payload
                    .input
                    .selection
                    .filter
                    .clone()
                    .ok_or_else(|| SeoError::validation("bulk selection filter is required"))?,
                tenant.default_locale.as_str(),
            )?;
            for target_id in chunk {
                match self
                    .apply_bulk_patch_to_target(
                        &tenant,
                        job.id,
                        filter.target_kind.clone(),
                        filter.locale.as_str(),
                        target_id,
                        &payload.input.patch,
                        payload.input.apply_mode,
                        job.publish_after_write,
                    )
                    .await
                {
                    Ok(BulkApplyOutcome::Written { published_revision }) => {
                        self.insert_bulk_job_item(job, target_id, None, published_revision)
                            .await?;
                    }
                    Ok(BulkApplyOutcome::Skipped) => {
                        self.insert_bulk_skipped_job_item(job, target_id).await?;
                    }
                    Err(error) => {
                        let message = error.to_string();
                        self.insert_bulk_job_item(job, target_id, Some(message.clone()), None)
                            .await?;
                        failure_rows.push((
                            empty_csv_row(
                                filter.target_kind.as_str(),
                                target_id,
                                filter.locale.as_str(),
                            ),
                            message,
                        ));
                    }
                }
            }
        }

        if !preview_rows.is_empty() {
            let content = build_preview_csv(&preview_rows)?;
            self.insert_bulk_job_artifact(
                job,
                "preview_report",
                format!(
                    "seo-bulk-preview-{}-{}-{}.csv",
                    job.id, chunk_start, chunk_end
                ),
                CSV_MIME_TYPE,
                content,
            )
            .await?;
        }
        if !failure_rows.is_empty() {
            let content = build_failure_csv(&failure_rows)?;
            self.insert_bulk_job_artifact(
                job,
                "failure_report",
                format!(
                    "seo-bulk-apply-failures-{}-{}-{}.csv",
                    job.id, chunk_start, chunk_end
                ),
                CSV_MIME_TYPE,
                content,
            )
            .await?;
        }

        let progress = self.load_bulk_job_progress(job.id).await?;
        if progress.processed as usize >= payload.target_ids.len() {
            self.finish_bulk_job(
                job,
                progress.processed,
                progress.succeeded,
                progress.failed,
                progress.skipped,
                progress.artifacts,
                None,
            )
            .await
        } else {
            self.checkpoint_bulk_apply_job(job, payload.target_ids.len(), progress)
                .await
        }
    }

    async fn decode_bounded_apply_payload(
        &self,
        tenant: &TenantContext,
        job: &seo_bulk_job::Model,
    ) -> SeoResult<QueuedBulkApplyPayload> {
        if let Ok(payload) =
            serde_json::from_value::<QueuedBulkApplyPayload>(job.input_payload.clone())
        {
            return Ok(payload);
        }

        let input = serde_json::from_value::<SeoBulkApplyInput>(job.input_payload.clone())
            .map_err(|err| {
                SeoError::validation(format!("failed to decode bulk apply payload: {err}"))
            })?;
        let resolution = self
            .resolve_bulk_selection_batched(tenant, input.selection.clone())
            .await?;
        Ok(QueuedBulkApplyPayload {
            input,
            target_ids: resolution
                .rows
                .into_iter()
                .map(|row| row.target_id)
                .collect(),
        })
    }

    async fn load_bulk_job_progress(&self, job_id: Uuid) -> SeoResult<BulkJobProgress> {
        let items = seo_bulk_job_item::Entity::find()
            .filter(seo_bulk_job_item::Column::JobId.eq(job_id))
            .all(&self.db)
            .await?;
        let succeeded = items
            .iter()
            .filter(|item| item.status == "completed")
            .count() as i32;
        let failed = items.iter().filter(|item| item.status == "failed").count() as i32;
        let skipped = items.iter().filter(|item| item.status == "skipped").count() as i32;
        let artifacts = seo_bulk_job_artifact::Entity::find()
            .filter(seo_bulk_job_artifact::Column::JobId.eq(job_id))
            .all(&self.db)
            .await?
            .len() as i32;
        Ok(BulkJobProgress {
            processed: items.len() as i32,
            succeeded,
            failed,
            skipped,
            artifacts,
        })
    }

    async fn checkpoint_bulk_apply_job(
        &self,
        job: &seo_bulk_job::Model,
        matched_count: usize,
        progress: BulkJobProgress,
    ) -> SeoResult<()> {
        let now = Utc::now().fixed_offset();
        let mut active: seo_bulk_job::ActiveModel = job.clone().into();
        active.status = Set(SeoBulkJobStatus::Running.as_str().to_string());
        active.matched_count = Set(matched_count as i32);
        active.processed_count = Set(progress.processed);
        active.succeeded_count = Set(progress.succeeded);
        active.failed_count = Set(progress.failed);
        active.skipped_count = Set(progress.skipped);
        active.artifact_count = Set(progress.artifacts);
        active.last_error = Set(None);
        active.completed_at = Set(None);
        active.updated_at = Set(now);
        active.update(&self.db).await?;
        Ok(())
    }

    #[allow(dead_code)]
    async fn execute_export_job_batched(&self, job: &seo_bulk_job::Model) -> SeoResult<()> {
        let tenant = self.load_tenant_context(job.tenant_id).await?;
        let input = serde_json::from_value::<SeoBulkExportInput>(job.input_payload.clone())
            .map_err(|err| {
                SeoError::validation(format!("failed to decode bulk export payload: {err}"))
            })?;
        let filter = normalize_bulk_list_input(input.filter, tenant.default_locale.as_str())?;
        let rows = self.collect_bulk_rows_for_filter(&tenant, &filter).await?;
        let mut writer = WriterBuilder::new()
            .has_headers(false)
            .from_writer(Vec::<u8>::new());
        writer.write_record(CSV_HEADERS).map_err(|err| {
            SeoError::validation(format!("failed to write export CSV header: {err}"))
        })?;

        for row in &rows {
            writer
                .write_record(export_bulk_projection_row(
                    filter.target_kind.clone(),
                    row.target_id,
                    filter.locale.as_str(),
                    &row.projection,
                ))
                .map_err(|err| {
                    SeoError::validation(format!(
                        "failed to serialize export row for {}: {err}",
                        row.target_id
                    ))
                })?;
            self.insert_bulk_job_item(job, row.target_id, None, None)
                .await?;
        }

        let bytes = writer.into_inner().map_err(|err| {
            SeoError::validation(format!("failed to finalize export CSV writer: {err}"))
        })?;
        let content = String::from_utf8(bytes)
            .map_err(|err| SeoError::validation(format!("export CSV is not valid UTF-8: {err}")))?;
        self.insert_bulk_job_artifact(
            job,
            "export_csv",
            format!(
                "seo-bulk-export-{}-{}-{}.csv",
                filter.target_kind.as_str(),
                filter.locale,
                job.id
            ),
            CSV_MIME_TYPE,
            content,
        )
        .await?;

        self.finish_bulk_job(job, rows.len() as i32, rows.len() as i32, 0, 0, 1, None)
            .await
    }
}

fn export_bulk_projection_row(
    target_kind: SeoTargetSlug,
    target_id: Uuid,
    locale: &str,
    projection: &super::bulk_read_model::BulkReadProjection,
) -> Vec<String> {
    vec![
        target_kind.as_str().to_string(),
        target_id.to_string(),
        locale.to_string(),
        projection.title.clone().unwrap_or_default(),
        projection.description.clone().unwrap_or_default(),
        projection.keywords.clone().unwrap_or_default(),
        projection.canonical_url.clone().unwrap_or_default(),
        projection.og_title.clone().unwrap_or_default(),
        projection.og_description.clone().unwrap_or_default(),
        projection.og_image.clone().unwrap_or_default(),
        projection
            .structured_data
            .as_ref()
            .map(Value::to_string)
            .unwrap_or_default(),
        projection.noindex.to_string(),
        projection.nofollow.to_string(),
    ]
}

#[cfg(test)]
mod bounded_execution_tests {
    use super::*;

    #[test]
    fn projection_csv_row_preserves_public_column_order() {
        let target_kind = SeoTargetSlug::new("page").expect("valid target kind");
        let target_id = Uuid::new_v4();
        let row = export_bulk_projection_row(
            target_kind,
            target_id,
            "en-US",
            &super::bulk_read_model::BulkReadProjection {
                effective_locale: "en-US".to_string(),
                source: SeoBulkSource::Explicit,
                title: Some("Title".to_string()),
                description: Some("Description".to_string()),
                keywords: Some("keywords".to_string()),
                canonical_url: Some("/canonical".to_string()),
                og_title: Some("OG Title".to_string()),
                og_description: Some("OG Description".to_string()),
                og_image: Some("https://example.test/image.jpg".to_string()),
                structured_data: Some(json!({"@type": "WebPage"})),
                noindex: true,
                nofollow: false,
            },
        );

        assert_eq!(row.len(), CSV_HEADERS.len());
        assert_eq!(row[1], target_id.to_string());
        assert_eq!(row[10], "{\"@type\":\"WebPage\"}");
        assert_eq!(row[11], "true");
        assert_eq!(row[12], "false");
    }

    #[test]
    fn apply_chunk_size_stays_bounded() {
        assert_eq!(BULK_APPLY_CHUNK_SIZE, 50);
    }
}
use crate::dto::SeoModuleSettings;
use crate::entities::{self as seo_meta, meta_translation};

use super::robots::first_open_graph_image_url;
use super::templates::render_generated_record;
use super::{LoadedMeta, TargetState};

const BULK_IO_CHUNK_SIZE: usize = 50;
const BULK_IO_META_BATCH_SIZE: usize = 256;
const MAX_BULK_IMPORT_BYTES: usize = 10 * 1024 * 1024;
const MAX_BULK_IMPORT_ROWS: usize = 100_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct QueuedBulkExportPayload {
    input: SeoBulkExportInput,
    target_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct QueuedBulkImportPayload {
    input: SeoBulkImportInput,
    next_byte_offset: usize,
    next_row_number: usize,
    total_rows: usize,
}

#[derive(Debug)]
struct BulkImportChunk {
    rows: Vec<BulkImportRow>,
    next_byte_offset: usize,
    next_row_number: usize,
    exhausted: bool,
}

impl SeoService {
    pub(super) async fn queue_bulk_export_bounded_io(
        &self,
        tenant: &TenantContext,
        created_by: Option<Uuid>,
        input: SeoBulkExportInput,
    ) -> SeoResult<SeoBulkJobRecord> {
        let filter =
            normalize_bulk_list_input(input.filter.clone(), tenant.default_locale.as_str())?;
        let rows = self.collect_bulk_rows_for_filter(tenant, &filter).await?;
        let target_ids = rows
            .into_iter()
            .map(|row| row.target_id)
            .collect::<Vec<_>>();
        if target_ids.len() > MAX_BULK_TARGETS {
            return Err(SeoError::validation(format!(
                "bulk export selection exceeds the {MAX_BULK_TARGETS} target limit"
            )));
        }
        let payload = QueuedBulkExportPayload {
            input: input.clone(),
            target_ids: target_ids.clone(),
        };
        let now = Utc::now().fixed_offset();
        let model = seo_bulk_job::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant.id),
            operation_kind: Set(SeoBulkJobOperationKind::ExportCsv.as_str().to_string()),
            status: Set(SeoBulkJobStatus::Queued.as_str().to_string()),
            target_kind: Set(filter.target_kind.as_str().to_string()),
            locale: Set(filter.locale.clone()),
            filter_payload: Set(serde_json::to_value(&filter).map_err(|error| {
                SeoError::validation(format!("failed to serialize bulk filter: {error}"))
            })?),
            input_payload: Set(serde_json::to_value(&payload).map_err(|error| {
                SeoError::validation(format!(
                    "failed to serialize bounded bulk export payload: {error}"
                ))
            })?),
            publish_after_write: Set(false),
            matched_count: Set(target_ids.len() as i32),
            processed_count: Set(0),
            succeeded_count: Set(0),
            failed_count: Set(0),
            skipped_count: Set(0),
            artifact_count: Set(0),
            last_error: Set(None),
            created_by: Set(created_by),
            started_at: Set(None),
            completed_at: Set(None),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(&self.db)
        .await?;

        self.bulk_job(tenant.id, model.id)
            .await?
            .ok_or(SeoError::NotFound)
    }

    pub(super) async fn queue_bulk_import_bounded_io(
        &self,
        tenant: &TenantContext,
        created_by: Option<Uuid>,
        input: SeoBulkImportInput,
    ) -> SeoResult<SeoBulkJobRecord> {
        if input.csv_utf8.len() > MAX_BULK_IMPORT_BYTES {
            return Err(SeoError::validation(format!(
                "bulk import CSV exceeds the {MAX_BULK_IMPORT_BYTES} byte limit"
            )));
        }
        let locale = super::normalize_effective_locale(
            input.locale.as_str(),
            tenant.default_locale.as_str(),
        )?;
        let (next_byte_offset, total_rows) = scan_bulk_import_csv(
            input.target_kind.clone(),
            locale.as_str(),
            input.csv_utf8.as_str(),
        )?;
        if total_rows > MAX_BULK_IMPORT_ROWS {
            return Err(SeoError::validation(format!(
                "bulk import CSV exceeds the {MAX_BULK_IMPORT_ROWS} row limit"
            )));
        }
        let payload = QueuedBulkImportPayload {
            input: input.clone(),
            next_byte_offset,
            next_row_number: 2,
            total_rows,
        };
        let now = Utc::now().fixed_offset();
        let model = seo_bulk_job::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant.id),
            operation_kind: Set(SeoBulkJobOperationKind::ImportCsv.as_str().to_string()),
            status: Set(SeoBulkJobStatus::Queued.as_str().to_string()),
            target_kind: Set(input.target_kind.as_str().to_string()),
            locale: Set(locale.clone()),
            filter_payload: Set(json!({
                "target_kind": input.target_kind.as_str(),
                "locale": locale,
            })),
            input_payload: Set(serde_json::to_value(&payload).map_err(|error| {
                SeoError::validation(format!(
                    "failed to serialize bounded bulk import payload: {error}"
                ))
            })?),
            publish_after_write: Set(input.publish_after_write),
            matched_count: Set(total_rows as i32),
            processed_count: Set(0),
            succeeded_count: Set(0),
            failed_count: Set(0),
            skipped_count: Set(0),
            artifact_count: Set(0),
            last_error: Set(None),
            created_by: Set(created_by),
            started_at: Set(None),
            completed_at: Set(None),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(&self.db)
        .await?;

        self.bulk_job(tenant.id, model.id)
            .await?
            .ok_or(SeoError::NotFound)
    }

    #[allow(dead_code)]
    pub(super) async fn execute_next_bulk_job_fully_bounded(
        &self,
    ) -> SeoResult<Option<SeoBulkJobRecord>> {
        let running = seo_bulk_job::Entity::find()
            .filter(seo_bulk_job::Column::Status.eq(SeoBulkJobStatus::Running.as_str()))
            .filter(seo_bulk_job::Column::OperationKind.is_in([
                SeoBulkJobOperationKind::Apply.as_str(),
                SeoBulkJobOperationKind::ExportCsv.as_str(),
                SeoBulkJobOperationKind::ImportCsv.as_str(),
            ]))
            .order_by_asc(seo_bulk_job::Column::UpdatedAt)
            .one(&self.db)
            .await?;

        let running = if let Some(job) = running {
            job
        } else {
            let Some(job) = seo_bulk_job::Entity::find()
                .filter(seo_bulk_job::Column::Status.eq(SeoBulkJobStatus::Queued.as_str()))
                .order_by_asc(seo_bulk_job::Column::CreatedAt)
                .one(&self.db)
                .await?
            else {
                return Ok(None);
            };

            let now = Utc::now().fixed_offset();
            let mut active: seo_bulk_job::ActiveModel = job.into();
            active.status = Set(SeoBulkJobStatus::Running.as_str().to_string());
            active.started_at = Set(Some(now));
            active.updated_at = Set(now);
            active.last_error = Set(None);
            active.update(&self.db).await?
        };

        let result = if !self.runtime_module_enabled(running.tenant_id).await?
            || !self
                .load_settings(running.tenant_id)
                .await?
                .submodule_bulk_editor_enabled
        {
            Err(SeoError::configuration(
                "SEO bulk editor was disabled after the job was queued",
            ))
        } else {
            match SeoBulkJobOperationKind::parse(running.operation_kind.as_str()) {
                Some(SeoBulkJobOperationKind::Apply) => {
                    self.execute_apply_job_chunk(&running).await
                }
                Some(SeoBulkJobOperationKind::ExportCsv) => {
                    self.execute_export_job_chunk(&running).await
                }
                Some(SeoBulkJobOperationKind::ImportCsv) => {
                    self.execute_import_job_chunk(&running).await
                }
                None => Err(SeoError::validation(format!(
                    "unknown bulk operation kind `{}`",
                    running.operation_kind
                ))),
            }
        };

        if let Err(error) = result {
            self.fail_bulk_job(&running, error.to_string()).await?;
        }

        self.bulk_job(running.tenant_id, running.id).await
    }

    async fn execute_export_job_chunk(&self, job: &seo_bulk_job::Model) -> SeoResult<()> {
        let tenant = self.load_tenant_context(job.tenant_id).await?;
        let payload = self.decode_bounded_export_payload(&tenant, job).await?;
        let processed_items = seo_bulk_job_item::Entity::find()
            .filter(seo_bulk_job_item::Column::JobId.eq(job.id))
            .all(&self.db)
            .await?;
        let processed_ids = processed_items
            .iter()
            .map(|item| item.target_id)
            .collect::<HashSet<_>>();
        let chunk = payload
            .target_ids
            .iter()
            .copied()
            .filter(|target_id| !processed_ids.contains(target_id))
            .take(BULK_IO_CHUNK_SIZE)
            .collect::<Vec<_>>();

        if chunk.is_empty() {
            let progress = self.load_bulk_job_progress(job.id).await?;
            return self
                .finish_bulk_job(
                    job,
                    progress.processed,
                    progress.succeeded,
                    progress.failed,
                    progress.skipped,
                    progress.artifacts,
                    None,
                )
                .await;
        }

        let filter = normalize_bulk_list_input(
            payload.input.filter.clone(),
            tenant.default_locale.as_str(),
        )?;
        let projections = self
            .load_bulk_export_projection_chunk(&tenant, &filter, chunk.as_slice())
            .await?;
        let chunk_start = processed_ids.len() + 1;
        let chunk_end = chunk_start + chunk.len() - 1;
        let mut writer = WriterBuilder::new()
            .has_headers(false)
            .from_writer(Vec::<u8>::new());
        writer.write_record(CSV_HEADERS).map_err(|error| {
            SeoError::validation(format!("failed to write export CSV header: {error}"))
        })?;
        let mut failure_rows = Vec::new();

        for target_id in chunk {
            if let Some(projection) = projections.get(&target_id) {
                writer
                    .write_record(export_bulk_projection_row(
                        filter.target_kind.clone(),
                        target_id,
                        filter.locale.as_str(),
                        projection,
                    ))
                    .map_err(|error| {
                        SeoError::validation(format!(
                            "failed to serialize export row for {target_id}: {error}"
                        ))
                    })?;
                self.insert_bulk_job_item(job, target_id, None, None)
                    .await?;
            } else {
                let message = "SEO target not found".to_string();
                self.insert_bulk_job_item(job, target_id, Some(message.clone()), None)
                    .await?;
                failure_rows.push((
                    empty_csv_row(
                        filter.target_kind.as_str(),
                        target_id,
                        filter.locale.as_str(),
                    ),
                    message,
                ));
            }
        }

        let bytes = writer.into_inner().map_err(|error| {
            SeoError::validation(format!("failed to finalize export CSV writer: {error}"))
        })?;
        let content = String::from_utf8(bytes).map_err(|error| {
            SeoError::validation(format!("export CSV is not valid UTF-8: {error}"))
        })?;
        self.insert_bulk_job_artifact(
            job,
            "export_csv",
            format!(
                "seo-bulk-export-{}-{}-{}-{}-{}.csv",
                filter.target_kind.as_str(),
                filter.locale,
                job.id,
                chunk_start,
                chunk_end,
            ),
            CSV_MIME_TYPE,
            content,
        )
        .await?;

        if !failure_rows.is_empty() {
            let content = build_failure_csv(&failure_rows)?;
            self.insert_bulk_job_artifact(
                job,
                "failure_report",
                format!(
                    "seo-bulk-export-failures-{}-{}-{}.csv",
                    job.id, chunk_start, chunk_end
                ),
                CSV_MIME_TYPE,
                content,
            )
            .await?;
        }

        let progress = self.load_bulk_job_progress(job.id).await?;
        if progress.processed as usize >= payload.target_ids.len() {
            self.finish_bulk_job(
                job,
                progress.processed,
                progress.succeeded,
                progress.failed,
                progress.skipped,
                progress.artifacts,
                None,
            )
            .await
        } else {
            self.checkpoint_bulk_io_job(
                job,
                payload.target_ids.len(),
                progress,
                serde_json::to_value(&payload).map_err(|error| {
                    SeoError::validation(format!(
                        "failed to checkpoint bounded bulk export payload: {error}"
                    ))
                })?,
            )
            .await
        }
    }

    async fn execute_import_job_chunk(&self, job: &seo_bulk_job::Model) -> SeoResult<()> {
        let tenant = self.load_tenant_context(job.tenant_id).await?;
        let mut payload = self.decode_bounded_import_payload(job).await?;
        let chunk = read_bulk_import_chunk(&payload)?;

        if chunk.rows.is_empty() {
            let progress = self.load_bulk_job_progress(job.id).await?;
            return self
                .finish_bulk_job(
                    job,
                    progress.processed,
                    progress.succeeded,
                    progress.failed,
                    progress.skipped,
                    progress.artifacts,
                    None,
                )
                .await;
        }

        let locale = super::normalize_effective_locale(
            payload.input.locale.as_str(),
            tenant.default_locale.as_str(),
        )?;
        let chunk_start = payload.next_row_number;
        let chunk_end = chunk.next_row_number.saturating_sub(1);
        let mut failure_rows = Vec::new();

        for row in chunk.rows {
            match self
                .import_bulk_row(
                    &tenant,
                    job.id,
                    payload.input.target_kind.clone(),
                    locale.as_str(),
                    &row,
                    job.publish_after_write,
                )
                .await
            {
                Ok(revision) => {
                    self.insert_bulk_job_item(job, row.target_id, None, revision)
                        .await?;
                }
                Err(error) => {
                    let message = error.to_string();
                    self.insert_bulk_job_item(job, row.target_id, Some(message.clone()), None)
                        .await?;
                    failure_rows.push((
                        export_csv_row_values(
                            payload.input.target_kind.clone(),
                            locale.as_str(),
                            &row,
                        ),
                        format!("row {}: {}", row.row_number, message),
                    ));
                }
            }
        }

        if !failure_rows.is_empty() {
            let content = build_failure_csv(&failure_rows)?;
            self.insert_bulk_job_artifact(
                job,
                "failure_report",
                format!(
                    "seo-bulk-import-failures-{}-{}-{}.csv",
                    job.id, chunk_start, chunk_end
                ),
                CSV_MIME_TYPE,
                content,
            )
            .await?;
        }

        payload.next_byte_offset = chunk.next_byte_offset;
        payload.next_row_number = chunk.next_row_number;
        let progress = self.load_bulk_job_progress(job.id).await?;
        if chunk.exhausted || progress.processed as usize >= payload.total_rows {
            self.finish_bulk_job(
                job,
                progress.processed,
                progress.succeeded,
                progress.failed,
                progress.skipped,
                progress.artifacts,
                None,
            )
            .await
        } else {
            self.checkpoint_bulk_io_job(
                job,
                payload.total_rows,
                progress,
                serde_json::to_value(&payload).map_err(|error| {
                    SeoError::validation(format!(
                        "failed to checkpoint bounded bulk import payload: {error}"
                    ))
                })?,
            )
            .await
        }
    }

    async fn decode_bounded_export_payload(
        &self,
        tenant: &TenantContext,
        job: &seo_bulk_job::Model,
    ) -> SeoResult<QueuedBulkExportPayload> {
        if let Ok(payload) =
            serde_json::from_value::<QueuedBulkExportPayload>(job.input_payload.clone())
        {
            return Ok(payload);
        }

        let input = serde_json::from_value::<SeoBulkExportInput>(job.input_payload.clone())
            .map_err(|error| {
                SeoError::validation(format!("failed to decode bulk export payload: {error}"))
            })?;
        let filter =
            normalize_bulk_list_input(input.filter.clone(), tenant.default_locale.as_str())?;
        let rows = self.collect_bulk_rows_for_filter(tenant, &filter).await?;
        Ok(QueuedBulkExportPayload {
            input,
            target_ids: rows.into_iter().map(|row| row.target_id).collect(),
        })
    }

    async fn decode_bounded_import_payload(
        &self,
        job: &seo_bulk_job::Model,
    ) -> SeoResult<QueuedBulkImportPayload> {
        if let Ok(payload) =
            serde_json::from_value::<QueuedBulkImportPayload>(job.input_payload.clone())
        {
            return Ok(payload);
        }

        let input = serde_json::from_value::<SeoBulkImportInput>(job.input_payload.clone())
            .map_err(|error| {
                SeoError::validation(format!("failed to decode bulk import payload: {error}"))
            })?;
        let (next_byte_offset, total_rows) = scan_bulk_import_csv(
            input.target_kind.clone(),
            job.locale.as_str(),
            input.csv_utf8.as_str(),
        )?;
        let progress = self.load_bulk_job_progress(job.id).await?;
        advance_legacy_import_cursor(
            QueuedBulkImportPayload {
                input,
                next_byte_offset,
                next_row_number: 2,
                total_rows,
            },
            progress.processed.max(0) as usize,
        )
    }

    async fn checkpoint_bulk_io_job(
        &self,
        job: &seo_bulk_job::Model,
        matched_count: usize,
        progress: BulkJobProgress,
        input_payload: Value,
    ) -> SeoResult<()> {
        let now = Utc::now().fixed_offset();
        let mut active: seo_bulk_job::ActiveModel = job.clone().into();
        active.status = Set(SeoBulkJobStatus::Running.as_str().to_string());
        active.input_payload = Set(input_payload);
        active.matched_count = Set(matched_count as i32);
        active.processed_count = Set(progress.processed);
        active.succeeded_count = Set(progress.succeeded);
        active.failed_count = Set(progress.failed);
        active.skipped_count = Set(progress.skipped);
        active.artifact_count = Set(progress.artifacts);
        active.last_error = Set(None);
        active.completed_at = Set(None);
        active.updated_at = Set(now);
        active.update(&self.db).await?;
        Ok(())
    }

    async fn load_bulk_export_projection_chunk(
        &self,
        tenant: &TenantContext,
        filter: &NormalizedBulkListFilter,
        target_ids: &[Uuid],
    ) -> SeoResult<HashMap<Uuid, super::bulk_read_model::BulkReadProjection>> {
        if target_ids.is_empty() || !self.is_enabled(tenant.id).await? {
            return Ok(HashMap::new());
        }
        let Some(provider) = self.registry.get(&filter.target_kind) else {
            return Ok(HashMap::new());
        };
        let selected = target_ids.iter().copied().collect::<HashSet<_>>();
        let summaries = provider
            .list_bulk_summaries(
                &self.target_runtime(),
                SeoTargetBulkListRequest {
                    tenant_id: tenant.id,
                    default_locale: tenant.default_locale.as_str(),
                    locale: filter.locale.as_str(),
                },
            )
            .await
            .map_err(|error| {
                SeoError::validation(format!(
                    "SEO target provider `{}` failed to collect bounded export summaries: {error}",
                    filter.target_kind.as_str()
                ))
            })?;
        let available = summaries
            .into_iter()
            .filter(|summary| selected.contains(&summary.target_id))
            .map(|summary| summary.target_id)
            .collect::<HashSet<_>>();
        let mut explicit_by_target = self
            .load_bulk_io_explicit_meta_batches(tenant.id, filter.target_kind.as_str(), target_ids)
            .await?;
        let settings = self.load_settings(tenant.id).await?;
        let mut projections = HashMap::new();

        for target_id in target_ids.iter().copied() {
            if !available.contains(&target_id) {
                continue;
            }
            let state = self
                .load_target_state(
                    tenant,
                    filter.target_kind.clone(),
                    target_id,
                    filter.locale.as_str(),
                )
                .await?;
            let explicit = explicit_by_target.remove(&target_id);
            if let Some(projection) = resolve_bulk_io_projection(
                tenant,
                filter.target_kind.clone(),
                target_id,
                filter.locale.as_str(),
                explicit,
                state,
                &settings,
            ) {
                projections.insert(target_id, projection);
            }
        }

        Ok(projections)
    }

    async fn load_bulk_io_explicit_meta_batches(
        &self,
        tenant_id: Uuid,
        target_kind: &str,
        target_ids: &[Uuid],
    ) -> SeoResult<HashMap<Uuid, LoadedMeta>> {
        let mut loaded = HashMap::new();
        for target_ids in target_ids.chunks(BULK_IO_META_BATCH_SIZE) {
            let metas = seo_meta::Entity::find()
                .filter(seo_meta::Column::TenantId.eq(tenant_id))
                .filter(seo_meta::Column::TargetType.eq(target_kind))
                .filter(seo_meta::Column::TargetId.is_in(target_ids.iter().copied()))
                .all(&self.db)
                .await?;
            if metas.is_empty() {
                continue;
            }

            let meta_ids = metas.iter().map(|meta| meta.id).collect::<Vec<_>>();
            let translations = meta_translation::Entity::find()
                .filter(meta_translation::Column::MetaId.is_in(meta_ids))
                .order_by_asc(meta_translation::Column::MetaId)
                .order_by_asc(meta_translation::Column::Locale)
                .all(&self.db)
                .await?;
            let mut translations_by_meta = HashMap::<Uuid, Vec<meta_translation::Model>>::new();
            for translation in translations {
                translations_by_meta
                    .entry(translation.meta_id)
                    .or_default()
                    .push(translation);
            }

            for meta in metas {
                loaded.insert(
                    meta.target_id,
                    LoadedMeta {
                        translations: translations_by_meta.remove(&meta.id).unwrap_or_default(),
                        meta,
                    },
                );
            }
        }
        Ok(loaded)
    }
}

fn scan_bulk_import_csv(
    expected_kind: SeoTargetSlug,
    expected_locale: &str,
    csv_utf8: &str,
) -> SeoResult<(usize, usize)> {
    let mut reader = ReaderBuilder::new()
        .has_headers(true)
        .flexible(false)
        .from_reader(csv_utf8.as_bytes());
    let headers = reader
        .headers()
        .map_err(|error| SeoError::validation(format!("failed to read CSV headers: {error}")))?
        .clone();
    validate_csv_headers(&headers)?;
    let next_byte_offset = reader.position().byte() as usize;
    let mut total_rows = 0_usize;
    for (index, result) in reader.records().enumerate() {
        let record = result.map_err(|error| {
            SeoError::validation(format!("failed to read CSV row {}: {error}", index + 2))
        })?;
        parse_bulk_csv_record(&record, &expected_kind, expected_locale, index + 2)?;
        total_rows += 1;
    }
    Ok((next_byte_offset, total_rows))
}

fn read_bulk_import_chunk(payload: &QueuedBulkImportPayload) -> SeoResult<BulkImportChunk> {
    let bytes = payload.input.csv_utf8.as_bytes();
    if payload.next_byte_offset >= bytes.len() {
        return Ok(BulkImportChunk {
            rows: Vec::new(),
            next_byte_offset: bytes.len(),
            next_row_number: payload.next_row_number,
            exhausted: true,
        });
    }

    let mut reader = ReaderBuilder::new()
        .has_headers(false)
        .flexible(false)
        .from_reader(&bytes[payload.next_byte_offset..]);
    let mut rows = Vec::new();
    let mut next_row_number = payload.next_row_number;
    {
        let mut records = reader.records();
        while rows.len() < BULK_IO_CHUNK_SIZE {
            let Some(result) = records.next() else {
                break;
            };
            let record = result.map_err(|error| {
                SeoError::validation(format!("failed to read CSV row {next_row_number}: {error}"))
            })?;
            rows.push(parse_bulk_csv_record(
                &record,
                &payload.input.target_kind,
                payload.input.locale.as_str(),
                next_row_number,
            )?);
            next_row_number += 1;
        }
    }
    let next_byte_offset = payload.next_byte_offset + reader.position().byte() as usize;
    Ok(BulkImportChunk {
        rows,
        next_byte_offset,
        next_row_number,
        exhausted: next_byte_offset >= bytes.len(),
    })
}

fn advance_legacy_import_cursor(
    mut payload: QueuedBulkImportPayload,
    processed_rows: usize,
) -> SeoResult<QueuedBulkImportPayload> {
    let bytes = payload.input.csv_utf8.as_bytes();
    if processed_rows == 0 || payload.next_byte_offset >= bytes.len() {
        return Ok(payload);
    }

    let mut reader = ReaderBuilder::new()
        .has_headers(false)
        .flexible(false)
        .from_reader(&bytes[payload.next_byte_offset..]);
    let mut skipped = 0_usize;
    {
        let mut records = reader.records();
        while skipped < processed_rows {
            let Some(result) = records.next() else {
                break;
            };
            result.map_err(|error| {
                SeoError::validation(format!(
                    "failed to resume legacy bulk import at row {}: {error}",
                    payload.next_row_number + skipped
                ))
            })?;
            skipped += 1;
        }
    }
    payload.next_byte_offset += reader.position().byte() as usize;
    payload.next_row_number += skipped;
    Ok(payload)
}

#[allow(clippy::too_many_arguments)]
fn resolve_bulk_io_projection(
    tenant: &TenantContext,
    target_kind: SeoTargetSlug,
    target_id: Uuid,
    requested_locale: &str,
    explicit: Option<LoadedMeta>,
    state: Option<TargetState>,
    settings: &SeoModuleSettings,
) -> Option<super::bulk_read_model::BulkReadProjection> {
    match (explicit, state) {
        (Some(explicit), Some(state)) => {
            let resolved = resolve_by_locale_with_fallback(
                explicit.translations.as_slice(),
                state.effective_locale.as_str(),
                Some(tenant.default_locale.as_str()),
                |item| item.locale.as_str(),
            );
            let translation = resolved.item.cloned();
            Some(super::bulk_read_model::BulkReadProjection {
                effective_locale: resolved.effective_locale,
                source: SeoBulkSource::Explicit,
                title: translation
                    .as_ref()
                    .and_then(|item| trimmed_option(item.title.clone()))
                    .or(Some(state.title)),
                description: translation
                    .as_ref()
                    .and_then(|item| trimmed_option(item.description.clone()))
                    .or(state.description),
                keywords: translation
                    .as_ref()
                    .and_then(|item| trimmed_option(item.keywords.clone())),
                canonical_url: explicit.meta.canonical_url,
                og_title: translation
                    .as_ref()
                    .and_then(|item| trimmed_option(item.og_title.clone())),
                og_description: translation
                    .as_ref()
                    .and_then(|item| trimmed_option(item.og_description.clone())),
                og_image: translation
                    .as_ref()
                    .and_then(|item| trimmed_option(item.og_image.clone())),
                structured_data: explicit.meta.structured_data,
                noindex: explicit.meta.no_index,
                nofollow: explicit.meta.no_follow,
            })
        }
        (Some(explicit), None) => {
            let resolved = resolve_by_locale_with_fallback(
                explicit.translations.as_slice(),
                requested_locale,
                Some(tenant.default_locale.as_str()),
                |item| item.locale.as_str(),
            );
            let translation = resolved.item.cloned();
            Some(super::bulk_read_model::BulkReadProjection {
                effective_locale: resolved.effective_locale,
                source: SeoBulkSource::Explicit,
                title: translation.as_ref().and_then(|item| item.title.clone()),
                description: translation
                    .as_ref()
                    .and_then(|item| item.description.clone()),
                keywords: translation.as_ref().and_then(|item| item.keywords.clone()),
                canonical_url: explicit.meta.canonical_url,
                og_title: translation.as_ref().and_then(|item| item.og_title.clone()),
                og_description: translation
                    .as_ref()
                    .and_then(|item| item.og_description.clone()),
                og_image: translation.as_ref().and_then(|item| item.og_image.clone()),
                structured_data: explicit.meta.structured_data,
                noindex: explicit.meta.no_index,
                nofollow: explicit.meta.no_follow,
            })
        }
        (None, Some(state)) => {
            debug_assert_eq!(state.target_kind, target_kind);
            debug_assert_eq!(state.target_id, target_id);
            let generated = render_generated_record(
                &state,
                &settings.template_defaults,
                settings.template_overrides.get(state.target_kind.as_str()),
            );
            let generated_source = generated.title.is_some()
                || generated.description.is_some()
                || generated.canonical_url.is_some()
                || generated.keywords.is_some()
                || generated.og_title.is_some()
                || generated.og_description.is_some()
                || generated.robots.is_some()
                || generated.twitter_title.is_some()
                || generated.twitter_description.is_some();
            Some(super::bulk_read_model::BulkReadProjection {
                effective_locale: state.effective_locale,
                source: if generated_source {
                    SeoBulkSource::Generated
                } else {
                    SeoBulkSource::Fallback
                },
                title: generated.title.or(Some(state.title)),
                description: generated.description.or(state.description),
                keywords: generated.keywords,
                canonical_url: generated.canonical_url,
                og_title: generated.og_title.or(state.open_graph.title.clone()),
                og_description: generated
                    .og_description
                    .or(state.open_graph.description.clone()),
                og_image: first_open_graph_image_url(&state.open_graph),
                structured_data: Some(state.structured_data),
                noindex: false,
                nofollow: false,
            })
        }
        (None, None) => None,
    }
}

#[cfg(test)]
mod bounded_io_execution_tests {
    use super::*;

    #[test]
    fn io_chunk_size_stays_bounded() {
        assert_eq!(BULK_IO_CHUNK_SIZE, 50);
    }

    #[test]
    fn import_scan_keeps_a_byte_cursor_after_headers() {
        let csv = "target_kind,target_id,locale,title,description,keywords,canonical_url,og_title,og_description,og_image,structured_data,noindex,nofollow\npage,11111111-1111-1111-1111-111111111111,en-US,Title,,,,,,,,false,false\n";
        let (offset, rows) = scan_bulk_import_csv(
            SeoTargetSlug::new("page").expect("valid target kind"),
            "en-US",
            csv,
        )
        .expect("scan import");
        assert!(offset > 0);
        assert_eq!(rows, 1);
    }
}
impl SeoService {
    #[allow(dead_code)]
    pub(super) async fn execute_next_bulk_job_with_bounded_io(
        &self,
    ) -> SeoResult<Option<SeoBulkJobRecord>> {
        // The server already owns one configured SEO poller. Keep that stable lifecycle boundary,
        // but let every poll advance at most one bounded job from each durable SEO queue.
        let bulk_result = self.execute_next_bulk_job_only_with_bounded_io().await;
        let sitemap_result = self.execute_next_sitemap_job_background().await;
        let index_result = self.execute_next_index_repair_replay_job_background().await;

        if let Ok(Some(job)) = &sitemap_result {
            tracing::info!(
                job_id = %job.id,
                status = %job.status,
                "Executed queued SEO sitemap job phase"
            );
        }
        if let Ok(Some(result)) = &index_result {
            tracing::info!(
                target_type = ?result.target_type,
                replay_mode = %result.replay_mode.as_str(),
                repaired_count = result.repaired_count,
                replayed_count = result.replayed_count,
                "Executed queued SEO index repair/replay job"
            );
        }

        let bulk_job = bulk_result?;
        sitemap_result?;
        index_result?;
        Ok(bulk_job)
    }

    pub(super) async fn execute_next_bulk_job_only_with_bounded_io(
        &self,
    ) -> SeoResult<Option<SeoBulkJobRecord>> {
        const JOB_LEASE_SECS: i64 = 30 * 60;
        let now = Utc::now().fixed_offset();
        let stale_before = now - chrono::Duration::seconds(JOB_LEASE_SECS);

        // A fresh running job must not block queued work belonging to another tenant. Only
        // stale leases are resumed here; the conditional update remains the ownership claim.
        let stale_running = seo_bulk_job::Entity::find()
            .filter(seo_bulk_job::Column::Status.eq(SeoBulkJobStatus::Running.as_str()))
            .filter(seo_bulk_job::Column::OperationKind.is_in([
                SeoBulkJobOperationKind::Apply.as_str(),
                SeoBulkJobOperationKind::ExportCsv.as_str(),
                SeoBulkJobOperationKind::ImportCsv.as_str(),
            ]))
            .filter(seo_bulk_job::Column::UpdatedAt.lte(stale_before))
            .order_by_asc(seo_bulk_job::Column::UpdatedAt)
            .one(&self.db)
            .await?;

        let running = if let Some(job) = stale_running {
            let claimed = seo_bulk_job::Entity::update_many()
                .col_expr(
                    seo_bulk_job::Column::UpdatedAt,
                    sea_orm::sea_query::Expr::value(now),
                )
                .filter(seo_bulk_job::Column::Id.eq(job.id))
                .filter(seo_bulk_job::Column::Status.eq(SeoBulkJobStatus::Running.as_str()))
                .filter(seo_bulk_job::Column::UpdatedAt.lte(stale_before))
                .exec(&self.db)
                .await?;

            if claimed.rows_affected != 1 {
                return Ok(None);
            }

            seo_bulk_job::Entity::find_by_id(job.id)
                .one(&self.db)
                .await?
                .ok_or(SeoError::NotFound)?
        } else {
            let Some(job) = seo_bulk_job::Entity::find()
                .filter(seo_bulk_job::Column::Status.eq(SeoBulkJobStatus::Queued.as_str()))
                .order_by_asc(seo_bulk_job::Column::CreatedAt)
                .one(&self.db)
                .await?
            else {
                return Ok(None);
            };

            let claimed = seo_bulk_job::Entity::update_many()
                .col_expr(
                    seo_bulk_job::Column::Status,
                    sea_orm::sea_query::Expr::value(SeoBulkJobStatus::Running.as_str()),
                )
                .col_expr(
                    seo_bulk_job::Column::StartedAt,
                    sea_orm::sea_query::Expr::value(Some(now)),
                )
                .col_expr(
                    seo_bulk_job::Column::UpdatedAt,
                    sea_orm::sea_query::Expr::value(now),
                )
                .filter(seo_bulk_job::Column::Id.eq(job.id))
                .filter(seo_bulk_job::Column::Status.eq(SeoBulkJobStatus::Queued.as_str()))
                .exec(&self.db)
                .await?;

            if claimed.rows_affected != 1 {
                return Ok(None);
            }

            seo_bulk_job::Entity::find_by_id(job.id)
                .one(&self.db)
                .await?
                .ok_or(SeoError::NotFound)?
        };

        let result = if !self.runtime_module_enabled(running.tenant_id).await?
            || !self
                .load_settings(running.tenant_id)
                .await?
                .submodule_bulk_editor_enabled
        {
            Err(SeoError::configuration(
                "SEO bulk editor was disabled after the job was queued",
            ))
        } else {
            match SeoBulkJobOperationKind::parse(running.operation_kind.as_str()) {
                Some(SeoBulkJobOperationKind::Apply) => {
                    self.execute_apply_job_chunk(&running).await
                }
                Some(SeoBulkJobOperationKind::ExportCsv) => {
                    self.execute_export_job_chunk_compat(&running).await
                }
                Some(SeoBulkJobOperationKind::ImportCsv) => {
                    match self.normalize_bulk_import_job_payload(&running).await {
                        Ok(normalized) => self.execute_import_job_chunk(&normalized).await,
                        Err(error) => Err(error),
                    }
                }
                None => Err(SeoError::validation(format!(
                    "unknown bulk operation kind `{}`",
                    running.operation_kind
                ))),
            }
        };

        if let Err(error) = result {
            self.fail_bulk_job(&running, error.to_string()).await?;
        }
        self.prune_bulk_history(running.tenant_id).await?;

        self.bulk_job(running.tenant_id, running.id).await
    }

    async fn prune_bulk_history(&self, tenant_id: Uuid) -> SeoResult<()> {
        let cutoff =
            Utc::now().fixed_offset() - chrono::Duration::days(super::SEO_HISTORY_RETENTION_DAYS);
        let old_jobs = seo_bulk_job::Entity::find()
            .filter(seo_bulk_job::Column::TenantId.eq(tenant_id))
            .filter(seo_bulk_job::Column::Status.is_in([
                SeoBulkJobStatus::Completed.as_str(),
                SeoBulkJobStatus::Partial.as_str(),
                SeoBulkJobStatus::Failed.as_str(),
            ]))
            .filter(seo_bulk_job::Column::CreatedAt.lt(cutoff))
            .order_by_asc(seo_bulk_job::Column::CreatedAt)
            .limit(super::SEO_HISTORY_PRUNE_BATCH_SIZE as u64)
            .all(&self.db)
            .await?;
        if old_jobs.is_empty() {
            return Ok(());
        }
        let ids = old_jobs.into_iter().map(|job| job.id).collect::<Vec<_>>();
        seo_bulk_job::Entity::delete_many()
            .filter(seo_bulk_job::Column::Id.is_in(ids))
            .exec(&self.db)
            .await?;
        Ok(())
    }

    async fn execute_export_job_chunk_compat(&self, job: &seo_bulk_job::Model) -> SeoResult<()> {
        let tenant = self.load_tenant_context(job.tenant_id).await?;
        let payload = self.decode_bounded_export_payload(&tenant, job).await?;
        if !payload.target_ids.is_empty() {
            return self.execute_export_job_chunk(job).await;
        }

        let progress = self.load_bulk_job_progress(job.id).await?;
        if progress.artifacts == 0 {
            let filter =
                normalize_bulk_list_input(payload.input.filter, tenant.default_locale.as_str())?;
            let mut writer = WriterBuilder::new()
                .has_headers(false)
                .from_writer(Vec::<u8>::new());
            writer.write_record(CSV_HEADERS).map_err(|error| {
                SeoError::validation(format!("failed to write empty export CSV header: {error}"))
            })?;
            let bytes = writer.into_inner().map_err(|error| {
                SeoError::validation(format!(
                    "failed to finalize empty export CSV writer: {error}"
                ))
            })?;
            let content = String::from_utf8(bytes).map_err(|error| {
                SeoError::validation(format!("empty export CSV is not valid UTF-8: {error}"))
            })?;
            self.insert_bulk_job_artifact(
                job,
                "export_csv",
                format!(
                    "seo-bulk-export-{}-{}-{}.csv",
                    filter.target_kind.as_str(),
                    filter.locale,
                    job.id,
                ),
                CSV_MIME_TYPE,
                content,
            )
            .await?;
        }

        let progress = self.load_bulk_job_progress(job.id).await?;
        self.finish_bulk_job(
            job,
            progress.processed,
            progress.succeeded,
            progress.failed,
            progress.skipped,
            progress.artifacts,
            None,
        )
        .await
    }

    async fn normalize_bulk_import_job_payload(
        &self,
        job: &seo_bulk_job::Model,
    ) -> SeoResult<seo_bulk_job::Model> {
        let mut payload = self.decode_bounded_import_payload(job).await?;
        if payload.input.locale == job.locale
            && serde_json::from_value::<QueuedBulkImportPayload>(job.input_payload.clone()).is_ok()
        {
            return Ok(job.clone());
        }

        payload.input.locale = job.locale.clone();
        let now = Utc::now().fixed_offset();
        let mut active: seo_bulk_job::ActiveModel = job.clone().into();
        active.input_payload = Set(serde_json::to_value(&payload).map_err(|error| {
            SeoError::validation(format!(
                "failed to normalize bounded bulk import payload: {error}"
            ))
        })?);
        active.updated_at = Set(now);
        active.update(&self.db).await.map_err(Into::into)
    }
}
use super::sitemaps::{
    SitemapUrlRecord, index_generation, sitemaps_enabled, submission_adapters,
    submission_aggregation,
};

const SITEMAP_JOB_QUEUED: &str = "queued";
const SITEMAP_JOB_RUNNING: &str = "running";
const SITEMAP_JOB_SUBMITTING: &str = "submitting";
const SITEMAP_JOB_COMPLETED: &str = "completed";
const SITEMAP_JOB_FAILED: &str = "failed";
const SITEMAP_BACKGROUND_SUBMIT_TIMEOUT_SECS: u64 = 5;
const SITEMAP_BACKGROUND_DELIVERY_STATUS_SENT: &str = "sent";

struct BackgroundSitemapEventPublication {
    tenant_id: Uuid,
    job_id: Uuid,
    event_type: String,
    idempotency_key: String,
    event: rustok_core::DomainEvent,
    occurred_at: chrono::DateTime<chrono::FixedOffset>,
}

impl SeoService {
    pub(super) async fn queue_sitemap_generation_background(
        &self,
        tenant: &TenantContext,
    ) -> SeoResult<crate::dto::SeoSitemapStatusRecord> {
        let settings = self.load_settings(tenant.id).await?;
        if !sitemaps_enabled(&settings) {
            return Ok(background_disabled_sitemap_status());
        }

        // Validate the public origin while the caller can still receive a configuration error,
        // but do not collect targets, render XML, or perform external submissions here.
        self.robots_preview(tenant).await?;

        let active = crate::entities::seo_sitemap_job::Entity::find()
            .filter(crate::entities::seo_sitemap_job::Column::TenantId.eq(tenant.id))
            .filter(crate::entities::seo_sitemap_job::Column::Status.is_in([
                SITEMAP_JOB_QUEUED,
                SITEMAP_JOB_RUNNING,
                SITEMAP_JOB_SUBMITTING,
            ]))
            .order_by_asc(crate::entities::seo_sitemap_job::Column::CreatedAt)
            .one(&self.db)
            .await?;
        if active.is_some() {
            return self.sitemap_status(tenant).await;
        }

        let now = chrono::Utc::now().fixed_offset();
        let inserted = crate::entities::seo_sitemap_job::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant.id),
            status: Set(SITEMAP_JOB_QUEUED.to_string()),
            file_count: Set(0),
            started_at: Set(None),
            completed_at: Set(None),
            last_error: Set(None),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(&self.db)
        .await;
        if let Err(error) = inserted {
            // The PostgreSQL partial unique index closes the check-then-insert race. A concurrent
            // enqueue should observe the existing active job instead of surfacing a conflict.
            if crate::entities::seo_sitemap_job::Entity::find()
                .filter(crate::entities::seo_sitemap_job::Column::TenantId.eq(tenant.id))
                .filter(crate::entities::seo_sitemap_job::Column::Status.is_in([
                    SITEMAP_JOB_QUEUED,
                    SITEMAP_JOB_RUNNING,
                    SITEMAP_JOB_SUBMITTING,
                ]))
                .one(&self.db)
                .await?
                .is_some()
            {
                return self.sitemap_status(tenant).await;
            }
            return Err(error.into());
        }

        self.sitemap_status(tenant).await
    }

    pub(super) async fn execute_next_sitemap_job_background(
        &self,
    ) -> SeoResult<Option<crate::dto::SeoSitemapJobRecord>> {
        const JOB_LEASE_SECS: i64 = 30 * 60;
        let now = chrono::Utc::now().fixed_offset();
        let stale_before = now - chrono::Duration::seconds(JOB_LEASE_SECS);

        // A fresh running/submitting job for one tenant must not stall queued work for every
        // other tenant. Only stale leases are resumed; the conditional update is the claim.
        let stale_active = crate::entities::seo_sitemap_job::Entity::find()
            .filter(
                crate::entities::seo_sitemap_job::Column::Status
                    .is_in([SITEMAP_JOB_RUNNING, SITEMAP_JOB_SUBMITTING]),
            )
            .filter(crate::entities::seo_sitemap_job::Column::UpdatedAt.lte(stale_before))
            .order_by_asc(crate::entities::seo_sitemap_job::Column::UpdatedAt)
            .one(&self.db)
            .await?;

        let job = if let Some(job) = stale_active {
            let claimed = crate::entities::seo_sitemap_job::Entity::update_many()
                .col_expr(
                    crate::entities::seo_sitemap_job::Column::UpdatedAt,
                    sea_orm::sea_query::Expr::value(now),
                )
                .filter(crate::entities::seo_sitemap_job::Column::Id.eq(job.id))
                .filter(
                    crate::entities::seo_sitemap_job::Column::Status
                        .is_in([SITEMAP_JOB_RUNNING, SITEMAP_JOB_SUBMITTING]),
                )
                .filter(crate::entities::seo_sitemap_job::Column::UpdatedAt.lte(stale_before))
                .exec(&self.db)
                .await?;

            if claimed.rows_affected != 1 {
                return Ok(None);
            }

            crate::entities::seo_sitemap_job::Entity::find_by_id(job.id)
                .one(&self.db)
                .await?
                .ok_or(SeoError::NotFound)?
        } else {
            let Some(job) = crate::entities::seo_sitemap_job::Entity::find()
                .filter(crate::entities::seo_sitemap_job::Column::Status.eq(SITEMAP_JOB_QUEUED))
                .order_by_asc(crate::entities::seo_sitemap_job::Column::CreatedAt)
                .one(&self.db)
                .await?
            else {
                return Ok(None);
            };

            let claimed = crate::entities::seo_sitemap_job::Entity::update_many()
                .col_expr(
                    crate::entities::seo_sitemap_job::Column::Status,
                    sea_orm::sea_query::Expr::value(SITEMAP_JOB_RUNNING),
                )
                .col_expr(
                    crate::entities::seo_sitemap_job::Column::StartedAt,
                    sea_orm::sea_query::Expr::value(Some(now)),
                )
                .col_expr(
                    crate::entities::seo_sitemap_job::Column::UpdatedAt,
                    sea_orm::sea_query::Expr::value(now),
                )
                .filter(crate::entities::seo_sitemap_job::Column::Id.eq(job.id))
                .filter(crate::entities::seo_sitemap_job::Column::Status.eq(SITEMAP_JOB_QUEUED))
                .exec(&self.db)
                .await?;

            if claimed.rows_affected != 1 {
                return Ok(None);
            }

            crate::entities::seo_sitemap_job::Entity::find_by_id(job.id)
                .one(&self.db)
                .await?
                .ok_or(SeoError::NotFound)?
        };

        let module_enabled = self.runtime_module_enabled(job.tenant_id).await?;
        let result = if !module_enabled {
            Err(SeoError::configuration(
                "SEO module was disabled after the sitemap job was queued",
            ))
        } else if job.status == SITEMAP_JOB_SUBMITTING {
            self.execute_sitemap_submission_phase(&job).await
        } else {
            self.execute_sitemap_generation_phase(&job).await
        };
        if let Err(error) = result {
            self.fail_background_sitemap_job(&job, error.to_string())
                .await?;
        }
        self.prune_sitemap_history(job.tenant_id).await?;

        self.sitemap_job(job.tenant_id, job.id).await
    }

    async fn prune_sitemap_history(&self, tenant_id: Uuid) -> SeoResult<()> {
        let cutoff = chrono::Utc::now().fixed_offset()
            - chrono::Duration::days(super::SEO_HISTORY_RETENTION_DAYS);
        let old_jobs = crate::entities::seo_sitemap_job::Entity::find()
            .filter(crate::entities::seo_sitemap_job::Column::TenantId.eq(tenant_id))
            .filter(
                crate::entities::seo_sitemap_job::Column::Status
                    .is_in([SITEMAP_JOB_COMPLETED, SITEMAP_JOB_FAILED]),
            )
            .filter(crate::entities::seo_sitemap_job::Column::CreatedAt.lt(cutoff))
            .order_by_asc(crate::entities::seo_sitemap_job::Column::CreatedAt)
            .limit(super::SEO_HISTORY_PRUNE_BATCH_SIZE as u64)
            .all(&self.db)
            .await?;
        if old_jobs.is_empty() {
            return Ok(());
        }
        let ids = old_jobs.into_iter().map(|job| job.id).collect::<Vec<_>>();
        crate::entities::seo_sitemap_job::Entity::delete_many()
            .filter(crate::entities::seo_sitemap_job::Column::Id.is_in(ids))
            .exec(&self.db)
            .await?;
        Ok(())
    }

    async fn execute_sitemap_generation_phase(
        &self,
        job: &crate::entities::seo_sitemap_job::Model,
    ) -> SeoResult<()> {
        let tenant = self.load_background_sitemap_tenant(job.tenant_id).await?;
        let settings = self.load_settings(tenant.id).await?;
        if !self.runtime_module_enabled(tenant.id).await? || !sitemaps_enabled(&settings) {
            return Err(SeoError::configuration(
                "sitemap generation was disabled after the job was queued",
            ));
        }

        let preview = self.robots_preview(&tenant).await?;
        let public_origin = preview
            .public_url
            .strip_suffix("/robots.txt")
            .ok_or_else(|| SeoError::configuration("invalid SEO robots public URL"))?
            .to_string();
        let urls = self
            .collect_background_sitemap_urls(
                &tenant,
                public_origin.as_str(),
                &settings.sitemap_exclude_patterns,
            )
            .await?;
        if urls.len() > super::MAX_SITEMAP_URLS {
            return Err(SeoError::validation(format!(
                "sitemap URL set exceeds the {} URL limit",
                super::MAX_SITEMAP_URLS
            )));
        }
        let generated_at = chrono::Utc::now().fixed_offset();
        self.persist_background_sitemap_generation(
            job,
            &tenant,
            public_origin.as_str(),
            urls.as_slice(),
            settings.sitemap_max_entries_per_file as usize,
            settings.sitemap_changefreq.as_str(),
            settings.sitemap_priority.as_str(),
            settings.sitemap_include_images,
            settings.hreflang_in_sitemap
                && settings.hreflang_enabled
                && settings.submodule_hreflang_enabled,
            !settings.sitemap_submission_endpoints.is_empty(),
            generated_at,
        )
        .await
    }

    async fn execute_sitemap_submission_phase(
        &self,
        job: &crate::entities::seo_sitemap_job::Model,
    ) -> SeoResult<()> {
        let tenant = self.load_background_sitemap_tenant(job.tenant_id).await?;
        let settings = self.load_settings(tenant.id).await?;
        if !sitemaps_enabled(&settings) || !self.any_public_sitemap_owner_enabled(tenant.id).await?
        {
            return Err(SeoError::configuration(
                "sitemap submission was disabled after the job was queued",
            ));
        }
        let preview = self.robots_preview(&tenant).await?;
        let sitemap_index_url = preview
            .public_url
            .strip_suffix("/robots.txt")
            .map(|origin| format!("{origin}/sitemap.xml"))
            .ok_or_else(|| SeoError::configuration("invalid SEO robots public URL"))?;
        let endpoint_count = settings.sitemap_submission_endpoints.len() as i32;
        let error = if settings.sitemap_submission_endpoints.is_empty() {
            None
        } else {
            match submit_background_sitemap_endpoints(
                settings.sitemap_submission_endpoints.as_slice(),
                sitemap_index_url.as_str(),
            )
            .await
            {
                Ok(()) => None,
                Err(error) => {
                    tracing::warn!(
                        tenant_id = %tenant.id,
                        job_id = %job.id,
                        error = %error,
                        "background SEO sitemap submission failed"
                    );
                    Some(error)
                }
            }
        };

        self.record_background_sitemap_submission(job, endpoint_count, error)
            .await
    }

    async fn load_background_sitemap_tenant(&self, tenant_id: Uuid) -> SeoResult<TenantContext> {
        let tenant = rustok_tenant::TenantService::new(self.db.clone())
            .get_tenant(tenant_id)
            .await
            .map_err(|err| match err {
                rustok_tenant::TenantError::NotFound => SeoError::NotFound,
                rustok_tenant::TenantError::Database(db_err) => SeoError::Database(db_err),
                rustok_tenant::TenantError::Validation(msg) => SeoError::validation(msg),
                other => SeoError::configuration(other.to_string()),
            })?;
        Ok(TenantContext {
            id: tenant.id,
            name: tenant.name,
            slug: tenant.slug,
            domain: tenant.domain,
            settings: tenant.settings,
            default_locale: tenant.default_locale,
            is_active: tenant.is_active,
        })
    }

    async fn collect_background_sitemap_urls(
        &self,
        tenant: &TenantContext,
        public_origin: &str,
        exclude_patterns: &[String],
    ) -> SeoResult<Vec<SitemapUrlRecord>> {
        let mut urls = Vec::new();
        for provider in self
            .registry
            .providers_with_capability(rustok_seo_targets::SeoTargetCapabilityKind::Sitemaps)
        {
            if !self
                .owner_module_enabled(tenant.id, provider.owner_module_slug())
                .await?
            {
                continue;
            }
            let candidates = provider
                .sitemap_candidates(
                    &self.target_runtime(),
                    rustok_seo_targets::SeoTargetSitemapRequest {
                        tenant_id: tenant.id,
                        default_locale: tenant.default_locale.as_str(),
                    },
                )
                .await
                .map_err(|error| {
                    SeoError::validation(format!(
                        "SEO target provider `{}` failed to collect sitemap candidates: {error}",
                        provider.slug().as_str()
                    ))
                })?;
            let candidate_target_ids: Vec<Uuid> = candidates.iter().map(|c| c.target_id).collect();
            let noindex_ids = super::sitemaps::load_noindex_target_ids(
                &self.db,
                tenant.id,
                provider.slug().as_str(),
                candidate_target_ids.as_slice(),
            )
            .await?;
            for candidate in candidates {
                if noindex_ids.contains(&candidate.target_id) {
                    continue;
                }
                let locale = super::normalize_effective_locale(
                    candidate.locale.as_str(),
                    tenant.default_locale.as_str(),
                )?;
                let route =
                    super::sitemaps::sitemap_locale_path(locale.as_str(), candidate.route.as_str());
                if !super::sitemaps::sitemap_route_excluded(
                    candidate.route.as_str(),
                    exclude_patterns,
                ) && !super::sitemaps::sitemap_route_excluded(route.as_str(), exclude_patterns)
                {
                    let alternates = candidate
                        .alternates
                        .into_iter()
                        .filter_map(|alternate| {
                            let alternate_locale = super::normalize_effective_locale(
                                alternate.locale.as_str(),
                                tenant.default_locale.as_str(),
                            )
                            .ok()?;
                            let alternate_route = super::sitemaps::sitemap_locale_path(
                                alternate_locale.as_str(),
                                alternate.route.as_str(),
                            );
                            Some(rustok_seo_targets::SeoTargetAlternateRoute {
                                locale: alternate_locale,
                                route: super::sitemaps::sitemap_public_url(
                                    public_origin,
                                    alternate_route.as_str(),
                                ),
                            })
                        })
                        .collect();
                    let images = candidate
                        .images
                        .into_iter()
                        .filter_map(|mut image| {
                            let is_absolute = image.url.starts_with("http://")
                                || image.url.starts_with("https://");
                            let is_root_relative =
                                image.url.starts_with('/') && !image.url.starts_with("//");
                            if is_root_relative {
                                image.url = super::sitemaps::sitemap_public_url(
                                    public_origin,
                                    image.url.as_str(),
                                );
                            }
                            (is_absolute || is_root_relative).then_some(image)
                        })
                        .collect();
                    urls.push(SitemapUrlRecord {
                        url: super::sitemaps::sitemap_public_url(public_origin, route.as_str()),
                        images,
                        alternates,
                    });
                }
            }
        }

        urls.sort_by(|left, right| left.url.cmp(&right.url));
        urls.dedup_by(|left, right| left.url == right.url);
        Ok(urls)
    }

    async fn persist_background_sitemap_generation(
        &self,
        job: &crate::entities::seo_sitemap_job::Model,
        tenant: &TenantContext,
        public_origin: &str,
        urls: &[SitemapUrlRecord],
        chunk_size: usize,
        changefreq: &str,
        priority: &str,
        include_images: bool,
        include_hreflang: bool,
        requires_submission: bool,
        generated_at: chrono::DateTime<chrono::FixedOffset>,
    ) -> SeoResult<()> {
        let txn = self.db.begin().await?;
        let file_count = background_sitemap_file_count(urls.len(), chunk_size) as i32;

        crate::entities::seo_sitemap_file::Entity::delete_many()
            .filter(crate::entities::seo_sitemap_file::Column::TenantId.eq(tenant.id))
            .filter(crate::entities::seo_sitemap_file::Column::JobId.eq(job.id))
            .exec(&txn)
            .await?;
        self.persist_background_sitemap_files(
            &txn,
            tenant,
            public_origin,
            job.id,
            urls,
            chunk_size,
            changefreq,
            priority,
            include_images,
            include_hreflang,
            generated_at,
        )
        .await?;

        let mut active: crate::entities::seo_sitemap_job::ActiveModel = job.clone().into();
        active.status = Set(if requires_submission {
            SITEMAP_JOB_SUBMITTING.to_string()
        } else {
            SITEMAP_JOB_COMPLETED.to_string()
        });
        active.file_count = Set(file_count);
        active.last_error = Set(None);
        active.completed_at = Set((!requires_submission).then_some(generated_at));
        active.updated_at = Set(generated_at);
        active.update(&txn).await?;

        let event_type = "seo.sitemap.generated";
        let idempotency_key = background_sitemap_event_key(
            event_type,
            tenant.id,
            &[job.id.to_string(), file_count.to_string()],
        );
        self.publish_background_sitemap_event(
            &txn,
            BackgroundSitemapEventPublication {
                tenant_id: tenant.id,
                job_id: job.id,
                event_type: event_type.to_string(),
                idempotency_key: idempotency_key.clone(),
                event: rustok_core::DomainEvent::SeoSitemapGenerated {
                    job_id: job.id,
                    file_count,
                    idempotency_key,
                },
                occurred_at: generated_at,
            },
        )
        .await?;

        txn.commit().await?;
        Ok(())
    }

    async fn persist_background_sitemap_files(
        &self,
        txn: &sea_orm::DatabaseTransaction,
        tenant: &TenantContext,
        public_origin: &str,
        job_id: Uuid,
        urls: &[SitemapUrlRecord],
        chunk_size: usize,
        changefreq: &str,
        priority: &str,
        include_images: bool,
        include_hreflang: bool,
        now: chrono::DateTime<chrono::FixedOffset>,
    ) -> SeoResult<()> {
        let mut files = Vec::new();
        let chunks = if chunk_size == crate::services::SITEMAP_CHUNK_SIZE {
            urls.chunks(crate::services::SITEMAP_CHUNK_SIZE)
                .collect::<Vec<_>>()
        } else {
            urls.chunks(chunk_size.max(1)).collect::<Vec<_>>()
        };
        for (index, chunk) in chunks.into_iter().enumerate() {
            files.push(
                crate::entities::seo_sitemap_file::ActiveModel {
                    id: Set(Uuid::new_v4()),
                    tenant_id: Set(tenant.id),
                    job_id: Set(job_id),
                    path: Set(format!("sitemap-{}.xml", index + 1)),
                    url_count: Set(chunk.len() as i32),
                    content: Set(index_generation::render_sitemap_file(
                        chunk,
                        changefreq,
                        priority,
                        include_images,
                        include_hreflang,
                    )),
                    created_at: Set(now),
                    updated_at: Set(now),
                }
                .insert(txn)
                .await?,
            );
        }

        let index_urls = files
            .iter()
            .map(|file| format!("{public_origin}/sitemaps/{}", file.path))
            .collect::<Vec<_>>();
        crate::entities::seo_sitemap_file::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant.id),
            job_id: Set(job_id),
            path: Set("sitemap.xml".to_string()),
            url_count: Set(urls.len() as i32),
            content: Set(index_generation::render_sitemap_index(
                index_urls.as_slice(),
            )),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(txn)
        .await?;
        Ok(())
    }

    async fn record_background_sitemap_submission(
        &self,
        job: &crate::entities::seo_sitemap_job::Model,
        endpoint_count: i32,
        error: Option<String>,
    ) -> SeoResult<()> {
        let txn = self.db.begin().await?;
        let current = crate::entities::seo_sitemap_job::Entity::find_by_id(job.id)
            .filter(crate::entities::seo_sitemap_job::Column::TenantId.eq(job.tenant_id))
            .one(&txn)
            .await?
            .ok_or(SeoError::NotFound)?;
        let now = chrono::Utc::now().fixed_offset();
        let success = error.is_none();
        let mut active: crate::entities::seo_sitemap_job::ActiveModel = current.into();
        active.status = Set(SITEMAP_JOB_COMPLETED.to_string());
        active.last_error = Set(error.clone());
        active.completed_at = Set(Some(now));
        active.updated_at = Set(now);
        active.update(&txn).await?;

        let event_type = "seo.sitemap.submitted";
        let idempotency_key = background_sitemap_event_key(
            event_type,
            job.tenant_id,
            &[
                job.id.to_string(),
                endpoint_count.to_string(),
                success.to_string(),
            ],
        );
        self.publish_background_sitemap_event(
            &txn,
            BackgroundSitemapEventPublication {
                tenant_id: job.tenant_id,
                job_id: job.id,
                event_type: event_type.to_string(),
                idempotency_key: idempotency_key.clone(),
                event: rustok_core::DomainEvent::SeoSitemapSubmitted {
                    job_id: job.id,
                    endpoint_count,
                    success,
                    error,
                    idempotency_key,
                },
                occurred_at: now,
            },
        )
        .await?;

        txn.commit().await?;
        Ok(())
    }

    async fn publish_background_sitemap_event(
        &self,
        txn: &sea_orm::DatabaseTransaction,
        publication: BackgroundSitemapEventPublication,
    ) -> SeoResult<()> {
        let existing = crate::entities::seo_event_delivery::Entity::find()
            .filter(crate::entities::seo_event_delivery::Column::TenantId.eq(publication.tenant_id))
            .filter(
                crate::entities::seo_event_delivery::Column::IdempotencyKey
                    .eq(publication.idempotency_key.as_str()),
            )
            .one(txn)
            .await?;
        if existing.is_some() {
            return Ok(());
        }

        let outbox_event_id = self
            .event_bus
            .publish_in_tx_with_envelope_id(txn, publication.tenant_id, None, publication.event)
            .await
            .map_err(background_sitemap_transactional_event_error)?;
        crate::entities::seo_event_delivery::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(publication.tenant_id),
            event_type: Set(publication.event_type),
            idempotency_key: Set(publication.idempotency_key),
            source_kind: Set(Some("sitemap_job".to_string())),
            source_id: Set(Some(publication.job_id)),
            status: Set(SITEMAP_BACKGROUND_DELIVERY_STATUS_SENT.to_string()),
            outbox_event_id: Set(Some(outbox_event_id)),
            last_error: Set(None),
            created_at: Set(publication.occurred_at),
            updated_at: Set(publication.occurred_at),
            dispatched_at: Set(Some(publication.occurred_at)),
        }
        .insert(txn)
        .await?;
        Ok(())
    }

    async fn fail_background_sitemap_job(
        &self,
        job: &crate::entities::seo_sitemap_job::Model,
        message: String,
    ) -> SeoResult<()> {
        let now = chrono::Utc::now().fixed_offset();
        let mut active: crate::entities::seo_sitemap_job::ActiveModel = job.clone().into();
        active.status = Set(SITEMAP_JOB_FAILED.to_string());
        active.last_error = Set(Some(rustok_core::truncate(message.trim(), 2048)));
        active.completed_at = Set(Some(now));
        active.updated_at = Set(now);
        active.update(&self.db).await?;
        Ok(())
    }
}

async fn submit_background_sitemap_endpoints(
    endpoints: &[String],
    sitemap_index_url: &str,
) -> Result<(), String> {
    let runtime = submission_adapters::SitemapSubmissionRuntime::default_with_timeout(
        SITEMAP_BACKGROUND_SUBMIT_TIMEOUT_SECS,
    )?;
    let mut summary = submission_aggregation::SitemapSubmissionSummary::default();
    for endpoint in endpoints {
        let Some(request_url) =
            build_background_sitemap_submission_url(endpoint.as_str(), sitemap_index_url)
        else {
            submission_aggregation::record_invalid_endpoint(&mut summary, endpoint.as_str());
            continue;
        };
        let request = submission_adapters::SitemapSubmitEndpoint {
            endpoint: endpoint.clone(),
            request_url,
        };
        match runtime.adapter().submit_sitemap_index(request).await {
            Ok(()) => {
                submission_aggregation::record_submission_success(&mut summary, endpoint.as_str())
            }
            Err(message) => {
                submission_aggregation::record_submission_failure(
                    &mut summary,
                    endpoint.as_str(),
                    message,
                );
            }
        }
    }

    let telemetry = summary.telemetry_snapshot();
    tracing::debug!(
        success_count = summary.success_count,
        failure_count = summary.failure_count,
        endpoint_status_count = telemetry.endpoint_statuses.len(),
        omitted_endpoint_status_count = telemetry.omitted_endpoint_status_count,
        "background SEO sitemap endpoint submission finished"
    );
    match summary.into_error() {
        Some(message) => Err(message),
        None => Ok(()),
    }
}

fn build_background_sitemap_submission_url(
    endpoint: &str,
    sitemap_index_url: &str,
) -> Option<String> {
    let normalized = endpoint.trim();
    if normalized.is_empty() {
        return None;
    }
    if normalized.contains("{sitemap_url}") {
        let encoded: String =
            url::form_urlencoded::byte_serialize(sitemap_index_url.as_bytes()).collect();
        let replaced = normalized.replace("{sitemap_url}", encoded.as_str());
        let parsed = url::Url::parse(replaced.as_str()).ok()?;
        if !matches!(parsed.scheme(), "http" | "https") {
            return None;
        }
        return Some(parsed.to_string());
    }
    let mut parsed = url::Url::parse(normalized).ok()?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return None;
    }
    if !parsed
        .query_pairs()
        .any(|(name, _)| name.eq_ignore_ascii_case("sitemap"))
    {
        parsed
            .query_pairs_mut()
            .append_pair("sitemap", sitemap_index_url);
    }
    Some(parsed.to_string())
}

fn background_sitemap_file_count(url_count: usize, chunk_size: usize) -> usize {
    if url_count == 0 {
        1
    } else {
        ((url_count - 1) / chunk_size.max(1)) + 2
    }
}

fn background_sitemap_event_key(scope: &str, tenant_id: Uuid, parts: &[String]) -> String {
    let mut payload = format!("{scope}|{tenant_id}");
    for part in parts {
        payload.push('|');
        payload.push_str(part.as_str());
    }
    format!(
        "{scope}:{:016x}",
        rustok_core::simple_hash(payload.as_str())
    )
}

fn background_sitemap_transactional_event_error(error: rustok_core::Error) -> SeoError {
    SeoError::Database(sea_orm::DbErr::Custom(format!(
        "failed to enqueue sitemap event transactionally: {error}"
    )))
}

fn background_disabled_sitemap_status() -> crate::dto::SeoSitemapStatusRecord {
    crate::dto::SeoSitemapStatusRecord {
        enabled: false,
        latest_job_id: None,
        status: None,
        file_count: 0,
        generated_at: None,
        files: Vec::new(),
    }
}

#[cfg(test)]
mod sitemap_background_tests {
    use super::*;
    use crate::services::SITEMAP_CHUNK_SIZE;

    #[test]
    fn background_file_count_keeps_the_index_and_chunks() {
        assert_eq!(background_sitemap_file_count(0, SITEMAP_CHUNK_SIZE), 1);
        assert_eq!(background_sitemap_file_count(1, SITEMAP_CHUNK_SIZE), 2);
        assert_eq!(
            background_sitemap_file_count(SITEMAP_CHUNK_SIZE + 1, SITEMAP_CHUNK_SIZE),
            3
        );
    }

    #[test]
    fn background_job_states_are_phase_distinct() {
        assert_ne!(SITEMAP_JOB_QUEUED, SITEMAP_JOB_RUNNING);
        assert_ne!(SITEMAP_JOB_RUNNING, SITEMAP_JOB_SUBMITTING);
        assert_ne!(SITEMAP_JOB_SUBMITTING, SITEMAP_JOB_COMPLETED);
    }
}
mod index_repair_background_impl {
    use super::*;
    use sea_orm::ActiveValue::Set;

    const INDEX_REPAIR_JOB_QUEUED: &str = "queued";
    const INDEX_REPAIR_JOB_RUNNING: &str = "running";
    const INDEX_REPAIR_JOB_COMPLETED: &str = "completed";
    const INDEX_REPAIR_JOB_FAILED: &str = "failed";
    const INDEX_REPAIR_JOB_MAX_LIMIT: usize = 500;

    mod job_entity {
        use sea_orm::entity::prelude::*;
        use serde::{Deserialize, Serialize};
        use uuid::Uuid;

        #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
        #[sea_orm(table_name = "seo_index_repair_jobs")]
        pub struct Model {
            #[sea_orm(primary_key, auto_increment = false)]
            pub id: Uuid,
            pub tenant_id: Uuid,
            pub status: String,
            pub target_type: Option<String>,
            pub limit: i32,
            pub replay_historical: bool,
            pub repaired_count: i32,
            pub replayed_count: i32,
            pub historical_events_scanned: i32,
            pub replay_run_id: Option<Uuid>,
            pub last_error: Option<String>,
            pub started_at: Option<DateTimeWithTimeZone>,
            pub completed_at: Option<DateTimeWithTimeZone>,
            pub created_at: DateTimeWithTimeZone,
            pub updated_at: DateTimeWithTimeZone,
        }

        #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
        pub enum Relation {}

        impl ActiveModelBehavior for ActiveModel {}
    }

    impl SeoService {
        pub(crate) async fn queue_index_repair_replay_background(
            &self,
            tenant_id: Uuid,
            target_type: Option<&str>,
            limit: usize,
            replay_historical: bool,
        ) -> SeoResult<crate::dto::SeoIndexRepairReplayResultRecord> {
            let target_type = normalize_background_index_target_type(target_type)?;
            let bounded_limit = limit.clamp(1, INDEX_REPAIR_JOB_MAX_LIMIT);

            let active = job_entity::Entity::find()
                .filter(job_entity::Column::TenantId.eq(tenant_id))
                .filter(
                    job_entity::Column::Status
                        .is_in([INDEX_REPAIR_JOB_QUEUED, INDEX_REPAIR_JOB_RUNNING]),
                )
                .order_by_asc(job_entity::Column::CreatedAt)
                .one(&self.db)
                .await?;
            if let Some(active) = active {
                return Ok(map_background_index_repair_job(&active));
            }

            let now = chrono::Utc::now().fixed_offset();
            let job = job_entity::ActiveModel {
                id: Set(Uuid::new_v4()),
                tenant_id: Set(tenant_id),
                status: Set(INDEX_REPAIR_JOB_QUEUED.to_string()),
                target_type: Set(target_type),
                limit: Set(bounded_limit as i32),
                replay_historical: Set(replay_historical),
                repaired_count: Set(0),
                replayed_count: Set(0),
                historical_events_scanned: Set(0),
                replay_run_id: Set(None),
                last_error: Set(None),
                started_at: Set(None),
                completed_at: Set(None),
                created_at: Set(now),
                updated_at: Set(now),
            }
            .insert(&self.db)
            .await;

            match job {
                Ok(job) => Ok(map_background_index_repair_job(&job)),
                Err(error) => {
                    // The partial unique index closes the check-then-insert race. A concurrent
                    // caller should observe the already queued job rather than receive a spurious
                    // database conflict.
                    if let Some(active) = job_entity::Entity::find()
                        .filter(job_entity::Column::TenantId.eq(tenant_id))
                        .filter(
                            job_entity::Column::Status
                                .is_in([INDEX_REPAIR_JOB_QUEUED, INDEX_REPAIR_JOB_RUNNING]),
                        )
                        .order_by_asc(job_entity::Column::CreatedAt)
                        .one(&self.db)
                        .await?
                    {
                        Ok(map_background_index_repair_job(&active))
                    } else {
                        Err(error.into())
                    }
                }
            }
        }

        pub(crate) async fn execute_next_index_repair_replay_job_background(
            &self,
        ) -> SeoResult<Option<crate::dto::SeoIndexRepairReplayResultRecord>> {
            const JOB_LEASE_SECS: i64 = 30 * 60;
            let now = chrono::Utc::now().fixed_offset();
            let stale_before = now - chrono::Duration::seconds(JOB_LEASE_SECS);

            // A fresh running job for one tenant must not block queued work for every other
            // tenant. Only look for stale leases here; an atomic claim below still protects
            // against two pollers resuming the same job concurrently.
            let stale_running = job_entity::Entity::find()
                .filter(job_entity::Column::Status.eq(INDEX_REPAIR_JOB_RUNNING))
                .filter(job_entity::Column::UpdatedAt.lte(stale_before))
                .order_by_asc(job_entity::Column::UpdatedAt)
                .one(&self.db)
                .await?;

            let job = if let Some(job) = stale_running {
                let claimed = job_entity::Entity::update_many()
                    .col_expr(
                        job_entity::Column::UpdatedAt,
                        sea_orm::sea_query::Expr::value(now),
                    )
                    .filter(job_entity::Column::Id.eq(job.id))
                    .filter(job_entity::Column::Status.eq(INDEX_REPAIR_JOB_RUNNING))
                    .filter(job_entity::Column::UpdatedAt.lte(stale_before))
                    .exec(&self.db)
                    .await?;

                if claimed.rows_affected != 1 {
                    return Ok(None);
                }

                job_entity::Entity::find_by_id(job.id)
                    .one(&self.db)
                    .await?
                    .ok_or(SeoError::NotFound)?
            } else {
                let Some(job) = job_entity::Entity::find()
                    .filter(job_entity::Column::Status.eq(INDEX_REPAIR_JOB_QUEUED))
                    .order_by_asc(job_entity::Column::CreatedAt)
                    .one(&self.db)
                    .await?
                else {
                    return Ok(None);
                };

                let claimed = job_entity::Entity::update_many()
                    .col_expr(
                        job_entity::Column::Status,
                        sea_orm::sea_query::Expr::value(INDEX_REPAIR_JOB_RUNNING),
                    )
                    .col_expr(
                        job_entity::Column::StartedAt,
                        sea_orm::sea_query::Expr::value(Some(now)),
                    )
                    .col_expr(
                        job_entity::Column::CompletedAt,
                        sea_orm::sea_query::Expr::value(
                            Option::<chrono::DateTime<chrono::FixedOffset>>::None,
                        ),
                    )
                    .col_expr(
                        job_entity::Column::LastError,
                        sea_orm::sea_query::Expr::value(Option::<String>::None),
                    )
                    .col_expr(
                        job_entity::Column::UpdatedAt,
                        sea_orm::sea_query::Expr::value(now),
                    )
                    .filter(job_entity::Column::Id.eq(job.id))
                    .filter(job_entity::Column::Status.eq(INDEX_REPAIR_JOB_QUEUED))
                    .exec(&self.db)
                    .await?;

                if claimed.rows_affected != 1 {
                    return Ok(None);
                }

                job_entity::Entity::find_by_id(job.id)
                    .one(&self.db)
                    .await?
                    .ok_or(SeoError::NotFound)?
            };

            let result = if !self.runtime_module_enabled(job.tenant_id).await? {
                Err(SeoError::configuration(
                    "SEO module was disabled after the index repair job was queued",
                ))
            } else {
                self.run_index_repair_replay(
                    job.tenant_id,
                    job.target_type.as_deref(),
                    job.limit.clamp(1, INDEX_REPAIR_JOB_MAX_LIMIT as i32) as usize,
                    job.replay_historical,
                )
                .await
            };

            let result = match result {
                Ok(result) => result,
                Err(error) => {
                    self.fail_background_index_repair_job(&job, error.to_string())
                        .await?;
                    self.prune_index_repair_history(job.tenant_id).await?;
                    return Err(error);
                }
            };

            let current = job_entity::Entity::find_by_id(job.id)
                .one(&self.db)
                .await?
                .ok_or(SeoError::NotFound)?;
            let now = chrono::Utc::now().fixed_offset();
            let mut active: job_entity::ActiveModel = current.into();
            active.status = Set(INDEX_REPAIR_JOB_COMPLETED.to_string());
            active.repaired_count = Set(result.repaired_count);
            active.replayed_count = Set(result.replayed_count);
            active.historical_events_scanned = Set(result.historical_events_scanned);
            active.replay_run_id = Set(result.replay_run_id);
            active.last_error = Set(None);
            active.completed_at = Set(Some(now));
            active.updated_at = Set(now);
            let completed = active.update(&self.db).await?;
            self.prune_index_repair_history(job.tenant_id).await?;

            Ok(Some(map_background_index_repair_job(&completed)))
        }

        async fn prune_index_repair_history(&self, tenant_id: Uuid) -> SeoResult<()> {
            let cutoff = chrono::Utc::now().fixed_offset()
                - chrono::Duration::days(super::super::SEO_HISTORY_RETENTION_DAYS);
            let old_jobs = job_entity::Entity::find()
                .filter(job_entity::Column::TenantId.eq(tenant_id))
                .filter(
                    job_entity::Column::Status
                        .is_in([INDEX_REPAIR_JOB_COMPLETED, INDEX_REPAIR_JOB_FAILED]),
                )
                .filter(job_entity::Column::CreatedAt.lt(cutoff))
                .order_by_asc(job_entity::Column::CreatedAt)
                .limit(super::super::SEO_HISTORY_PRUNE_BATCH_SIZE as u64)
                .all(&self.db)
                .await?;
            if old_jobs.is_empty() {
                return Ok(());
            }
            let ids = old_jobs.into_iter().map(|job| job.id).collect::<Vec<_>>();
            job_entity::Entity::delete_many()
                .filter(job_entity::Column::Id.is_in(ids))
                .exec(&self.db)
                .await?;
            Ok(())
        }

        async fn fail_background_index_repair_job(
            &self,
            job: &job_entity::Model,
            message: String,
        ) -> SeoResult<()> {
            let current = job_entity::Entity::find_by_id(job.id)
                .one(&self.db)
                .await?
                .ok_or(SeoError::NotFound)?;
            let now = chrono::Utc::now().fixed_offset();
            let mut active: job_entity::ActiveModel = current.into();
            active.status = Set(INDEX_REPAIR_JOB_FAILED.to_string());
            active.last_error = Set(Some(rustok_core::truncate(message.trim(), 2048)));
            active.completed_at = Set(Some(now));
            active.updated_at = Set(now);
            active.update(&self.db).await?;
            Ok(())
        }
    }

    fn normalize_background_index_target_type(
        target_type: Option<&str>,
    ) -> SeoResult<Option<String>> {
        let Some(value) = target_type else {
            return Ok(None);
        };
        let normalized = value.trim().to_ascii_lowercase();
        if normalized.is_empty() {
            return Ok(None);
        }
        match normalized.as_str() {
            "content" | "product" => Ok(Some(normalized)),
            _ => Err(SeoError::validation(format!(
                "unsupported index target_type `{}`; expected `content` or `product`",
                value.trim()
            ))),
        }
    }

    fn map_background_index_repair_job(
        job: &job_entity::Model,
    ) -> crate::dto::SeoIndexRepairReplayResultRecord {
        let replay_mode = if job.status == INDEX_REPAIR_JOB_COMPLETED {
            if job.replay_historical {
                crate::dto::SeoIndexReplayMode::ReplayCompleted
            } else {
                crate::dto::SeoIndexReplayMode::RepairOnly
            }
        } else {
            crate::dto::SeoIndexReplayMode::NotStarted
        };
        crate::dto::SeoIndexRepairReplayResultRecord {
            target_type: job.target_type.clone(),
            limit: job.limit,
            replay_mode,
            repaired_count: job.repaired_count,
            replayed_count: job.replayed_count,
            historical_events_scanned: job.historical_events_scanned,
            replay_run_id: job.replay_run_id,
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn worker_limit_matches_existing_operator_bound() {
            assert_eq!(INDEX_REPAIR_JOB_MAX_LIMIT, 500);
        }

        #[test]
        fn worker_states_are_durable_and_distinct() {
            assert_ne!(INDEX_REPAIR_JOB_QUEUED, INDEX_REPAIR_JOB_RUNNING);
            assert_ne!(INDEX_REPAIR_JOB_RUNNING, INDEX_REPAIR_JOB_COMPLETED);
            assert_ne!(INDEX_REPAIR_JOB_COMPLETED, INDEX_REPAIR_JOB_FAILED);
        }
    }
}

#[cfg(test)]
mod bulk_read_model {
    pub(super) use super::super::bulk_read_model::BulkReadProjection;
}
