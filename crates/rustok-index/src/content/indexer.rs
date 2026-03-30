use async_trait::async_trait;
use chrono::Utc;
use rustok_core::{
    events::{EventHandler, HandlerResult},
    simple_hash,
    slugify,
};
use rustok_events::{DomainEvent, EventEnvelope};
use sea_orm::{ConnectionTrait, DatabaseBackend, DatabaseConnection, FromQueryResult, Statement};
use serde_json::Value as JsonValue;
use tracing::{debug, info, instrument, warn};
use uuid::Uuid;

use crate::content::model::IndexContentModel;
use crate::error::IndexResult;
use crate::traits::{
    run_bounded_reindex, Indexer, IndexerContext, IndexerRuntimeConfig, LocaleIndexer,
};

/// Raw DB row for building index_content
#[derive(Debug, FromQueryResult)]
struct NodeRow {
    id: Uuid,
    tenant_id: Uuid,
    parent_id: Option<Uuid>,
    author_id: Option<Uuid>,
    kind: String,
    category_id: Option<Uuid>,
    status: String,
    position: i32,
    depth: i32,
    reply_count: i32,
    metadata: JsonValue,
    published_at: Option<chrono::DateTime<chrono::FixedOffset>>,
    created_at: chrono::DateTime<chrono::FixedOffset>,
    updated_at: chrono::DateTime<chrono::FixedOffset>,
    locale: Option<String>,
    title: Option<String>,
    slug: Option<String>,
    excerpt: Option<String>,
    body: Option<String>,
    body_format: Option<String>,
    category_name: Option<String>,
    category_slug: Option<String>,
    author_name: Option<String>,
}

/// Content indexer - listens to events and updates index_content table
#[derive(Clone)]
pub struct ContentIndexer {
    db: DatabaseConnection,
    runtime: IndexerRuntimeConfig,
}

impl ContentIndexer {
    pub fn new(db: DatabaseConnection) -> Self {
        Self::with_runtime(db, IndexerRuntimeConfig::load())
    }

    pub fn with_runtime(db: DatabaseConnection, runtime: IndexerRuntimeConfig) -> Self {
        Self { db, runtime }
    }

    fn backend(&self) -> DatabaseBackend {
        self.db.get_database_backend()
    }

    /// Build denormalized content from normalized tables
    #[instrument(skip(self, ctx))]
    async fn build_index_content(
        &self,
        ctx: &IndexerContext,
        node_id: Uuid,
        locale: &str,
    ) -> IndexResult<Option<IndexContentModel>> {
        let stmt = Statement::from_sql_and_values(
            self.backend(),
            r#"
            SELECT
                n.id,
                n.tenant_id,
                n.parent_id,
                n.author_id,
                n.kind,
                n.category_id,
                n.status::text AS status,
                n.position,
                n.depth,
                n.reply_count,
                n.metadata,
                n.published_at,
                n.created_at,
                n.updated_at,
                nt.locale,
                nt.title,
                nt.slug,
                nt.excerpt,
                b.body,
                b.format AS body_format,
                ct.name AS category_name,
                ct.slug AS category_slug,
                u.name AS author_name
            FROM nodes n
            LEFT JOIN node_translations nt
                ON nt.node_id = n.id AND nt.locale = $3
            LEFT JOIN bodies b
                ON b.node_id = n.id AND b.locale = $3
            LEFT JOIN category_translations ct
                ON ct.category_id = n.category_id AND ct.locale = $3
            LEFT JOIN users u
                ON u.id = n.author_id
            WHERE n.id = $1
              AND n.tenant_id = $2
              AND n.deleted_at IS NULL
            "#,
            vec![node_id.into(), ctx.tenant_id.into(), locale.into()],
        );

        let row = NodeRow::find_by_statement(stmt)
            .one(&self.db)
            .await
            .map_err(crate::error::IndexError::from)?;

        let row = match row {
            Some(r) => r,
            None => {
                debug!(node_id = %node_id, locale = locale, "Node not found in DB, skipping index");
                return Ok(None);
            }
        };

        let tags = Self::extract_node_tags(&row.metadata);

        let model = IndexContentModel {
            id: Uuid::new_v4(),
            tenant_id: row.tenant_id,
            node_id: row.id,
            locale: row.locale.unwrap_or_else(|| locale.to_string()),
            kind: row.kind,
            status: row.status,
            title: row.title,
            slug: row.slug,
            excerpt: row.excerpt,
            body: row.body,
            body_format: row.body_format,
            author_id: row.author_id,
            author_name: row.author_name,
            author_avatar: None,
            category_id: row.category_id,
            category_name: row.category_name,
            category_slug: row.category_slug,
            tags,
            meta_title: None,
            meta_description: None,
            og_image: None,
            featured_image_url: None,
            featured_image_alt: None,
            parent_id: row.parent_id,
            depth: row.depth,
            position: row.position,
            reply_count: row.reply_count,
            view_count: 0,
            published_at: row.published_at.map(|dt| dt.with_timezone(&Utc)),
            created_at: row.created_at.with_timezone(&Utc),
            updated_at: row.updated_at.with_timezone(&Utc),
        };

        Ok(Some(model))
    }

    fn extract_node_tags(metadata: &JsonValue) -> Vec<crate::content::model::IndexTag> {
        let mut seen = std::collections::HashSet::new();
        metadata
            .get("tags")
            .and_then(JsonValue::as_array)
            .into_iter()
            .flatten()
            .filter_map(JsonValue::as_str)
            .filter_map(|tag_name| {
                let trimmed = tag_name.trim();
                if trimmed.is_empty() {
                    return None;
                }

                let slug = slugify(trimmed);
                if slug.is_empty() || !seen.insert(slug.clone()) {
                    return None;
                }

                Some(crate::content::model::IndexTag {
                    id: Uuid::from_u128(simple_hash(&format!("content-tag:{slug}")) as u128),
                    name: trimmed.to_string(),
                    slug,
                })
            })
            .collect()
    }

    /// Upsert the index_content row
    #[instrument(skip(self, model))]
    async fn upsert_index_content(&self, model: &IndexContentModel) -> IndexResult<()> {
        let tags_json = serde_json::to_value(&model.tags).unwrap_or(JsonValue::Array(vec![]));

        let stmt = Statement::from_sql_and_values(
            self.backend(),
            r#"
            INSERT INTO index_content (
                id, tenant_id, node_id, locale, kind, status,
                title, slug, excerpt, body, body_format,
                author_id, author_name, author_avatar,
                category_id, category_name, category_slug,
                tags,
                meta_title, meta_description, og_image,
                featured_image_url, featured_image_alt,
                parent_id, depth, position,
                reply_count, view_count,
                published_at, created_at, updated_at, indexed_at
            ) VALUES (
                $1, $2, $3, $4, $5, $6,
                $7, $8, $9, $10, $11,
                $12, $13, $14,
                $15, $16, $17,
                $18,
                $19, $20, $21,
                $22, $23,
                $24, $25, $26,
                $27, $28,
                $29, $30, $31, NOW()
            )
            ON CONFLICT (node_id, locale) DO UPDATE SET
                kind = EXCLUDED.kind,
                status = EXCLUDED.status,
                title = EXCLUDED.title,
                slug = EXCLUDED.slug,
                excerpt = EXCLUDED.excerpt,
                body = EXCLUDED.body,
                body_format = EXCLUDED.body_format,
                author_id = EXCLUDED.author_id,
                author_name = EXCLUDED.author_name,
                author_avatar = EXCLUDED.author_avatar,
                category_id = EXCLUDED.category_id,
                category_name = EXCLUDED.category_name,
                category_slug = EXCLUDED.category_slug,
                tags = EXCLUDED.tags,
                meta_title = EXCLUDED.meta_title,
                meta_description = EXCLUDED.meta_description,
                og_image = EXCLUDED.og_image,
                featured_image_url = EXCLUDED.featured_image_url,
                featured_image_alt = EXCLUDED.featured_image_alt,
                parent_id = EXCLUDED.parent_id,
                depth = EXCLUDED.depth,
                position = EXCLUDED.position,
                reply_count = EXCLUDED.reply_count,
                view_count = EXCLUDED.view_count,
                published_at = EXCLUDED.published_at,
                updated_at = EXCLUDED.updated_at,
                indexed_at = NOW()
            "#,
            vec![
                model.id.into(),
                model.tenant_id.into(),
                model.node_id.into(),
                model.locale.clone().into(),
                model.kind.clone().into(),
                model.status.clone().into(),
                model.title.clone().into(),
                model.slug.clone().into(),
                model.excerpt.clone().into(),
                model.body.clone().into(),
                model.body_format.clone().into(),
                model.author_id.into(),
                model.author_name.clone().into(),
                model.author_avatar.clone().into(),
                model.category_id.into(),
                model.category_name.clone().into(),
                model.category_slug.clone().into(),
                tags_json.into(),
                model.meta_title.clone().into(),
                model.meta_description.clone().into(),
                model.og_image.clone().into(),
                model.featured_image_url.clone().into(),
                model.featured_image_alt.clone().into(),
                model.parent_id.into(),
                model.depth.into(),
                model.position.into(),
                model.reply_count.into(),
                model.view_count.into(),
                model.published_at.into(),
                model.created_at.into(),
                model.updated_at.into(),
            ],
        );

        self.db
            .execute(stmt)
            .await
            .map(|_| ())
            .map_err(crate::error::IndexError::from)
    }

    /// Remove all locales for a node from index_content
    #[instrument(skip(self))]
    async fn delete_node_from_index(&self, node_id: Uuid) -> IndexResult<()> {
        let stmt = Statement::from_sql_and_values(
            self.backend(),
            "DELETE FROM index_content WHERE node_id = $1",
            vec![node_id.into()],
        );
        self.db
            .execute(stmt)
            .await
            .map(|_| ())
            .map_err(crate::error::IndexError::from)
    }

    /// Remove a specific locale for a node from index_content
    #[instrument(skip(self))]
    async fn delete_node_locale_from_index(&self, node_id: Uuid, locale: &str) -> IndexResult<()> {
        let stmt = Statement::from_sql_and_values(
            self.backend(),
            "DELETE FROM index_content WHERE node_id = $1 AND locale = $2",
            vec![node_id.into(), locale.into()],
        );
        self.db
            .execute(stmt)
            .await
            .map(|_| ())
            .map_err(crate::error::IndexError::from)
    }

    async fn get_tenant_locales(&self, ctx: &IndexerContext) -> IndexResult<Vec<String>> {
        #[derive(FromQueryResult)]
        struct LocaleRow {
            locale: String,
        }

        let stmt = Statement::from_sql_and_values(
            self.backend(),
            "SELECT locale FROM tenant_locales WHERE tenant_id = $1",
            vec![ctx.tenant_id.into()],
        );

        let rows = LocaleRow::find_by_statement(stmt)
            .all(&self.db)
            .await
            .unwrap_or_default();

        if rows.is_empty() {
            Ok(vec!["en".to_string()])
        } else {
            Ok(rows.into_iter().map(|r| r.locale).collect())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ContentIndexer;

    #[test]
    fn extract_node_tags_reads_metadata_tags_without_duplicates() {
        let tags = ContentIndexer::extract_node_tags(&serde_json::json!({
            "tags": ["Rust", " rust ", "", "Backend", "backend"]
        }));

        assert_eq!(tags.len(), 2);
        assert_eq!(tags[0].name, "Rust");
        assert_eq!(tags[0].slug, "rust");
        assert_eq!(tags[1].name, "Backend");
        assert_eq!(tags[1].slug, "backend");
    }
}

#[async_trait]
impl Indexer for ContentIndexer {
    fn name(&self) -> &'static str {
        "content_indexer"
    }

    #[instrument(skip(self, ctx))]
    async fn index_one(&self, ctx: &IndexerContext, entity_id: Uuid) -> IndexResult<()> {
        let locales = self.get_tenant_locales(ctx).await?;

        let mut failed = 0usize;
        for locale in &locales {
            if let Err(err) = self.index_locale(ctx, entity_id, locale).await {
                failed += 1;
                warn!(
                    node_id = %entity_id,
                    locale = locale,
                    error = %err,
                    "Failed to index content locale; continuing with remaining locales"
                );
            }
        }

        if failed > 0 && failed == locales.len() {
            return Err(crate::error::IndexError::Index(format!(
                "Failed to index node {entity_id} in all {} locale(s)",
                locales.len()
            )));
        }

        Ok(())
    }

    #[instrument(skip(self, ctx))]
    async fn remove_one(&self, ctx: &IndexerContext, entity_id: Uuid) -> IndexResult<()> {
        let _ = ctx;
        debug!(node_id = %entity_id, "Removing from content index");
        self.delete_node_from_index(entity_id).await
    }

    #[instrument(skip(self, ctx))]
    async fn reindex_all(&self, ctx: &IndexerContext) -> IndexResult<u64> {
        info!(tenant_id = %ctx.tenant_id, "Reindexing all content");

        #[derive(FromQueryResult)]
        struct IdRow {
            id: Uuid,
        }

        let stmt = Statement::from_sql_and_values(
            self.backend(),
            "SELECT id FROM nodes WHERE tenant_id = $1 AND deleted_at IS NULL",
            vec![ctx.tenant_id.into()],
        );

        let rows = IdRow::find_by_statement(stmt)
            .all(&self.db)
            .await
            .unwrap_or_default();

        let ids = rows.into_iter().map(|row| row.id).collect();
        let stats = run_bounded_reindex(self.clone(), ctx, ids, "reindex_all").await;
        Ok(stats.scheduled)
    }
}

#[async_trait]
impl LocaleIndexer for ContentIndexer {
    #[instrument(skip(self, ctx))]
    async fn index_locale(
        &self,
        ctx: &IndexerContext,
        entity_id: Uuid,
        locale: &str,
    ) -> IndexResult<()> {
        let content = self.build_index_content(ctx, entity_id, locale).await?;

        match content {
            Some(model) => {
                self.upsert_index_content(&model).await?;
                debug!(node_id = %entity_id, locale = locale, "Indexed content");
            }
            None => {
                self.delete_node_locale_from_index(entity_id, locale)
                    .await?;
            }
        }

        Ok(())
    }

    async fn remove_locale(
        &self,
        _ctx: &IndexerContext,
        entity_id: Uuid,
        locale: &str,
    ) -> IndexResult<()> {
        debug!(node_id = %entity_id, locale = locale, "Removed locale from content index");
        self.delete_node_locale_from_index(entity_id, locale).await
    }
}

#[async_trait]
impl EventHandler for ContentIndexer {
    fn name(&self) -> &'static str {
        "content_indexer"
    }

    fn handles(&self, event: &DomainEvent) -> bool {
        match event {
            DomainEvent::NodeCreated { .. }
            | DomainEvent::NodeUpdated { .. }
            | DomainEvent::NodeTranslationUpdated { .. }
            | DomainEvent::NodePublished { .. }
            | DomainEvent::NodeUnpublished { .. }
            | DomainEvent::NodeDeleted { .. }
            | DomainEvent::BodyUpdated { .. }
            | DomainEvent::CategoryUpdated { .. } => true,
            DomainEvent::TagAttached { target_type, .. }
            | DomainEvent::TagDetached { target_type, .. } => target_type == "node",
            DomainEvent::ReindexRequested { target_type, .. } => target_type == "content",
            _ => false,
        }
    }

    async fn handle(&self, envelope: &EventEnvelope) -> HandlerResult {
        let ctx = IndexerContext::new_with_runtime(
            self.db.clone(),
            envelope.tenant_id,
            self.runtime.clone(),
        );

        match &envelope.event {
            DomainEvent::NodeCreated { node_id, .. }
            | DomainEvent::NodeUpdated { node_id, .. }
            | DomainEvent::NodePublished { node_id, .. }
            | DomainEvent::NodeUnpublished { node_id, .. } => {
                self.index_one(&ctx, *node_id).await?;
            }

            DomainEvent::NodeTranslationUpdated { node_id, locale } => {
                self.index_locale(&ctx, *node_id, locale).await?;
            }

            DomainEvent::BodyUpdated { node_id, locale } => {
                self.index_locale(&ctx, *node_id, locale).await?;
            }

            DomainEvent::NodeDeleted { node_id, .. } => {
                self.remove_one(&ctx, *node_id).await?;
            }

            DomainEvent::TagAttached { target_id, .. }
            | DomainEvent::TagDetached { target_id, .. } => {
                self.index_one(&ctx, *target_id).await?;
            }

            DomainEvent::ReindexRequested { target_id, .. } => {
                if let Some(id) = target_id {
                    self.index_one(&ctx, *id).await?;
                } else {
                    self.reindex_all(&ctx).await?;
                }
            }

            DomainEvent::CategoryUpdated { category_id, .. } => {
                self.reindex_category_nodes(&ctx, *category_id).await?;
            }

            _ => {}
        }

        Ok(())
    }
}

impl ContentIndexer {
    async fn reindex_category_nodes(
        &self,
        ctx: &IndexerContext,
        category_id: Uuid,
    ) -> IndexResult<()> {
        #[derive(FromQueryResult)]
        struct IdRow {
            id: Uuid,
        }

        let stmt = Statement::from_sql_and_values(
            self.backend(),
            "SELECT id FROM nodes WHERE tenant_id = $1 AND category_id = $2 AND deleted_at IS NULL",
            vec![ctx.tenant_id.into(), category_id.into()],
        );

        let rows = IdRow::find_by_statement(stmt)
            .all(&self.db)
            .await
            .unwrap_or_default();

        let ids = rows.into_iter().map(|row| row.id).collect();
        let _stats = run_bounded_reindex(self.clone(), ctx, ids, "reindex_category_nodes").await;

        Ok(())
    }
}
