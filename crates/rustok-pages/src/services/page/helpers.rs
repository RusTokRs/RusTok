use std::collections::BTreeMap;

use chrono::Utc;
use sea_orm::{
    sea_query::{Expr, Query, SelectStatement},
    ActiveValue::Set,
    Condition, QueryFilter, Select,
};
use uuid::Uuid;

use rustok_content::{
    available_locales_from, entities::node::ContentStatus, normalize_locale_code,
    resolve_by_locale_with_fallback,
};
use rustok_core::{
    normalize_content_format, prepare_content_payload, CONTENT_FORMAT_GRAPESJS,
    CONTENT_FORMAT_RT_JSON_V1,
};
use rustok_events::DomainEvent;

use crate::dto::{
    PageBodyInput, PageBodyResponse, PageTranslationInput, PageTranslationResponse,
};
use crate::entities::{page, page_body, page_channel_visibility, page_translation};
use crate::error::{PagesError, PagesResult};
use crate::services::page_builder_artifact::CompiledLandingArtifact;
use crate::services::PageBuilderArtifactService;

use super::{PageTransition, PreparedPageBody, ResolvedBodyRecord, ResolvedTranslationRecord};

pub(super) fn validate_page_translations(
    translations: &[PageTranslationInput],
) -> PagesResult<()> {
    if translations.is_empty() {
        return Err(PagesError::validation(
            "At least one page translation is required",
        ));
    }
    for translation in translations {
        if translation.locale.trim().is_empty() {
            return Err(PagesError::validation("Translation locale cannot be empty"));
        }
        if translation.title.trim().is_empty() {
            return Err(PagesError::validation("Page title cannot be empty"));
        }
    }
    Ok(())
}

pub(super) fn normalize_page_body_input(
    body: Option<PageBodyInput>,
) -> PagesResult<Option<PreparedPageBody>> {
    let Some(body) = body else {
        return Ok(None);
    };
    let locale = normalize_locale(&body.locale)?;
    let format =
        normalize_content_format(body.format.as_deref()).map_err(PagesError::validation)?;
    if body_requires_json_payload(&format)
        && body.content_json.is_none()
        && body.content.trim().is_empty()
    {
        return Err(PagesError::validation(format!(
            "content_json is required for {format} format"
        )));
    }
    let markdown_source = if body.content.trim().is_empty() {
        None
    } else {
        Some(body.content.as_str())
    };
    let prepared_body = prepare_content_payload(
        Some(&format),
        markdown_source,
        body.content_json.as_ref(),
        &locale,
        "Body",
    )
    .map_err(PagesError::validation)?;
    Ok(Some(PreparedPageBody {
        locale,
        content: prepared_body.body,
        format: prepared_body.format,
    }))
}

pub(super) fn normalize_locale(locale: &str) -> PagesResult<String> {
    normalize_locale_code(locale).ok_or_else(|| PagesError::validation("Invalid locale"))
}

pub(super) fn normalize_slug(value: &str) -> String {
    let mut normalized = String::with_capacity(value.len());
    let mut previous_dash = false;
    for ch in value.chars().flat_map(|ch| ch.to_lowercase()) {
        if ch.is_ascii_alphanumeric() {
            normalized.push(ch);
            previous_dash = false;
        } else if !previous_dash {
            normalized.push('-');
            previous_dash = true;
        }
    }
    normalized.trim_matches('-').to_string()
}

pub(super) fn is_builder_publish_enabled(settings: &serde_json::Value) -> bool {
    settings
        .get("builder")
        .and_then(|builder| builder.get("publish"))
        .and_then(|publish| publish.get("enabled"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(true)
}

pub(super) fn is_builder_enabled(settings: &serde_json::Value) -> bool {
    settings
        .get("builder")
        .and_then(|builder| builder.get("enabled"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(true)
}

pub(super) fn is_builder_preview_enabled(settings: &serde_json::Value) -> bool {
    settings
        .get("builder")
        .and_then(|builder| builder.get("preview"))
        .and_then(|preview| preview.get("enabled"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(true)
}

pub(super) fn is_builder_properties_enabled(settings: &serde_json::Value) -> bool {
    settings
        .get("builder")
        .and_then(|builder| builder.get("properties"))
        .and_then(|properties| properties.get("enabled"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(true)
}

pub(super) fn body_uses_builder_capability(body: Option<&PreparedPageBody>) -> bool {
    body.is_some_and(|item| item.format == CONTENT_FORMAT_GRAPESJS)
}

pub(super) fn resolve_translation_record<'a>(
    translations: &'a [page_translation::Model],
    requested: &str,
    fallback_locale: Option<&str>,
) -> ResolvedTranslationRecord<'a> {
    let resolved =
        resolve_by_locale_with_fallback(translations, requested, fallback_locale, |item| {
            item.locale.as_str()
        });
    ResolvedTranslationRecord {
        translation: resolved.item,
        effective_locale: resolved.effective_locale,
    }
}

pub(super) fn resolve_body_record<'a>(
    bodies: &'a [page_body::Model],
    requested: &str,
    fallback_locale: Option<&str>,
) -> ResolvedBodyRecord<'a> {
    let resolved = resolve_by_locale_with_fallback(bodies, requested, fallback_locale, |item| {
        item.locale.as_str()
    });
    ResolvedBodyRecord {
        body: resolved.item,
        effective_locale: resolved.effective_locale,
    }
}

pub(super) fn collect_builder_project_values(
    existing_bodies: &[page_body::Model],
    candidate: Option<&PreparedPageBody>,
    include_existing: bool,
) -> PagesResult<Vec<serde_json::Value>> {
    collect_builder_sources(existing_bodies, candidate, include_existing)
        .into_iter()
        .map(|(locale, content)| {
            serde_json::from_str(&content).map_err(|error| {
                PagesError::validation(format!(
                    "Page Builder project for locale `{locale}` is not valid JSON: {error}"
                ))
            })
        })
        .collect()
}

pub(super) fn collect_builder_sources(
    existing_bodies: &[page_body::Model],
    candidate: Option<&PreparedPageBody>,
    include_existing: bool,
) -> BTreeMap<String, String> {
    let mut sources = BTreeMap::<String, String>::new();
    if include_existing {
        for body in existing_bodies {
            if body.format == CONTENT_FORMAT_GRAPESJS {
                sources.insert(body.locale.clone(), body.content.clone());
            }
        }
    }
    if let Some(candidate) = candidate {
        if candidate.format == CONTENT_FORMAT_GRAPESJS {
            sources.insert(candidate.locale.clone(), candidate.content.clone());
        } else {
            sources.remove(&candidate.locale);
        }
    }
    sources
}

pub(super) fn compile_builder_sources(
    existing_bodies: &[page_body::Model],
    candidate: Option<&PreparedPageBody>,
    include_existing: bool,
) -> PagesResult<Vec<CompiledLandingArtifact>> {
    collect_builder_sources(existing_bodies, candidate, include_existing)
        .into_iter()
        .map(|(locale, content)| PageBuilderArtifactService::compile_source(&locale, &content))
        .collect()
}

pub(super) fn enforce_expected_version(expected: Option<i32>, actual: i32) -> PagesResult<()> {
    if let Some(expected_version) = expected {
        if expected_version != actual {
            return Err(PagesError::VersionConflict {
                expected_version,
                actual_version: actual,
            });
        }
    }
    Ok(())
}

pub(super) fn apply_transition(
    active: &mut page::ActiveModel,
    transition: Option<PageTransition>,
    now: chrono::DateTime<Utc>,
) {
    let Some(transition) = transition else {
        return;
    };
    active.status = Set(status_to_storage(&transition.status()).to_string());
    match transition {
        PageTransition::Publish => {
            active.published_at = Set(Some(now.into()));
            active.archived_at = Set(None);
        }
        PageTransition::Unpublish => {
            active.published_at = Set(None);
            active.archived_at = Set(None);
        }
        PageTransition::Archive => {
            active.published_at = Set(None);
            active.archived_at = Set(Some(now.into()));
        }
    }
}

pub(super) fn transition_event(
    transition: Option<PageTransition>,
    page_id: Uuid,
) -> Option<DomainEvent> {
    match transition {
        Some(PageTransition::Publish) => Some(DomainEvent::NodePublished {
            node_id: page_id,
            kind: super::PAGE_KIND.to_string(),
        }),
        Some(PageTransition::Unpublish) => Some(DomainEvent::NodeUnpublished {
            node_id: page_id,
            kind: super::PAGE_KIND.to_string(),
        }),
        Some(PageTransition::Archive) | None => None,
    }
}

pub(super) fn storage_to_status(status: &str) -> PagesResult<ContentStatus> {
    Ok(match status {
        "draft" => ContentStatus::Draft,
        "published" => ContentStatus::Published,
        "archived" => ContentStatus::Archived,
        other => {
            return Err(PagesError::validation(format!(
                "Unknown page status: {other}"
            )))
        }
    })
}

pub(super) fn status_to_storage(status: &ContentStatus) -> &'static str {
    match status {
        ContentStatus::Draft => "draft",
        ContentStatus::Published => "published",
        ContentStatus::Archived => "archived",
    }
}

pub(super) fn build_page_metadata(
    template: &str,
    translations: &[PageTranslationInput],
    existing: Option<&serde_json::Value>,
) -> serde_json::Value {
    let mut metadata = existing
        .cloned()
        .filter(|value| value.is_object())
        .unwrap_or_else(|| serde_json::json!({}));
    metadata["template"] = serde_json::json!(template);

    let mut seo = serde_json::Map::new();
    for translation in translations {
        if translation.meta_title.is_some() || translation.meta_description.is_some() {
            seo.insert(
                translation.locale.clone(),
                serde_json::json!({
                    "meta_title": translation.meta_title,
                    "meta_description": translation.meta_description,
                }),
            );
        }
    }
    if !seo.is_empty() {
        metadata["seo"] = serde_json::Value::Object(seo);
    } else if let Some(existing) = existing.and_then(|value| value.get("seo")) {
        metadata["seo"] = existing.clone();
    }

    metadata
}

pub(crate) fn is_page_visible_for_channel(
    channel_slugs: &[String],
    channel_slug: Option<&str>,
) -> bool {
    if channel_slugs.is_empty() {
        return true;
    }
    let Some(channel_slug) = channel_slug else {
        return false;
    };
    let normalized = channel_slug.trim().to_ascii_lowercase();
    !normalized.is_empty() && channel_slugs.iter().any(|item| item == &normalized)
}

pub(super) fn normalize_channel_slugs(channel_slugs: &[String]) -> Vec<String> {
    let mut normalized = channel_slugs
        .iter()
        .map(|item| item.trim().to_ascii_lowercase())
        .filter(|item| !item.is_empty())
        .collect::<Vec<_>>();
    normalized.sort();
    normalized.dedup();
    normalized
}

pub(super) fn apply_public_page_channel_filter(
    select: Select<page::Entity>,
    tenant_id: Uuid,
    channel_slug: Option<&str>,
) -> Select<page::Entity> {
    let unrestricted = Expr::col((page::Entity, page::Column::Id))
        .not_in_subquery(all_page_channel_visibility_subquery(tenant_id));
    let condition = match normalize_public_channel_slug(channel_slug) {
        Some(channel_slug) => Condition::any().add(unrestricted).add(
            Expr::col((page::Entity, page::Column::Id)).in_subquery(
                matching_page_channel_visibility_subquery(tenant_id, &channel_slug),
            ),
        ),
        None => Condition::all().add(unrestricted),
    };

    select.filter(condition)
}

fn all_page_channel_visibility_subquery(tenant_id: Uuid) -> SelectStatement {
    Query::select()
        .column(page_channel_visibility::Column::PageId)
        .from(page_channel_visibility::Entity)
        .and_where(
            Expr::col((
                page_channel_visibility::Entity,
                page_channel_visibility::Column::TenantId,
            ))
            .eq(tenant_id),
        )
        .to_owned()
}

fn matching_page_channel_visibility_subquery(
    tenant_id: Uuid,
    channel_slug: &str,
) -> SelectStatement {
    Query::select()
        .column(page_channel_visibility::Column::PageId)
        .from(page_channel_visibility::Entity)
        .and_where(
            Expr::col((
                page_channel_visibility::Entity,
                page_channel_visibility::Column::TenantId,
            ))
            .eq(tenant_id),
        )
        .and_where(
            Expr::col((
                page_channel_visibility::Entity,
                page_channel_visibility::Column::ChannelSlug,
            ))
            .eq(channel_slug),
        )
        .to_owned()
}

fn normalize_public_channel_slug(channel_slug: Option<&str>) -> Option<String> {
    channel_slug
        .map(str::trim)
        .filter(|slug| !slug.is_empty())
        .map(|slug| slug.to_ascii_lowercase())
}

pub(super) fn page_translation_response(
    translation: &page_translation::Model,
) -> PageTranslationResponse {
    PageTranslationResponse {
        locale: translation.locale.clone(),
        title: Some(translation.title.clone()),
        slug: Some(translation.slug.clone()),
        meta_title: translation.meta_title.clone(),
        meta_description: translation.meta_description.clone(),
    }
}

pub(super) fn page_body_response(body: &page_body::Model) -> PageBodyResponse {
    let content_json =
        if body.format == CONTENT_FORMAT_RT_JSON_V1 || body.format == CONTENT_FORMAT_GRAPESJS {
            serde_json::from_str(&body.content).ok()
        } else {
            None
        };
    PageBodyResponse {
        locale: body.locale.clone(),
        content: body.content.clone(),
        format: body.format.clone(),
        content_json,
        updated_at: body.updated_at.to_string(),
    }
}

fn body_requires_json_payload(format: &str) -> bool {
    matches!(format, CONTENT_FORMAT_RT_JSON_V1 | CONTENT_FORMAT_GRAPESJS)
}

pub(super) fn available_locales(translations: &[page_translation::Model]) -> Vec<String> {
    available_locales_from(translations, |item| item.locale.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channels_are_normalized_and_deduplicated() {
        assert_eq!(
            normalize_channel_slugs(&[
                " Web ".to_string(),
                "mobile".to_string(),
                "web".to_string(),
            ]),
            vec!["mobile".to_string(), "web".to_string()]
        );
    }

    #[test]
    fn page_visibility_respects_channel_allowlist() {
        let channel_slugs = vec!["web".to_string()];
        assert!(is_page_visible_for_channel(&channel_slugs, Some("web")));
        assert!(!is_page_visible_for_channel(&channel_slugs, Some("blog")));
        assert!(!is_page_visible_for_channel(&channel_slugs, None));
    }

    #[test]
    fn expected_version_fails_closed_on_stale_writes() {
        assert!(enforce_expected_version(None, 4).is_ok());
        assert!(enforce_expected_version(Some(4), 4).is_ok());
        assert!(matches!(
            enforce_expected_version(Some(3), 4),
            Err(PagesError::VersionConflict {
                expected_version: 3,
                actual_version: 4,
            })
        ));
    }

    #[test]
    fn builder_flags_default_to_enabled() {
        let settings = serde_json::json!({});
        assert!(is_builder_enabled(&settings));
        assert!(is_builder_publish_enabled(&settings));
        assert!(is_builder_preview_enabled(&settings));
        assert!(is_builder_properties_enabled(&settings));
    }

    #[test]
    fn builder_body_locale_is_normalized_before_source_collection() {
        let prepared = normalize_page_body_input(Some(PageBodyInput {
            locale: " EN ".to_string(),
            content: String::new(),
            format: Some(CONTENT_FORMAT_GRAPESJS.to_string()),
            content_json: Some(serde_json::json!({})),
        }))
        .expect("valid builder body")
        .expect("prepared body");

        assert_eq!(prepared.locale, "en");
        let sources = collect_builder_sources(&[], Some(&prepared), false);
        assert_eq!(sources.keys().cloned().collect::<Vec<_>>(), vec!["en"]);
    }
}
