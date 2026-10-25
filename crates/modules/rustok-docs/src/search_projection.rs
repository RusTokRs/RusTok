/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sea_orm::{
    ColumnTrait, Condition, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, QuerySelect,
};
use serde_json::json;
use uuid::Uuid;

use rustok_core::search_projection::{
    MAX_SEARCH_PROJECTION_PAGE_SIZE, SearchProjectionDocument, SearchProjectionPage,
    SearchProjectionSource, SearchProjectionSourceFactory,
};
use rustok_core::{Error, Result};

use crate::entities::page;
use crate::services::DocumentService;

pub const DOCS_SOURCE_MODULE: &str = "docs";
pub const DOC_PAGE_ENTITY_TYPE: &str = "doc_page";
const DEFAULT_DOC_LOCALE: &str = "en";

#[derive(Clone, Default)]
pub struct DocsSearchProjectionSourceFactory;

impl SearchProjectionSourceFactory for DocsSearchProjectionSourceFactory {
    fn source_module(&self) -> &'static str {
        DOCS_SOURCE_MODULE
    }

    fn build(&self, db: DatabaseConnection) -> Arc<dyn SearchProjectionSource> {
        Arc::new(DocsSearchProjectionSource::new(db))
    }
}

#[derive(Clone)]
pub struct DocsSearchProjectionSource {
    db: DatabaseConnection,
}

impl DocsSearchProjectionSource {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    async fn project_page(&self, tenant_id: Uuid, page: &page::Model) -> SearchProjectionDocument {
        let markdown = DocumentService::render_markdown(&self.db, tenant_id, page.id)
            .await
            .unwrap_or_default();

        let plain_body = extract_plain_text(&markdown);
        let keywords_text = format!("{} {}", page.title, page.slug);

        SearchProjectionDocument {
            document_key: format!("{DOCS_SOURCE_MODULE}:{}:{DEFAULT_DOC_LOCALE}", page.id),
            tenant_id,
            document_id: page.id,
            source_module: DOCS_SOURCE_MODULE.to_string(),
            entity_type: DOC_PAGE_ENTITY_TYPE.to_string(),
            locale: DEFAULT_DOC_LOCALE.to_string(),
            status: if page.is_archived {
                "archived".to_string()
            } else {
                "published".to_string()
            },
            is_public: !page.is_archived,
            title: page.title.clone(),
            subtitle: None,
            slug: Some(page.slug.clone()),
            handle: None,
            body: plain_body,
            keywords_text,
            facets: json!({
                "is_archived": page.is_archived,
                "has_parent": page.parent_id.is_some(),
            }),
            payload: json!({
                "slug": page.slug,
                "icon": page.icon,
                "parent_id": page.parent_id,
                "cover_image": page.cover_image,
            }),
            published_at: Some(page.created_at.into()),
            created_at: page.created_at.into(),
            updated_at: page.updated_at.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DocPageCursor {
    created_at: DateTime<Utc>,
    id: Uuid,
}

impl DocPageCursor {
    fn parse(cursor: &str) -> Option<Self> {
        let mut parts = cursor.split('#');
        let created_at_str = parts.next()?;
        let id_str = parts.next()?;
        if parts.next().is_some() {
            return None;
        }
        let created_at = DateTime::parse_from_rfc3339(created_at_str)
            .ok()?
            .with_timezone(&Utc);
        let id = Uuid::parse_str(id_str).ok()?;
        Some(Self { created_at, id })
    }

    fn to_cursor_string(&self) -> String {
        format!("{}#{}", self.created_at.to_rfc3339(), self.id)
    }
}

fn extract_plain_text(markdown: &str) -> String {
    let mut words = Vec::new();
    for line in markdown.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let cleaned = trimmed
            .trim_start_matches('#')
            .trim_start_matches('*')
            .trim_start_matches('-')
            .trim_start_matches('>')
            .trim();
        if !cleaned.is_empty() {
            words.push(cleaned);
        }
    }
    words.join("\n")
}

#[async_trait]
impl SearchProjectionSource for DocsSearchProjectionSource {
    fn source_module(&self) -> &'static str {
        DOCS_SOURCE_MODULE
    }

    async fn list_public_documents(
        &self,
        tenant_id: Uuid,
        after: Option<String>,
        limit: usize,
    ) -> Result<SearchProjectionPage> {
        if tenant_id.is_nil() {
            return Err(Error::Validation("Tenant ID cannot be a nil UUID".to_string()));
        }

        let bounded_limit = limit.clamp(1, MAX_SEARCH_PROJECTION_PAGE_SIZE);

        let mut query = page::Entity::find()
            .filter(page::Column::TenantId.eq(tenant_id))
            .filter(page::Column::DeletedAt.is_null())
            .filter(page::Column::IsArchived.eq(false));

        if let Some(cursor_str) = after.as_deref() {
            let cursor = DocPageCursor::parse(cursor_str).ok_or_else(|| {
                Error::Validation(format!("Invalid Docs Search projection cursor `{cursor_str}`"))
            })?;

            let cursor_created_at: chrono::DateTime<chrono::FixedOffset> = cursor.created_at.into();
            query = query.filter(
                Condition::any()
                    .add(page::Column::CreatedAt.gt(cursor_created_at))
                    .add(
                        Condition::all()
                            .add(page::Column::CreatedAt.eq(cursor_created_at))
                            .add(page::Column::Id.gt(cursor.id)),
                    ),
            );
        }

        let pages = query
            .order_by_asc(page::Column::CreatedAt)
            .order_by_asc(page::Column::Id)
            .limit(bounded_limit as u64 + 1)
            .all(&self.db)
            .await
            .map_err(Error::Database)?;

        let has_more = pages.len() > bounded_limit;
        let mut slice = pages;
        if has_more {
            slice.pop();
        }

        let next_cursor = if has_more {
            slice.last().map(|p| {
                DocPageCursor {
                    created_at: p.created_at.into(),
                    id: p.id,
                }
                .to_cursor_string()
            })
        } else {
            None
        };

        let mut documents = Vec::with_capacity(slice.len());
        for p in &slice {
            documents.push(self.project_page(tenant_id, p).await);
        }

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
        if tenant_id.is_nil() {
            return Err(Error::Validation("Tenant ID cannot be a nil UUID".to_string()));
        }
        if entity_id.is_nil() {
            return Err(Error::Validation("Entity ID cannot be a nil UUID".to_string()));
        }
        if entity_type != DOC_PAGE_ENTITY_TYPE {
            return Ok(Vec::new());
        }

        let page = page::Entity::find()
            .filter(page::Column::TenantId.eq(tenant_id))
            .filter(page::Column::Id.eq(entity_id))
            .filter(page::Column::DeletedAt.is_null())
            .one(&self.db)
            .await
            .map_err(Error::Database)?;

        let Some(page) = page else {
            return Ok(Vec::new());
        };

        if page.is_archived {
            return Ok(Vec::new());
        }

        let doc = self.project_page(tenant_id, &page).await;
        Ok(vec![doc])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cursor_round_trip() {
        let cursor = DocPageCursor {
            created_at: Utc::now(),
            id: Uuid::new_v4(),
        };
        let encoded = cursor.to_cursor_string();
        let decoded = DocPageCursor::parse(&encoded).expect("valid cursor");
        assert_eq!(cursor.id, decoded.id);
        assert_eq!(
            cursor.created_at.timestamp_millis(),
            decoded.created_at.timestamp_millis()
        );
    }

    #[test]
    fn invalid_cursor_fails_closed() {
        assert!(DocPageCursor::parse("not-a-cursor").is_none());
        assert!(DocPageCursor::parse("2026-01-01T00:00:00Z#invalid-uuid").is_none());
    }

    #[test]
    fn plain_text_extractor_cleans_markdown_markers() {
        let md = "# Title\n\n* Bullet item 1\n- Bullet item 2\n> Blockquote text\nParagraph";
        let extracted = extract_plain_text(md);
        assert!(extracted.contains("Title"));
        assert!(extracted.contains("Bullet item 1"));
        assert!(extracted.contains("Bullet item 2"));
        assert!(extracted.contains("Blockquote text"));
        assert!(extracted.contains("Paragraph"));
        assert!(!extracted.starts_with('#'));
    }
}
