use std::collections::{BTreeMap, BTreeSet};

use chrono::Utc;
use sea_orm::{
    ActiveValue::Set,
    Condition, QueryFilter, Select,
    sea_query::{Expr, ExprTrait, Query, SelectStatement},
};
use uuid::Uuid;

use rustok_api::{
    PLATFORM_FALLBACK_LOCALE, build_locale_candidates, locale_tags_match, normalize_locale_tag,
};
use rustok_content::{
    available_locales_from, entities::node::ContentStatus, normalize_locale_code,
};
use rustok_events::DomainEvent;
use rustok_page_builder::{PAGE_BUILDER_DOCUMENT_FORMAT, validate_page_builder_document};

use crate::dto::{PageBodyInput, PageBodyResponse, PageTranslationInput, PageTranslationResponse};
use crate::entities::{page, page_body, page_channel_visibility, page_translation};
use crate::error::{PagesError, PagesResult};

use super::{PageTransition, PreparedPageBody, ResolvedTranslationRecord};

pub(super) fn validate_page_translations(translations: &[PageTranslationInput]) -> PagesResult<()> {
    if translations.is_empty() {
        return Err(PagesError::validation(
            "At least one page translation is required",
        ));
    }
    let mut locales = BTreeSet::new();
    for translation in translations {
        let locale = normalize_locale(&translation.locale)?;
        if !locales.insert(locale.clone()) {
            return Err(PagesError::validation(format!(
                "Duplicate normalized page locale: {locale}"
            )));
        }
        if translation.title.trim().is_empty() {
            return Err(PagesError::validation("Page title cannot be empty"));
        }
        normalize_slug(
            translation
                .slug
                .as_deref()
                .unwrap_or(translation.title.as_str()),
        )?;
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
    let document = body.document;
    validate_page_builder_document(&document).map_err(PagesError::validation)?;
    Ok(Some(PreparedPageBody {
        locale,
        content: document.to_string(),
        format: PAGE_BUILDER_DOCUMENT_FORMAT.to_string(),
    }))
}

pub(super) fn normalize_locale(locale: &str) -> PagesResult<String> {
    normalize_locale_code(locale).ok_or_else(|| PagesError::validation("Invalid locale"))
}

pub(super) fn normalize_slug(value: &str) -> PagesResult<String> {
    let mut normalized = String::with_capacity(value.len());
    let mut previous_dash = false;
    for ch in value.trim().chars().flat_map(|ch| ch.to_lowercase()) {
        if ch.is_alphanumeric() {
            normalized.push(ch);
            previous_dash = false;
        } else if !previous_dash && !normalized.is_empty() {
            normalized.push('-');
            previous_dash = true;
        }
    }
    let normalized = normalized.trim_matches('-').to_string();
    if normalized.is_empty() {
        return Err(PagesError::validation(
            "Localized page slug cannot be empty after normalization",
        ));
    }
    if normalized.chars().count() > 255 {
        return Err(PagesError::validation(
            "Localized page slug cannot exceed 255 characters",
        ));
    }
    Ok(normalized)
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
    body.is_some_and(|item| item.format == PAGE_BUILDER_DOCUMENT_FORMAT)
}

pub(super) fn resolve_translation_record<'a>(
    translations: &'a [page_translation::Model],
    requested: &str,
    fallback_locale: Option<&str>,
) -> ResolvedTranslationRecord<'a> {
    let candidates = build_locale_candidates(
        [
            Some(requested),
            fallback_locale,
            Some(PLATFORM_FALLBACK_LOCALE),
        ],
        true,
    );
    for candidate in candidates {
        if let Some(translation) = translations
            .iter()
            .find(|item| locale_tags_match(item.locale.as_str(), candidate.as_str()))
        {
            return ResolvedTranslationRecord {
                translation: Some(translation),
                effective_locale: normalize_locale_tag(translation.locale.as_str())
                    .unwrap_or_else(|| translation.locale.clone()),
            };
        }
    }
    ResolvedTranslationRecord {
        translation: None,
        effective_locale: normalize_locale_tag(requested).unwrap_or_else(|| requested.to_string()),
    }
}

pub(super) fn body_for_locale<'a>(
    bodies: &'a [page_body::Model],
    locale: &str,
) -> Option<&'a page_body::Model> {
    bodies
        .iter()
        .find(|body| locale_tags_match(body.locale.as_str(), locale))
}

pub(super) fn collect_builder_sources(
    existing_bodies: &[page_body::Model],
    candidate: Option<&PreparedPageBody>,
    include_existing: bool,
) -> BTreeMap<String, String> {
    let mut sources = BTreeMap::<String, String>::new();
    if include_existing {
        for body in existing_bodies {
            if body.format == PAGE_BUILDER_DOCUMENT_FORMAT {
                sources.insert(body.locale.clone(), body.content.clone());
            }
        }
    }
    if let Some(candidate) = candidate {
        if candidate.format == PAGE_BUILDER_DOCUMENT_FORMAT {
            sources.insert(candidate.locale.clone(), candidate.content.clone());
        } else {
            sources.remove(&candidate.locale);
        }
    }
    sources
}

pub(super) fn enforce_expected_version(expected: Option<i32>, actual: i32) -> PagesResult<()> {
    if let Some(expected_version) = expected
        && expected_version != actual
    {
        return Err(PagesError::VersionConflict {
            expected_version,
            actual_version: actual,
        });
    }
    Ok(())
}

pub(super) fn page_resource_revision(page: &page::Model) -> PagesResult<i64> {
    let revision = i64::from(page.version);
    (revision > 0)
        .then_some(revision)
        .ok_or_else(|| PagesError::version_exhausted(page.id))
}

pub(super) fn next_page_version(page_id: Uuid, revision: i32) -> PagesResult<i32> {
    revision
        .checked_add(1)
        .filter(|next_revision| revision > 0 && *next_revision > 0)
        .ok_or_else(|| PagesError::version_exhausted(page_id))
}

pub(super) fn next_page_translation_revision(
    page_id: Uuid,
    locale: &str,
    revision: i64,
) -> PagesResult<i64> {
    revision
        .checked_add(1)
        .filter(|next_revision| revision > 0 && *next_revision > 0)
        .ok_or_else(|| PagesError::translation_revision_exhausted(page_id, locale))
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
            )));
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
    existing: Option<&serde_json::Value>,
) -> serde_json::Value {
    let mut metadata = existing
        .cloned()
        .filter(|value| value.is_object())
        .unwrap_or_else(|| serde_json::json!({}));
    let object = metadata
        .as_object_mut()
        .expect("page metadata is normalized to an object");
    object.remove("seo");
    object.insert("template".to_string(), serde_json::json!(template));
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
    let content_json = if body.format == PAGE_BUILDER_DOCUMENT_FORMAT {
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
    fn localized_slug_preserves_unicode_and_rejects_empty_output() {
        assert_eq!(normalize_slug(" Дом ").expect("unicode slug"), "дом");
        assert_eq!(normalize_slug("首页").expect("CJK slug"), "首页");
        assert!(normalize_slug("---").is_err());
    }

    #[test]
    fn builder_body_locale_is_normalized_before_source_collection() {
        let prepared = normalize_page_body_input(Some(PageBodyInput {
            locale: " EN ".to_string(),
            document: serde_json::json!({}),
        }))
        .expect("valid builder body")
        .expect("prepared body");

        assert_eq!(prepared.locale, "en");
        let sources = collect_builder_sources(&[], Some(&prepared), false);
        assert_eq!(sources.keys().cloned().collect::<Vec<_>>(), vec!["en"]);
    }
}
