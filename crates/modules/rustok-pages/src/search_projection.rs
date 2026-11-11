/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use sea_orm::{
    ColumnTrait, Condition, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, QuerySelect,
};
use serde_json::{Value, json};
use uuid::Uuid;

use rustok_content::entities::node::ContentStatus;
use rustok_core::search_projection::{
    MAX_SEARCH_PROJECTION_PAGE_SIZE, SearchProjectionDocument, SearchProjectionPage,
    SearchProjectionSource, SearchProjectionSourceFactory,
};
use rustok_core::{Error, Result};

use crate::entities::{
    page, page_channel_visibility, page_published_landing_artifact, page_static_landing_artifact,
    page_translation,
};
use crate::services::page::PublishedPageCursor;
use crate::services::page::status_to_storage;

pub const PAGES_SOURCE_MODULE: &str = "pages";
pub const PAGE_ENTITY_TYPE: &str = "page";

#[derive(Clone, Default)]
pub struct PageSearchProjectionSourceFactory;

impl SearchProjectionSourceFactory for PageSearchProjectionSourceFactory {
    fn source_module(&self) -> &'static str {
        PAGES_SOURCE_MODULE
    }

    fn build(&self, db: DatabaseConnection) -> Arc<dyn SearchProjectionSource> {
        Arc::new(PageSearchProjectionSource::new(db))
    }
}

#[derive(Clone)]
pub struct PageSearchProjectionSource {
    db: DatabaseConnection,
}

impl PageSearchProjectionSource {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    async fn project_pages(
        &self,
        tenant_id: Uuid,
        pages: &[page::Model],
    ) -> Result<Vec<SearchProjectionDocument>> {
        if pages.is_empty() {
            return Ok(Vec::new());
        }

        let page_ids = pages.iter().map(|p| p.id).collect::<Vec<_>>();

        // 1. Load translations
        let translations = page_translation::Entity::find()
            .filter(page_translation::Column::TenantId.eq(tenant_id))
            .filter(page_translation::Column::PageId.is_in(page_ids.clone()))
            .all(&self.db)
            .await
            .map_err(Error::Database)?;

        let mut translations_by_page: HashMap<Uuid, Vec<page_translation::Model>> = HashMap::new();
        for translation in translations {
            translations_by_page
                .entry(translation.page_id)
                .or_default()
                .push(translation);
        }

        // 2. Load channel visibility
        let channel_visibilities = page_channel_visibility::Entity::find()
            .filter(page_channel_visibility::Column::TenantId.eq(tenant_id))
            .filter(page_channel_visibility::Column::PageId.is_in(page_ids.clone()))
            .all(&self.db)
            .await
            .map_err(Error::Database)?;

        let mut channels_by_page: HashMap<Uuid, Vec<String>> = HashMap::new();
        for vis in channel_visibilities {
            channels_by_page
                .entry(vis.page_id)
                .or_default()
                .push(vis.channel_slug);
        }

        // 3. Load the text of the published landing of every page locale
        let landing_text_by_page = self
            .load_published_landing_text(tenant_id, &page_ids)
            .await?;

        let mut documents = Vec::new();

        for page in pages {
            if is_no_index(&page.metadata) {
                continue;
            }

            let Some(page_translations) = translations_by_page.get(&page.id) else {
                continue;
            };

            // A published page without a published landing has no storefront route, so a
            // search hit for it would lead nowhere: it is not a public search document.
            let Some(landing_text_by_locale) = landing_text_by_page.get(&page.id) else {
                continue;
            };

            let page_channels = channels_by_page.get(&page.id).cloned().unwrap_or_default();

            for translation in page_translations {
                // A locale without a landing of its own is served the landing of a fallback
                // locale. That text is in another language and is indexed with its own
                // locale, so this document carries the titles of the page only.
                let body = landing_text_by_locale
                    .get(&translation.locale)
                    .cloned()
                    .unwrap_or_default();

                let keywords_text = [
                    Some(translation.title.as_str()),
                    translation.meta_title.as_deref(),
                    translation.meta_description.as_deref(),
                    Some(translation.slug.as_str()),
                    Some(page.template.as_str()),
                ]
                .into_iter()
                .flatten()
                .filter(|s| !s.trim().is_empty())
                .collect::<Vec<_>>()
                .join(" ");

                documents.push(SearchProjectionDocument {
                    document_key: format!("page:{}:{}", page.id, translation.locale),
                    tenant_id,
                    document_id: page.id,
                    source_module: PAGES_SOURCE_MODULE.to_string(),
                    entity_type: PAGE_ENTITY_TYPE.to_string(),
                    locale: translation.locale.clone(),
                    status: page.status.clone(),
                    is_public: true,
                    title: translation.title.clone(),
                    subtitle: translation.meta_title.clone(),
                    slug: Some(translation.slug.clone()),
                    handle: None,
                    body,
                    keywords_text,
                    facets: json!({
                        "template": page.template,
                        "has_channel_restrictions": !page_channels.is_empty(),
                    }),
                    payload: json!({
                        "slug": translation.slug,
                        "meta_title": translation.meta_title,
                        "meta_description": translation.meta_description,
                        "channel_visibility": {
                            "allowed_channel_slugs": page_channels,
                        },
                        "template": page.template,
                    }),
                    published_at: page.published_at.map(Into::into),
                    created_at: page.created_at.into(),
                    updated_at: page.updated_at.into(),
                });
            }
        }

        Ok(documents)
    }

    /// Visible text of the published landing of each page, by locale.
    ///
    /// The published landing of a page locale is the artifact its landing binding points at.
    /// The artifact table is not asked directly: it also keeps the landings that a later
    /// publish superseded. The page body is not read at all: it is the authoring document, not
    /// published content. A page that is absent from the result has no published landing.
    ///
    /// A binding without its artifact is skipped and logged. The artifact owner fails the
    /// public read of such a page, so it has no public content to index. Artifact hashes are
    /// not verified here; the owner verifies them on every read that renders the landing.
    async fn load_published_landing_text(
        &self,
        tenant_id: Uuid,
        page_ids: &[Uuid],
    ) -> Result<HashMap<Uuid, HashMap<String, String>>> {
        let bindings = page_published_landing_artifact::Entity::find()
            .filter(page_published_landing_artifact::Column::TenantId.eq(tenant_id))
            .filter(page_published_landing_artifact::Column::PageId.is_in(page_ids.to_vec()))
            .all(&self.db)
            .await
            .map_err(Error::Database)?;
        if bindings.is_empty() {
            return Ok(HashMap::new());
        }

        let artifact_ids = bindings
            .iter()
            .map(|binding| binding.artifact_id)
            .collect::<Vec<_>>();
        let artifacts_by_id = page_static_landing_artifact::Entity::find()
            .filter(page_static_landing_artifact::Column::TenantId.eq(tenant_id))
            .filter(page_static_landing_artifact::Column::Id.is_in(artifact_ids))
            .all(&self.db)
            .await
            .map_err(Error::Database)?
            .into_iter()
            .map(|artifact| (artifact.id, artifact))
            .collect::<HashMap<_, _>>();

        let mut text_by_page: HashMap<Uuid, HashMap<String, String>> = HashMap::new();
        for binding in bindings {
            let artifact = artifacts_by_id
                .get(&binding.artifact_id)
                .filter(|artifact| binding_matches_artifact(&binding, artifact));
            let Some(artifact) = artifact else {
                tracing::warn!(
                    tenant_id = %tenant_id,
                    page_id = %binding.page_id,
                    locale = %binding.locale,
                    artifact_id = %binding.artifact_id,
                    "published landing binding references an invalid artifact; \
                     the page locale is not indexed"
                );
                continue;
            };
            text_by_page
                .entry(binding.page_id)
                .or_default()
                .insert(binding.locale, strip_html_tags(&artifact.body_html));
        }
        Ok(text_by_page)
    }
}

/// The artifact a binding points at belongs to the same page locale as the binding. This is
/// the condition under which the artifact owner serves it.
///
/// The schema already guarantees it (`m20260721_000004`: a composite foreign key on
/// PostgreSQL, a trigger on SQLite), so with the migrations applied this never fails. It is
/// kept as a second line: without it a mismatched row would be indexed in the wrong language.
fn binding_matches_artifact(
    binding: &page_published_landing_artifact::Model,
    artifact: &page_static_landing_artifact::Model,
) -> bool {
    artifact.page_id == binding.page_id && artifact.locale == binding.locale
}

#[async_trait]
impl SearchProjectionSource for PageSearchProjectionSource {
    fn source_module(&self) -> &'static str {
        PAGES_SOURCE_MODULE
    }

    async fn list_public_documents(
        &self,
        tenant_id: Uuid,
        after: Option<String>,
        limit: usize,
    ) -> Result<SearchProjectionPage> {
        let limit = limit.clamp(1, MAX_SEARCH_PROJECTION_PAGE_SIZE) as u64;

        let mut query = page::Entity::find()
            .filter(page::Column::TenantId.eq(tenant_id))
            .filter(page::Column::Status.eq(status_to_storage(&ContentStatus::Published)));

        if let Some(cursor_str) = after.as_deref() {
            let cursor = PublishedPageCursor::decode(cursor_str)
                .map_err(|e| Error::Validation(format!("Invalid search projection cursor: {e}")))?;
            query = query.filter(
                Condition::any()
                    .add(page::Column::UpdatedAt.lt(cursor.updated_at))
                    .add(
                        Condition::all()
                            .add(page::Column::UpdatedAt.eq(cursor.updated_at))
                            .add(page::Column::Id.lt(cursor.id)),
                    ),
            );
        }

        let mut pages = query
            .order_by_desc(page::Column::UpdatedAt)
            .order_by_desc(page::Column::Id)
            .limit(limit + 1)
            .all(&self.db)
            .await
            .map_err(Error::Database)?;

        let has_next_page = pages.len() as u64 > limit;
        pages.truncate(limit as usize);

        let next_cursor = if has_next_page {
            pages
                .last()
                .map(PublishedPageCursor::from_page)
                .map(|c| c.encode())
        } else {
            None
        };

        let documents = self.project_pages(tenant_id, &pages).await?;

        Ok(SearchProjectionPage {
            documents,
            next_cursor,
        })
    }

    async fn load_public_entity(
        &self,
        tenant_id: Uuid,
        entity_type: &str,
        entity_id: Uuid,
    ) -> Result<Vec<SearchProjectionDocument>> {
        if entity_type != PAGE_ENTITY_TYPE {
            return Ok(Vec::new());
        }

        let page = page::Entity::find_by_id(entity_id)
            .filter(page::Column::TenantId.eq(tenant_id))
            .filter(page::Column::Status.eq(status_to_storage(&ContentStatus::Published)))
            .one(&self.db)
            .await
            .map_err(Error::Database)?;

        let Some(page) = page else {
            return Ok(Vec::new());
        };

        self.project_pages(tenant_id, &[page]).await
    }
}

pub fn is_no_index(metadata: &Value) -> bool {
    let Some(obj) = metadata.as_object() else {
        return false;
    };

    obj.get("no_index")
        .or_else(|| obj.get("noIndex"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

pub fn strip_html_tags(html: &str) -> String {
    let mut out = String::with_capacity(html.len() + 16);
    let mut in_tag = false;
    for ch in html.chars() {
        if ch == '<' {
            in_tag = true;
            out.push(' ');
        } else if ch == '>' {
            in_tag = false;
            out.push(' ');
        } else if !in_tag {
            out.push(ch);
        }
    }
    let collapsed = out
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'");
    collapsed.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn factory_exposes_pages_module() {
        let factory = PageSearchProjectionSourceFactory;
        assert_eq!(factory.source_module(), "pages");
    }

    #[test]
    fn strip_html_tags_removes_markup_and_normalizes_whitespace() {
        let input =
            "<p>Hello, &quot;world&quot;!</p><div>\n\n<span>Welcome &amp; enjoy.</span></div>";
        assert_eq!(strip_html_tags(input), "Hello, \"world\"! Welcome & enjoy.");
    }

    #[test]
    fn binding_matches_only_an_artifact_of_its_own_page_locale() {
        let page_id = Uuid::new_v4();
        let binding = landing_binding(page_id, "en");

        assert!(binding_matches_artifact(
            &binding,
            &landing_artifact(page_id, "en")
        ));
        assert!(!binding_matches_artifact(
            &binding,
            &landing_artifact(page_id, "ru")
        ));
        assert!(!binding_matches_artifact(
            &binding,
            &landing_artifact(Uuid::new_v4(), "en")
        ));
    }

    fn landing_binding(page_id: Uuid, locale: &str) -> page_published_landing_artifact::Model {
        page_published_landing_artifact::Model {
            page_body_id: Uuid::new_v4(),
            tenant_id: Uuid::nil(),
            page_id,
            locale: locale.to_string(),
            artifact_id: Uuid::new_v4(),
            published_at: chrono::Utc::now().into(),
        }
    }

    fn landing_artifact(page_id: Uuid, locale: &str) -> page_static_landing_artifact::Model {
        page_static_landing_artifact::Model {
            id: Uuid::new_v4(),
            tenant_id: Uuid::nil(),
            page_id,
            locale: locale.to_string(),
            source_hash: String::new(),
            build_hash: String::new(),
            artifact_hash: String::new(),
            materialization_hash: None,
            materialization_identity: None,
            runtime_snapshots: None,
            renderer_id: String::new(),
            renderer_release: String::new(),
            identity: json!({}),
            registry: json!({}),
            page_index: 0,
            fly_page_id: None,
            slug: None,
            head: json!({}),
            document_html: String::new(),
            body_html: String::new(),
            css: String::new(),
            content_hash: String::new(),
            landing_sections: json!([]),
            instance_key: String::new(),
            created_at: chrono::Utc::now().into(),
        }
    }

    #[test]
    fn is_no_index_detects_snake_case_and_camel_case() {
        assert!(is_no_index(&json!({ "no_index": true })));
        assert!(is_no_index(&json!({ "noIndex": true })));
        assert!(!is_no_index(&json!({ "no_index": false })));
        assert!(!is_no_index(&json!({})));
    }
}
