use async_trait::async_trait;
use chrono::Utc;
use rustok_content::entities::{body, node, node_translation};
use rustok_core::events::{DomainEvent, EventEnvelope, EventHandler, HandlerResult};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set,
};
use tracing::{debug, info, instrument, warn};
use uuid::Uuid;

use crate::error::IndexResult;
use crate::traits::{Indexer, IndexerContext, LocaleIndexer};
use super::entity::{self as index_content_entity, ActiveModel as IndexContentActiveModel};

pub struct ContentIndexer {
    db: DatabaseConnection,
}

impl ContentIndexer {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    #[instrument(skip(self, ctx))]
    async fn build_index_content(
        &self,
        ctx: &IndexerContext,
        node_id: Uuid,
        locale: &str,
    ) -> IndexResult<Option<super::model::IndexContentModel>> {
        let node = match node::Entity::find_by_id(node_id)
            .filter(node::Column::TenantId.eq(ctx.tenant_id))
            .filter(node::Column::DeletedAt.is_null())
            .one(&self.db)
            .await?
        {
            Some(n) => n,
            None => {
                debug!(node_id = %node_id, "Node not found or deleted, skipping index build");
                return Ok(None);
            }
        };

        let translation = node_translation::Entity::find()
            .filter(node_translation::Column::NodeId.eq(node_id))
            .filter(node_translation::Column::Locale.eq(locale))
            .one(&self.db)
            .await?;

        let body = body::Entity::find()
            .filter(body::Column::NodeId.eq(node_id))
            .filter(body::Column::Locale.eq(locale))
            .one(&self.db)
            .await?;

        use super::model::{IndexContentModel, IndexTag};

        let model = IndexContentModel {
            id: rustok_core::generate_id(),
            tenant_id: ctx.tenant_id,
            node_id,
            locale: locale.to_string(),
            kind: node.kind.clone(),
            status: format!("{:?}", node.status).to_lowercase(),
            title: translation.as_ref().and_then(|t| t.title.clone()),
            slug: translation.as_ref().and_then(|t| t.slug.clone()),
            excerpt: translation.as_ref().and_then(|t| t.excerpt.clone()),
            body: body.as_ref().and_then(|b| b.body.clone()),
            body_format: body.as_ref().map(|b| b.format.clone()),
            author_id: node.author_id,
            author_name: None,
            author_avatar: None,
            category_id: node.category_id,
            category_name: None,
            category_slug: None,
            tags: vec![],
            meta_title: None,
            meta_description: None,
            og_image: None,
            featured_image_url: None,
            featured_image_alt: None,
            parent_id: node.parent_id,
            depth: node.depth,
            position: node.position,
            reply_count: node.reply_count,
            view_count: 0,
            published_at: node.published_at.map(|dt| dt.with_timezone(&Utc)),
            created_at: node.created_at.with_timezone(&Utc),
            updated_at: node.updated_at.with_timezone(&Utc),
        };

        Ok(Some(model))
    }

    async fn upsert_index_content(
        &self,
        model: super::model::IndexContentModel,
    ) -> IndexResult<()> {
        let now = Utc::now();
        let tz_now: chrono::DateTime<chrono::FixedOffset> = now.fixed_offset();

        let existing = index_content_entity::Entity::find()
            .filter(index_content_entity::Column::NodeId.eq(model.node_id))
            .filter(index_content_entity::Column::Locale.eq(&model.locale))
            .one(&self.db)
            .await?;

        let tags_json = serde_json::to_value(&model.tags)
            .unwrap_or(serde_json::Value::Array(vec![]));

        match existing {
            Some(existing_model) => {
                let mut active: index_content_entity::ActiveModel = existing_model.into();
                active.tenant_id = Set(model.tenant_id);
                active.kind = Set(model.kind);
                active.status = Set(model.status);
                active.title = Set(model.title);
                active.slug = Set(model.slug);
                active.excerpt = Set(model.excerpt);
                active.body = Set(model.body);
                active.body_format = Set(model.body_format);
                active.author_id = Set(model.author_id);
                active.author_name = Set(model.author_name);
                active.author_avatar = Set(model.author_avatar);
                active.category_id = Set(model.category_id);
                active.category_name = Set(model.category_name);
                active.category_slug = Set(model.category_slug);
                active.tags = Set(tags_json);
                active.meta_title = Set(model.meta_title);
                active.meta_description = Set(model.meta_description);
                active.og_image = Set(model.og_image);
                active.featured_image_url = Set(model.featured_image_url);
                active.featured_image_alt = Set(model.featured_image_alt);
                active.parent_id = Set(model.parent_id);
                active.depth = Set(model.depth);
                active.position = Set(model.position);
                active.reply_count = Set(model.reply_count);
                active.view_count = Set(model.view_count);
                active.published_at = Set(model.published_at.map(|dt| dt.fixed_offset()));
                active.updated_at = Set(tz_now);
                active.indexed_at = Set(tz_now);
                active.update(&self.db).await?;
            }
            None => {
                let active = IndexContentActiveModel {
                    id: Set(model.id),
                    tenant_id: Set(model.tenant_id),
                    node_id: Set(model.node_id),
                    locale: Set(model.locale),
                    kind: Set(model.kind),
                    status: Set(model.status),
                    title: Set(model.title),
                    slug: Set(model.slug),
                    excerpt: Set(model.excerpt),
                    body: Set(model.body),
                    body_format: Set(model.body_format),
                    author_id: Set(model.author_id),
                    author_name: Set(model.author_name),
                    author_avatar: Set(model.author_avatar),
                    category_id: Set(model.category_id),
                    category_name: Set(model.category_name),
                    category_slug: Set(model.category_slug),
                    tags: Set(tags_json),
                    meta_title: Set(model.meta_title),
                    meta_description: Set(model.meta_description),
                    og_image: Set(model.og_image),
                    featured_image_url: Set(model.featured_image_url),
                    featured_image_alt: Set(model.featured_image_alt),
                    parent_id: Set(model.parent_id),
                    depth: Set(model.depth),
                    position: Set(model.position),
                    reply_count: Set(model.reply_count),
                    view_count: Set(model.view_count),
                    published_at: Set(model.published_at.map(|dt| dt.fixed_offset())),
                    created_at: Set(model.created_at.fixed_offset()),
                    updated_at: Set(tz_now),
                    indexed_at: Set(tz_now),
                };
                active.insert(&self.db).await?;
            }
        }

        Ok(())
    }

    async fn delete_index_content(&self, node_id: Uuid, locale: &str) -> IndexResult<()> {
        index_content_entity::Entity::delete_many()
            .filter(index_content_entity::Column::NodeId.eq(node_id))
            .filter(index_content_entity::Column::Locale.eq(locale))
            .exec(&self.db)
            .await?;
        Ok(())
    }

    async fn delete_all_locales(&self, node_id: Uuid) -> IndexResult<()> {
        index_content_entity::Entity::delete_many()
            .filter(index_content_entity::Column::NodeId.eq(node_id))
            .exec(&self.db)
            .await?;
        Ok(())
    }

    async fn get_tenant_locales(&self, ctx: &IndexerContext) -> IndexResult<Vec<String>> {
        let _ = ctx;
        Ok(vec!["en".to_string()])
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

        for locale in locales {
            self.index_locale(ctx, entity_id, &locale).await?;
        }

        Ok(())
    }

    #[instrument(skip(self, ctx))]
    async fn remove_one(&self, ctx: &IndexerContext, entity_id: Uuid) -> IndexResult<()> {
        let _ = ctx;
        debug!(node_id = %entity_id, "Removing all locales from content index");
        self.delete_all_locales(entity_id).await
    }

    #[instrument(skip(self, ctx))]
    async fn reindex_all(&self, ctx: &IndexerContext) -> IndexResult<u64> {
        info!(tenant_id = %ctx.tenant_id, "Reindexing all content");

        let locales = self.get_tenant_locales(ctx).await?;

        let nodes = node::Entity::find()
            .filter(node::Column::TenantId.eq(ctx.tenant_id))
            .filter(node::Column::DeletedAt.is_null())
            .all(&self.db)
            .await?;

        let count = nodes.len() as u64;

        for node_row in nodes {
            for locale in &locales {
                if let Err(err) = self.index_locale(ctx, node_row.id, locale).await {
                    warn!(
                        node_id = %node_row.id,
                        locale = locale,
                        error = %err,
                        "Failed to reindex content node"
                    );
                }
            }
        }

        info!(tenant_id = %ctx.tenant_id, indexed = count, "Content reindex complete");
        Ok(count)
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
                self.upsert_index_content(model).await?;
                debug!(node_id = %entity_id, locale = locale, "Indexed content");
            }
            None => {
                self.delete_index_content(entity_id, locale).await?;
                debug!(node_id = %entity_id, locale = locale, "Removed locale from content index (node not found/deleted)");
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
        self.delete_index_content(entity_id, locale).await
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
            | DomainEvent::BodyUpdated { .. } => true,
            DomainEvent::TagAttached { target_type, .. }
            | DomainEvent::TagDetached { target_type, .. } => target_type == "node",
            DomainEvent::ReindexRequested { target_type, .. } => target_type == "content",
            _ => false,
        }
    }

    async fn handle(&self, envelope: &EventEnvelope) -> HandlerResult {
        let ctx = IndexerContext::new(self.db.clone(), envelope.tenant_id);

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

            _ => {}
        }

        Ok(())
    }
}
