use std::collections::{HashMap, HashSet};

use async_graphql::Json;
use chrono::{DateTime, Utc};
use csv::{ReaderBuilder, StringRecord, WriterBuilder};
use sea_orm::ActiveValue::Set;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use rustok_api::TenantContext;
use rustok_seo_targets::{SeoTargetBulkListRequest, SeoTargetSlug};

use crate::dto::{
    SeoBulkApplyInput, SeoBulkApplyMode, SeoBulkArtifactRecord, SeoBulkBoolFieldPatch,
    SeoBulkExportInput, SeoBulkFieldPatchMode, SeoBulkImportInput, SeoBulkItem,
    SeoBulkJobOperationKind, SeoBulkJobRecord, SeoBulkJobStatus, SeoBulkJsonFieldPatch,
    SeoBulkListInput, SeoBulkMetaPatchInput, SeoBulkPage, SeoBulkSelectionInput,
    SeoBulkSelectionMode, SeoBulkSelectionPreviewRecord, SeoBulkSource, SeoBulkStringFieldPatch,
    SeoMetaInput, SeoMetaRecord, SeoMetaTranslationInput,
};
use crate::entities::{seo_bulk_job, seo_bulk_job_artifact, seo_bulk_job_item};
use crate::{SeoError, SeoResult};

use super::SeoService;

const MAX_BULK_PAGE_SIZE: i32 = 100;
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
    page: i32,
    per_page: i32,
}

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
        page: input.page.max(1),
        per_page: input.per_page.clamp(1, MAX_BULK_PAGE_SIZE),
    })
}

#[cfg(test)]
fn validate_bulk_patch(patch: &SeoBulkMetaPatchInput) -> SeoResult<()> {
    validate_bulk_patch_shape(patch)?;
    validate_bulk_patch_has_change(patch)
}

fn validate_bulk_apply(input: &SeoBulkApplyInput) -> SeoResult<()> {
    validate_bulk_patch_shape(&input.patch)?;
    if matches!(
        input.apply_mode,
        SeoBulkApplyMode::PreviewOnly
            | SeoBulkApplyMode::ApplyMissingOnly
            | SeoBulkApplyMode::OverwriteGeneratedOnly
    ) {
        return Ok(());
    }
    validate_bulk_patch_has_change(&input.patch)
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
    if let Some(patch) = patch {
        if matches!(patch.mode, SeoBulkFieldPatchMode::Set) && patch.value.is_none() {
            return Err(SeoError::validation(format!(
                "bulk patch field `{field}` requires a boolean `value`"
            )));
        }
    }
    Ok(())
}

fn validate_json_field_patch(patch: Option<&SeoBulkJsonFieldPatch>, field: &str) -> SeoResult<()> {
    if let Some(patch) = patch {
        if matches!(patch.mode, SeoBulkFieldPatchMode::Set) && patch.value.is_none() {
            return Err(SeoError::validation(format!(
                "bulk patch field `{field}` requires a JSON `value`"
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
                .map(|value| Some(value.0))
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
        operation_kind: SeoBulkJobOperationKind::from_str(model.operation_kind.as_str())
            .ok_or_else(|| SeoError::validation("invalid bulk operation kind"))?,
        status: SeoBulkJobStatus::from_str(model.status.as_str())
            .ok_or_else(|| SeoError::validation("invalid bulk job status"))?,
        target_kind: SeoTargetSlug::new(model.target_kind.as_str())
            .map_err(|_| SeoError::validation("invalid bulk target kind"))?,
        locale: model.locale,
        publish_after_write: model.publish_after_write,
        matched_count: model.matched_count,
        processed_count: model.processed_count,
        succeeded_count: model.succeeded_count,
        failed_count: model.failed_count,
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
        .map(str::to_string)
}

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
        record.translation.title.clone().unwrap_or_default(),
        record.translation.description.clone().unwrap_or_default(),
        record.translation.keywords.clone().unwrap_or_default(),
        record.canonical_url.clone().unwrap_or_default(),
        record.translation.og_title.clone().unwrap_or_default(),
        record
            .translation
            .og_description
            .clone()
            .unwrap_or_default(),
        record.translation.og_image.clone().unwrap_or_default(),
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
        row.title.clone().unwrap_or_default(),
        row.description.clone().unwrap_or_default(),
        row.keywords.clone().unwrap_or_default(),
        row.canonical_url.clone().unwrap_or_default(),
        row.og_title.clone().unwrap_or_default(),
        row.og_description.clone().unwrap_or_default(),
        row.og_image.clone().unwrap_or_default(),
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
        seo_builtin_slug, SeoBulkFieldPatchMode, SeoMetaRecord, SeoMetaTranslationRecord,
        SeoTargetSlug,
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
                    page: 1,
                    per_page: 20,
                }),
            },
            patch: noop_patch(),
            apply_mode,
            publish_after_write: true,
        }
    }

    #[test]
    fn normalize_bulk_list_input_canonicalizes_locale_and_bounds_page() {
        let filter = normalize_bulk_list_input(
            SeoBulkListInput {
                target_kind: page_slug(),
                locale: "en-us".to_string(),
                query: Some("  Sale  ".to_string()),
                source: None,
                page: 0,
                per_page: 999,
            },
            "ru-RU",
        )
        .expect("normalize filter");

        assert_eq!(filter.locale, "en-US");
        assert_eq!(filter.query.as_deref(), Some("sale"));
        assert_eq!(filter.page, 1);
        assert_eq!(filter.per_page, MAX_BULK_PAGE_SIZE);
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

        assert!(csv
            .lines()
            .next()
            .unwrap_or_default()
            .ends_with("error_message"));
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

        assert!(csv
            .lines()
            .next()
            .unwrap_or_default()
            .starts_with("target_kind,target_id,locale"));
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
}

#[derive(Debug, Clone)]
struct BulkTargetSummary {
    target_id: Uuid,
    label: String,
    route: String,
}

#[derive(Debug, Clone)]
struct BulkScopedSummary {
    summary: BulkTargetSummary,
    source: SeoBulkSource,
}

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

impl SeoService {
    pub async fn list_bulk_items(
        &self,
        tenant: &TenantContext,
        input: SeoBulkListInput,
    ) -> SeoResult<SeoBulkPage> {
        let filter = normalize_bulk_list_input(input, tenant.default_locale.as_str())?;
        let scoped = self.collect_bulk_scope(tenant, &filter).await?;
        let total = scoped.len() as i32;
        let offset = ((filter.page - 1) * filter.per_page) as usize;
        let items = scoped
            .into_iter()
            .skip(offset)
            .take(filter.per_page as usize)
            .collect::<Vec<_>>();

        let mut page_items = Vec::with_capacity(items.len());
        for item in items {
            let record = self
                .seo_meta(
                    tenant,
                    filter.target_kind.clone(),
                    item.summary.target_id,
                    Some(filter.locale.as_str()),
                )
                .await?
                .ok_or(SeoError::NotFound)?;
            page_items.push(SeoBulkItem {
                target_kind: filter.target_kind.clone(),
                target_id: item.summary.target_id,
                locale: filter.locale.clone(),
                effective_locale: record.effective_locale.clone(),
                label: item.summary.label,
                route: item.summary.route,
                source: map_bulk_source(record.source.as_str()).unwrap_or(item.source),
                title: record.translation.title,
                description: record.translation.description,
                canonical_url: record.canonical_url,
                noindex: record.noindex,
                nofollow: record.nofollow,
            });
        }

        Ok(SeoBulkPage {
            items: page_items,
            total,
            page: filter.page,
            per_page: filter.per_page,
        })
    }

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
        let jobs = query.all(&self.db).await?;
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

        let result = match SeoBulkJobOperationKind::from_str(running.operation_kind.as_str()) {
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
        let tenant = rustok_tenant::entities::tenant::Entity::find_by_id(tenant_id)
            .one(&self.db)
            .await?
            .ok_or(SeoError::NotFound)?;
        Ok(TenantContext {
            id: tenant.id,
            name: tenant.name,
            slug: tenant.slug,
            domain: tenant.domain,
            settings: tenant.settings.into(),
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
                Ok(revision) => {
                    succeeded += 1;
                    self.insert_bulk_job_item(job, target_id, None, revision)
                        .await?;
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

        self.finish_bulk_job(job, succeeded + failed, succeeded, failed, artifacts, None)
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

        self.finish_bulk_job(job, succeeded + failed, succeeded, failed, artifacts, None)
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

        self.finish_bulk_job(job, succeeded + failed, succeeded, failed, artifacts, None)
            .await
    }

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
    ) -> SeoResult<Option<i32>> {
        let current = self
            .seo_meta(tenant, target_kind.clone(), target_id, Some(locale))
            .await?
            .ok_or(SeoError::NotFound)?;
        let current_source =
            map_bulk_source(current.source.as_str()).unwrap_or(SeoBulkSource::Fallback);
        match apply_mode {
            SeoBulkApplyMode::PreviewOnly => {
                return Err(SeoError::validation(
                    "bulk apply mode `preview_only` does not write SEO records",
                ));
            }
            SeoBulkApplyMode::ApplyMissingOnly if current_source == SeoBulkSource::Explicit => {
                return Err(SeoError::validation(
                    "bulk apply mode `apply_missing_only` does not overwrite explicit SEO",
                ));
            }
            SeoBulkApplyMode::OverwriteGeneratedOnly
                if current_source != SeoBulkSource::Generated =>
            {
                return Err(SeoError::validation(format!(
                    "bulk apply mode `overwrite_generated_only` requires generated SEO, got `{}`",
                    current_source.as_str()
                )));
            }
            SeoBulkApplyMode::ApplyMissingOnly
            | SeoBulkApplyMode::OverwriteGeneratedOnly
            | SeoBulkApplyMode::ForceOverwriteExplicit => {}
        }
        let input = SeoMetaInput {
            target_kind: target_kind.clone(),
            target_id,
            noindex: apply_bool_patch(current.noindex, patch.noindex.as_ref())?,
            nofollow: apply_bool_patch(current.nofollow, patch.nofollow.as_ref())?,
            canonical_url: apply_string_patch(current.canonical_url, patch.canonical_url.as_ref())?,
            structured_data: apply_json_patch(
                current.structured_data.map(|value| value.0),
                patch.structured_data.as_ref(),
            )?
            .map(Json),
            translations: vec![SeoMetaTranslationInput {
                locale: locale.to_string(),
                title: apply_string_patch(current.translation.title, patch.title.as_ref())?,
                description: apply_string_patch(
                    current.translation.description,
                    patch.description.as_ref(),
                )?,
                keywords: apply_string_patch(
                    current.translation.keywords,
                    patch.keywords.as_ref(),
                )?,
                og_title: apply_string_patch(
                    current.translation.og_title,
                    patch.og_title.as_ref(),
                )?,
                og_description: apply_string_patch(
                    current.translation.og_description,
                    patch.og_description.as_ref(),
                )?,
                og_image: apply_string_patch(
                    current.translation.og_image,
                    patch.og_image.as_ref(),
                )?,
            }],
        };

        self.upsert_meta(tenant, input).await?;
        if publish_after_write {
            let revision = self
                .publish_revision(
                    tenant,
                    target_kind,
                    target_id,
                    Some(format!("bulk job {} apply", job_id)),
                )
                .await?;
            return Ok(Some(revision.revision));
        }

        Ok(None)
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

        self.upsert_meta(tenant, input).await?;
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
        let mut active: seo_bulk_job::ActiveModel = job.clone().into();
        active.status = Set(SeoBulkJobStatus::Failed.as_str().to_string());
        active.last_error = Set(Some(limit_job_message(message)));
        active.completed_at = Set(Some(now));
        active.updated_at = Set(now);
        active.update(&self.db).await?;
        Ok(())
    }

    async fn finish_bulk_job(
        &self,
        job: &seo_bulk_job::Model,
        processed_count: i32,
        succeeded_count: i32,
        failed_count: i32,
        artifact_count: i32,
        last_error: Option<String>,
    ) -> SeoResult<()> {
        let status = if failed_count == 0 {
            SeoBulkJobStatus::Completed
        } else if succeeded_count == 0 {
            SeoBulkJobStatus::Failed
        } else {
            SeoBulkJobStatus::Partial
        };
        let now = Utc::now().fixed_offset();
        let mut active: seo_bulk_job::ActiveModel = job.clone().into();
        active.status = Set(status.as_str().to_string());
        active.processed_count = Set(processed_count);
        active.succeeded_count = Set(succeeded_count);
        active.failed_count = Set(failed_count);
        active.artifact_count = Set(artifact_count);
        active.last_error = Set(last_error.map(limit_job_message));
        active.completed_at = Set(Some(now));
        active.updated_at = Set(now);
        active.update(&self.db).await?;
        Ok(())
    }

    async fn insert_bulk_job_item(
        &self,
        job: &seo_bulk_job::Model,
        target_id: Uuid,
        error_message: Option<String>,
        published_revision: Option<i32>,
    ) -> SeoResult<()> {
        let now = Utc::now().fixed_offset();
        seo_bulk_job_item::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(job.tenant_id),
            job_id: Set(job.id),
            target_id: Set(target_id),
            status: Set(if error_message.is_some() {
                "failed".to_string()
            } else {
                "completed".to_string()
            }),
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
