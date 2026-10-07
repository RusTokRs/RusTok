use std::collections::BTreeMap;
use anyhow::Result as AnyResult;
use async_trait::async_trait;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use uuid::Uuid;

use rustok_seo_targets::{
    SeoBulkSummaryRecord, SeoLoadedTargetRecord, SeoRouteMatchRecord, SeoSitemapCandidateRecord,
    SeoTargetAlternateRoute, SeoTargetBulkListRequest, SeoTargetCapabilities, SeoTargetLoadRequest,
    SeoTargetOpenGraphRecord, SeoTargetProvider, SeoTargetRouteResolveRequest,
    SeoTargetRuntimeContext, SeoTargetSitemapRequest, SeoTargetSlug, SeoTemplateFieldMap,
    builtin_slug, schema,
};

use crate::dto::TaxonomyTermKind;
use crate::entities::{taxonomy_term, taxonomy_term_translation};
use crate::owner_category_read::TaxonomyOwnerCategoryReader;
use crate::TaxonomyScopeType;

#[derive(Clone, Default)]
pub struct TaxonomyCategorySeoTargetProvider;

#[async_trait]
impl SeoTargetProvider for TaxonomyCategorySeoTargetProvider {
    fn slug(&self) -> SeoTargetSlug {
        SeoTargetSlug::new(builtin_slug::CATEGORY)
            .expect("builtin category SEO target slug must stay valid")
    }

    fn display_name(&self) -> &'static str {
        "Category"
    }

    fn owner_module_slug(&self) -> &'static str {
        "taxonomy"
    }

    fn capabilities(&self) -> SeoTargetCapabilities {
        SeoTargetCapabilities::new(true, true, true, true)
    }

    async fn load_target(
        &self,
        runtime: &SeoTargetRuntimeContext,
        request: SeoTargetLoadRequest<'_>,
    ) -> AnyResult<Option<SeoLoadedTargetRecord>> {
        let terms = taxonomy_term::Entity::find()
            .filter(taxonomy_term::Column::TenantId.eq(request.tenant_id))
            .filter(taxonomy_term::Column::Id.eq(request.target_id))
            .filter(taxonomy_term::Column::Kind.eq(TaxonomyTermKind::Category))
            .all(&runtime.db)
            .await?;

        let Some(term) = terms.into_iter().next() else {
            return Ok(None);
        };

        let categories = TaxonomyOwnerCategoryReader::load_scoped_categories_in(
            &runtime.db,
            request.tenant_id,
            term.scope_type,
            Some(term.scope_value.as_str()),
            Some(&[request.target_id]),
            request.locale,
            Some(request.default_locale),
        )
        .await?;

        let Some(category) = categories.into_iter().next() else {
            return Ok(None);
        };

        let route = format!("/categories/{}", category.slug);

        // Load all localized slugs to provide exact hreflang alternate routes
        let all_translations = taxonomy_term_translation::Entity::find()
            .filter(taxonomy_term_translation::Column::TenantId.eq(request.tenant_id))
            .filter(taxonomy_term_translation::Column::TermId.eq(category.id))
            .all(&runtime.db)
            .await?;

        let alternates = if !all_translations.is_empty() {
            all_translations
                .into_iter()
                .map(|translation| SeoTargetAlternateRoute {
                    locale: translation.locale,
                    route: format!("/categories/{}", translation.slug),
                })
                .collect()
        } else {
            category
                .available_locales
                .iter()
                .map(|locale| SeoTargetAlternateRoute {
                    locale: locale.clone(),
                    route: route.clone(),
                })
                .collect()
        };

        let mut template_fields = SeoTemplateFieldMap::default();
        template_fields.insert("title", category.name.as_str());
        template_fields.insert("slug", category.slug.as_str());
        template_fields.insert("canonical_key", category.canonical_key.as_str());
        if let Some(description) = category.description.as_deref() {
            template_fields.insert("description", description);
        }
        template_fields.insert("route", route.as_str());
        template_fields.insert("locale", category.effective_locale.as_str());

        let open_graph = SeoTargetOpenGraphRecord {
            title: Some(category.name.clone()),
            description: category.description.clone(),
            kind: Some("website".to_string()),
            site_name: None,
            url: Some(route.clone()),
            locale: Some(category.effective_locale.clone()),
            images: Vec::new(),
        };

        let structured_data = schema::collection_page(
            category.name.as_str(),
            category.description.as_deref(),
            category.effective_locale.as_str(),
        );

        Ok(Some(SeoLoadedTargetRecord {
            target_kind: self.slug(),
            target_id: category.id,
            requested_locale: Some(request.locale.to_string()),
            effective_locale: category.effective_locale,
            title: category.name,
            description: category.description,
            canonical_route: route,
            alternates,
            open_graph,
            structured_data,
            fallback_source: "taxonomy".to_string(),
            template_fields,
        }))
    }

    async fn resolve_route(
        &self,
        runtime: &SeoTargetRuntimeContext,
        request: SeoTargetRouteResolveRequest<'_>,
    ) -> AnyResult<Option<SeoRouteMatchRecord>> {
        let route = request.route.trim();
        let path = route
            .split_once('?')
            .map_or(route, |(p, _)| p)
            .split_once('#')
            .map_or(route, |(p, _)| p);
        let slug = path
            .strip_prefix("/categories/")
            .or_else(|| path.strip_prefix("/category/"))
            .or_else(|| path.strip_prefix("categories/"))
            .or_else(|| path.strip_prefix("category/"))
            .unwrap_or(path)
            .trim_matches('/');

        if slug.is_empty() {
            return Ok(None);
        }

        let matched = taxonomy_term_translation::Entity::find()
            .filter(taxonomy_term_translation::Column::TenantId.eq(request.tenant_id))
            .filter(taxonomy_term_translation::Column::Slug.eq(slug))
            .inner_join(taxonomy_term::Entity)
            .filter(taxonomy_term::Column::Kind.eq(TaxonomyTermKind::Category))
            .one(&runtime.db)
            .await?;

        if let Some(translation) = matched {
            return Ok(Some(SeoRouteMatchRecord {
                target_kind: self.slug(),
                target_id: translation.term_id,
            }));
        }

        Ok(None)
    }

    async fn list_bulk_summaries(
        &self,
        runtime: &SeoTargetRuntimeContext,
        request: SeoTargetBulkListRequest<'_>,
    ) -> AnyResult<Vec<SeoBulkSummaryRecord>> {
        let terms = taxonomy_term::Entity::find()
            .filter(taxonomy_term::Column::TenantId.eq(request.tenant_id))
            .filter(taxonomy_term::Column::Kind.eq(TaxonomyTermKind::Category))
            .order_by_asc(taxonomy_term::Column::CanonicalKey)
            .all(&runtime.db)
            .await?;

        let mut terms_by_scope: BTreeMap<(TaxonomyScopeType, String), Vec<Uuid>> = BTreeMap::new();
        for term in terms {
            terms_by_scope
                .entry((term.scope_type, term.scope_value))
                .or_default()
                .push(term.id);
        }

        let mut summaries = Vec::new();
        for ((scope_type, scope_value), ids) in terms_by_scope {
            let categories = TaxonomyOwnerCategoryReader::load_scoped_categories_in(
                &runtime.db,
                request.tenant_id,
                scope_type,
                Some(scope_value.as_str()),
                Some(ids.as_slice()),
                request.locale,
                Some(request.default_locale),
            )
            .await?;

            for category in categories {
                summaries.push(SeoBulkSummaryRecord {
                    target_kind: self.slug(),
                    target_id: category.id,
                    effective_locale: category.effective_locale,
                    label: category.name,
                    route: format!("/categories/{}", category.slug),
                });
            }
        }

        Ok(summaries)
    }

    async fn sitemap_candidates(
        &self,
        runtime: &SeoTargetRuntimeContext,
        request: SeoTargetSitemapRequest<'_>,
    ) -> AnyResult<Vec<SeoSitemapCandidateRecord>> {
        let terms = taxonomy_term::Entity::find()
            .filter(taxonomy_term::Column::TenantId.eq(request.tenant_id))
            .filter(taxonomy_term::Column::Kind.eq(TaxonomyTermKind::Category))
            .order_by_asc(taxonomy_term::Column::CanonicalKey)
            .all(&runtime.db)
            .await?;

        let term_ids: Vec<Uuid> = terms.iter().map(|term| term.id).collect();
        let all_translations = if !term_ids.is_empty() {
            taxonomy_term_translation::Entity::find()
                .filter(taxonomy_term_translation::Column::TenantId.eq(request.tenant_id))
                .filter(taxonomy_term_translation::Column::TermId.is_in(term_ids))
                .all(&runtime.db)
                .await?
        } else {
            Vec::new()
        };

        let mut translations_by_term: BTreeMap<Uuid, Vec<taxonomy_term_translation::Model>> =
            BTreeMap::new();
        for translation in all_translations {
            translations_by_term
                .entry(translation.term_id)
                .or_default()
                .push(translation);
        }

        let mut terms_by_scope: BTreeMap<(TaxonomyScopeType, String), Vec<Uuid>> = BTreeMap::new();
        for term in terms {
            terms_by_scope
                .entry((term.scope_type, term.scope_value))
                .or_default()
                .push(term.id);
        }

        let mut candidates = Vec::new();
        for ((scope_type, scope_value), ids) in terms_by_scope {
            let categories = TaxonomyOwnerCategoryReader::load_scoped_categories_in(
                &runtime.db,
                request.tenant_id,
                scope_type,
                Some(scope_value.as_str()),
                Some(ids.as_slice()),
                request.default_locale,
                Some(request.default_locale),
            )
            .await?;

            for category in categories {
                let route = format!("/categories/{}", category.slug);
                let alternates = if let Some(trans_list) = translations_by_term.get(&category.id) {
                    trans_list
                        .iter()
                        .map(|trans| SeoTargetAlternateRoute {
                            locale: trans.locale.clone(),
                            route: format!("/categories/{}", trans.slug),
                        })
                        .collect()
                } else {
                    category
                        .available_locales
                        .iter()
                        .map(|locale| SeoTargetAlternateRoute {
                            locale: locale.clone(),
                            route: route.clone(),
                        })
                        .collect()
                };

                candidates.push(SeoSitemapCandidateRecord {
                    target_kind: self.slug(),
                    target_id: category.id,
                    locale: category.effective_locale,
                    route,
                    images: Vec::new(),
                    alternates,
                });
            }
        }

        Ok(candidates)
    }
}
