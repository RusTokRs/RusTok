use std::collections::HashMap;

use rustok_api::TenantContext;
use rustok_content::resolve_by_locale_with_fallback;
use rustok_seo_targets::{SeoTargetBulkListRequest, SeoTargetSlug};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use uuid::Uuid;

use crate::dto::{SeoBulkItem, SeoBulkListInput, SeoBulkPage, SeoBulkSource, SeoModuleSettings};
use crate::entities::{self as seo_meta, meta_translation};
use crate::{SeoError, SeoResult};

use super::super::templates::render_generated_record;
use super::super::{trimmed_option, LoadedMeta, SeoService, TargetState};

const MAX_BULK_PAGE_SIZE: i32 = 100;
const BULK_META_BATCH_SIZE: usize = 256;

#[derive(Debug, Clone)]
struct BatchedBulkListFilter {
    target_kind: SeoTargetSlug,
    locale: String,
    query: Option<String>,
    source: SeoBulkSource,
    page: i32,
    per_page: i32,
}

#[derive(Debug, Clone)]
struct BatchedBulkSummary {
    target_id: Uuid,
    label: String,
    route: String,
}

#[derive(Debug, Clone)]
struct BatchedBulkMeta {
    effective_locale: String,
    source: SeoBulkSource,
    title: Option<String>,
    description: Option<String>,
    canonical_url: Option<String>,
    noindex: bool,
    nofollow: bool,
}

#[derive(Debug, Clone)]
struct BatchedBulkRow {
    summary: BatchedBulkSummary,
    meta: BatchedBulkMeta,
}

impl SeoService {
    pub(super) async fn list_bulk_items_batched(
        &self,
        tenant: &TenantContext,
        input: SeoBulkListInput,
    ) -> SeoResult<SeoBulkPage> {
        let filter = normalize_batched_bulk_list_input(input, tenant.default_locale.as_str())?;
        if !self.is_enabled(tenant.id).await? {
            return Ok(empty_bulk_page(&filter));
        }

        let Some(provider) = self.registry.get(&filter.target_kind) else {
            return Ok(empty_bulk_page(&filter));
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
            })?
            .into_iter()
            .map(|summary| BatchedBulkSummary {
                target_id: summary.target_id,
                label: summary.label,
                route: summary.route,
            })
            .collect::<Vec<_>>();

        let target_ids = summaries
            .iter()
            .map(|summary| summary.target_id)
            .collect::<Vec<_>>();
        let mut explicit_by_target = self
            .load_bulk_explicit_meta_batches(
                tenant.id,
                filter.target_kind.as_str(),
                target_ids.as_slice(),
            )
            .await?;
        let settings = self.load_settings(tenant.id).await?;
        let mut rows = Vec::with_capacity(summaries.len());

        for summary in summaries {
            let state = self
                .load_target_state(
                    tenant,
                    filter.target_kind.clone(),
                    summary.target_id,
                    filter.locale.as_str(),
                )
                .await?;
            let explicit = explicit_by_target.remove(&summary.target_id);
            let meta = resolve_batched_bulk_meta(
                tenant,
                filter.target_kind.clone(),
                summary.target_id,
                filter.locale.as_str(),
                explicit,
                state,
                &settings,
            )
            .ok_or(SeoError::NotFound)?;

            if filter.source != SeoBulkSource::Any && filter.source != meta.source {
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
            rows.push(BatchedBulkRow { summary, meta });
        }

        let total = rows.len() as i32;
        let offset = ((filter.page - 1) * filter.per_page) as usize;
        let items = rows
            .into_iter()
            .skip(offset)
            .take(filter.per_page as usize)
            .map(|row| SeoBulkItem {
                target_kind: filter.target_kind.clone(),
                target_id: row.summary.target_id,
                locale: filter.locale.clone(),
                effective_locale: row.meta.effective_locale,
                label: row.summary.label,
                route: row.summary.route,
                source: row.meta.source,
                title: row.meta.title,
                description: row.meta.description,
                canonical_url: row.meta.canonical_url,
                noindex: row.meta.noindex,
                nofollow: row.meta.nofollow,
            })
            .collect();

        Ok(SeoBulkPage {
            items,
            total,
            page: filter.page,
            per_page: filter.per_page,
        })
    }

    async fn load_bulk_explicit_meta_batches(
        &self,
        tenant_id: Uuid,
        target_kind: &str,
        target_ids: &[Uuid],
    ) -> SeoResult<HashMap<Uuid, LoadedMeta>> {
        let mut loaded = HashMap::new();
        for target_ids in target_ids.chunks(BULK_META_BATCH_SIZE) {
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

fn normalize_batched_bulk_list_input(
    input: SeoBulkListInput,
    fallback_locale: &str,
) -> SeoResult<BatchedBulkListFilter> {
    Ok(BatchedBulkListFilter {
        target_kind: input.target_kind,
        locale: super::super::normalize_effective_locale(input.locale.as_str(), fallback_locale)?,
        query: input
            .query
            .map(|value| value.trim().to_ascii_lowercase())
            .filter(|value| !value.is_empty()),
        source: input.source.unwrap_or(SeoBulkSource::Any),
        page: input.page.max(1),
        per_page: input.per_page.clamp(1, MAX_BULK_PAGE_SIZE),
    })
}

fn empty_bulk_page(filter: &BatchedBulkListFilter) -> SeoBulkPage {
    SeoBulkPage {
        items: Vec::new(),
        total: 0,
        page: filter.page,
        per_page: filter.per_page,
    }
}

#[allow(clippy::too_many_arguments)]
fn resolve_batched_bulk_meta(
    tenant: &TenantContext,
    target_kind: SeoTargetSlug,
    target_id: Uuid,
    requested_locale: &str,
    explicit: Option<LoadedMeta>,
    state: Option<TargetState>,
    settings: &SeoModuleSettings,
) -> Option<BatchedBulkMeta> {
    match (explicit, state) {
        (Some(explicit), Some(state)) => {
            let resolved = resolve_by_locale_with_fallback(
                explicit.translations.as_slice(),
                state.effective_locale.as_str(),
                Some(tenant.default_locale.as_str()),
                |item| item.locale.as_str(),
            );
            let translation = resolved.item.cloned();
            Some(BatchedBulkMeta {
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
                canonical_url: explicit.meta.canonical_url,
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
            Some(BatchedBulkMeta {
                effective_locale: resolved.effective_locale,
                source: SeoBulkSource::Explicit,
                title: translation.as_ref().and_then(|item| item.title.clone()),
                description: translation
                    .as_ref()
                    .and_then(|item| item.description.clone()),
                canonical_url: explicit.meta.canonical_url,
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
            Some(BatchedBulkMeta {
                effective_locale: state.effective_locale,
                source: if generated_source {
                    SeoBulkSource::Generated
                } else {
                    SeoBulkSource::Fallback
                },
                title: generated.title.or(Some(state.title)),
                description: generated.description.or(state.description),
                canonical_url: generated.canonical_url,
                noindex: false,
                nofollow: false,
            })
        }
        (None, None) => None,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::dto::{SeoAlternateLink, SeoOpenGraph};

    fn tenant() -> TenantContext {
        TenantContext {
            id: Uuid::new_v4(),
            name: "Tenant".to_string(),
            slug: "tenant".to_string(),
            domain: None,
            settings: json!({}),
            default_locale: "en-US".to_string(),
            is_active: true,
        }
    }

    fn state(target_kind: SeoTargetSlug, target_id: Uuid) -> TargetState {
        TargetState {
            target_kind,
            target_id,
            requested_locale: Some("en-US".to_string()),
            effective_locale: "en-US".to_string(),
            title: "Fallback title".to_string(),
            description: Some("Fallback description".to_string()),
            canonical_path: "/fallback".to_string(),
            alternates: vec![SeoAlternateLink {
                locale: "en-US".to_string(),
                href: "/en-US/fallback".to_string(),
                x_default: false,
            }],
            open_graph: SeoOpenGraph::default(),
            structured_data: json!({"@type": "WebPage"}),
            fallback_source: "target".to_string(),
            template_fields: Default::default(),
        }
    }

    fn explicit(target_kind: &SeoTargetSlug, target_id: Uuid) -> LoadedMeta {
        let meta_id = Uuid::new_v4();
        LoadedMeta {
            meta: seo_meta::Model {
                id: meta_id,
                tenant_id: tenant().id,
                target_type: target_kind.as_str().to_string(),
                target_id,
                no_index: true,
                no_follow: false,
                canonical_url: Some("/explicit".to_string()),
                structured_data: Some(json!({"@type": "WebPage"})),
            },
            translations: vec![meta_translation::Model {
                id: Uuid::new_v4(),
                meta_id,
                locale: "en-US".to_string(),
                title: Some("Explicit title".to_string()),
                description: Some("Explicit description".to_string()),
                keywords: None,
                og_title: None,
                og_description: None,
                og_image: None,
            }],
        }
    }

    #[test]
    fn explicit_projection_preserves_explicit_fields() {
        let target_kind = SeoTargetSlug::new("page").expect("valid target kind");
        let target_id = Uuid::new_v4();
        let resolved = resolve_batched_bulk_meta(
            &tenant(),
            target_kind.clone(),
            target_id,
            "en-US",
            Some(explicit(&target_kind, target_id)),
            Some(state(target_kind, target_id)),
            &SeoModuleSettings::default(),
        )
        .expect("projection");

        assert_eq!(resolved.source, SeoBulkSource::Explicit);
        assert_eq!(resolved.title.as_deref(), Some("Explicit title"));
        assert_eq!(resolved.canonical_url.as_deref(), Some("/explicit"));
        assert!(resolved.noindex);
    }

    #[test]
    fn fallback_projection_uses_target_fields_without_templates() {
        let target_kind = SeoTargetSlug::new("page").expect("valid target kind");
        let target_id = Uuid::new_v4();
        let resolved = resolve_batched_bulk_meta(
            &tenant(),
            target_kind.clone(),
            target_id,
            "en-US",
            None,
            Some(state(target_kind, target_id)),
            &SeoModuleSettings::default(),
        )
        .expect("projection");

        assert_eq!(resolved.source, SeoBulkSource::Fallback);
        assert_eq!(resolved.title.as_deref(), Some("Fallback title"));
        assert_eq!(resolved.description.as_deref(), Some("Fallback description"));
        assert!(!resolved.noindex);
    }

    #[test]
    fn normalization_bounds_page_and_canonicalizes_query() {
        let filter = normalize_batched_bulk_list_input(
            SeoBulkListInput {
                target_kind: SeoTargetSlug::new("page").expect("valid target kind"),
                locale: "en-us".to_string(),
                query: Some("  Sale  ".to_string()),
                source: None,
                page: 0,
                per_page: 999,
            },
            "en-US",
        )
        .expect("filter");

        assert_eq!(filter.locale, "en-US");
        assert_eq!(filter.query.as_deref(), Some("sale"));
        assert_eq!(filter.page, 1);
        assert_eq!(filter.per_page, MAX_BULK_PAGE_SIZE);
    }
}
