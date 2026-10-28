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
    page, page_body, page_channel_visibility, page_static_landing_artifact, page_translation,
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

        // 3. Load static landing artifacts
        let artifacts = page_static_landing_artifact::Entity::find()
            .filter(page_static_landing_artifact::Column::TenantId.eq(tenant_id))
            .filter(page_static_landing_artifact::Column::PageId.is_in(page_ids.clone()))
            .all(&self.db)
            .await
            .map_err(Error::Database)?;

        let mut artifacts_by_page_locale: HashMap<(Uuid, String), String> = HashMap::new();
        for artifact in artifacts {
            if !artifact.body_html.trim().is_empty() {
                artifacts_by_page_locale
                    .insert((artifact.page_id, artifact.locale), artifact.body_html);
            }
        }

        // 4. Load page bodies (fallback if no static landing artifact)
        let bodies = page_body::Entity::find()
            .filter(page_body::Column::TenantId.eq(tenant_id))
            .filter(page_body::Column::PageId.is_in(page_ids.clone()))
            .all(&self.db)
            .await
            .map_err(Error::Database)?;

        let mut bodies_by_page_locale: HashMap<(Uuid, String), (String, String)> = HashMap::new();
        for body in bodies {
            bodies_by_page_locale.insert((body.page_id, body.locale), (body.format, body.content));
        }

        let mut documents = Vec::new();

        for page in pages {
            if is_no_index(&page.metadata) {
                continue;
            }

            let Some(page_translations) = translations_by_page.get(&page.id) else {
                continue;
            };

            let page_channels = channels_by_page.get(&page.id).cloned().unwrap_or_default();

            for translation in page_translations {
                let locale = &translation.locale;
                let artifact_html = artifacts_by_page_locale
                    .get(&(page.id, locale.clone()))
                    .map(String::as_str);
                let body_content = bodies_by_page_locale
                    .get(&(page.id, locale.clone()))
                    .map(|(fmt, cnt)| (fmt.as_str(), cnt.as_str()));

                let body = extract_page_plain_text(artifact_html, body_content);

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

pub fn extract_page_plain_text(
    artifact_html: Option<&str>,
    body_content: Option<(&str, &str)>,
) -> String {
    if let Some(html) = artifact_html {
        let stripped = strip_html_tags(html);
        if !stripped.is_empty() {
            return stripped;
        }
    }

    let Some((format, content)) = body_content else {
        return String::new();
    };

    if format == "fly_artifact_payload"
        && let Ok(val) = serde_json::from_str::<Value>(content)
        && let Some(html) = val.get("html").and_then(Value::as_str)
    {
        return strip_html_tags(html);
    }

    if (format == "page_builder" || format == "grapesjs")
        && let Ok(val) = serde_json::from_str::<Value>(content)
    {
        let mut strings = Vec::new();
        collect_text_from_json(&val, &mut strings, 0);
        if !strings.is_empty() {
            return strings.join(" ");
        }
    }

    strip_html_tags(content)
}

fn collect_text_from_json(val: &Value, out: &mut Vec<String>, depth: usize) {
    if depth > 32 || out.len() > 200 {
        return;
    }
    match val {
        Value::Object(map) => {
            if let Some(content) = map.get("content").and_then(Value::as_str) {
                let stripped = strip_html_tags(content);
                if !stripped.is_empty() {
                    out.push(stripped);
                }
            }
            for (key, child) in map {
                if key != "style" && key != "attributes" && key != "css" {
                    collect_text_from_json(child, out, depth + 1);
                }
            }
        }
        Value::Array(arr) => {
            for item in arr {
                collect_text_from_json(item, out, depth + 1);
            }
        }
        _ => {}
    }
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
    fn extract_page_plain_text_prefers_artifact_html() {
        let html = "<h1>Landing Title</h1><p>Landing description.</p>";
        let body = ("page_builder", "{\"content\":\"raw content\"}");
        let text = extract_page_plain_text(Some(html), Some(body));
        assert_eq!(text, "Landing Title Landing description.");
    }

    #[test]
    fn extract_page_plain_text_extracts_from_grapesjs_json() {
        let json_body = r#"{
            "pages": [{
                "component": {
                    "type": "wrapper",
                    "components": [
                        { "type": "header", "content": "<h1>Header Title</h1>" },
                        { "type": "paragraph", "content": "<p>Some text content.</p>" }
                    ]
                }
            }]
        }"#;
        let text = extract_page_plain_text(None, Some(("page_builder", json_body)));
        assert_eq!(text, "Header Title Some text content.");
    }

    #[test]
    fn is_no_index_detects_snake_case_and_camel_case() {
        assert!(is_no_index(&json!({ "no_index": true })));
        assert!(is_no_index(&json!({ "noIndex": true })));
        assert!(!is_no_index(&json!({ "no_index": false })));
        assert!(!is_no_index(&json!({})));
    }
}
