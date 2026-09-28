use std::collections::HashMap;
use std::time::Instant;

use sea_orm::DatabaseConnection;
use uuid::Uuid;

use rustok_core::SecurityContext;
use rustok_outbox::TransactionalEventBus;

use crate::drivers::{
    import_blog_categories, import_blog_posts, import_forum_categories, import_forum_topics,
    import_navigation, import_pages,
};
use crate::error::StarterResult;
use crate::model::{StarterBlueprint, StarterExecutionReport};

/// The canonical orchestrator for importing Starter Blueprints into a tenant workspace.
pub struct StarterEngine {
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
}

impl StarterEngine {
    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        Self { db, event_bus }
    }

    /// Imports a declarative blueprint in topological dependency order.
    pub async fn import_blueprint(
        &self,
        tenant_id: Uuid,
        security: &SecurityContext,
        blueprint: &StarterBlueprint,
    ) -> StarterResult<StarterExecutionReport> {
        let start_time = Instant::now();
        let locale = &blueprint.locale;

        let mut report = StarterExecutionReport {
            tenant_id,
            blueprint_id: blueprint.id.clone(),
            ..Default::default()
        };

        // Ensure security context has a valid author ID for blog posts, forum topics, and pages.
        // If security.user_id is None (e.g. system context in CLI/installer), generate a deterministic
        // bootstrap author ID for this tenant.
        let effective_security = if security.user_id.is_none() {
            let mut sec = security.clone();
            sec.user_id = Some(Uuid::new_v5(&tenant_id, b"rustok-starter-bootstrap-author"));
            sec
        } else {
            security.clone()
        };

        // 1. Taxonomy & Blog Categories
        let mut blog_category_map = HashMap::new();
        if let Some(taxonomy) = &blueprint.content.taxonomy {
            let (cat_map, created, skipped) = import_blog_categories(
                &self.db,
                &self.event_bus,
                tenant_id,
                &effective_security,
                &taxonomy.categories,
                locale,
            )
            .await?;
            blog_category_map = cat_map;
            report.blog_categories_created = created;
            report.skipped_existing += skipped;
        }

        // 2. Landing Pages (Fly / GrapesJS)
        if let Some(pages) = &blueprint.content.pages {
            let (created, skipped) = import_pages(
                &self.db,
                &self.event_bus,
                tenant_id,
                &effective_security,
                pages,
                locale,
            )
            .await?;
            report.pages_created = created;
            report.skipped_existing += skipped;
        }

        // 3. Blog Posts
        if let Some(blog) = &blueprint.content.blog {
            let (created, skipped) = import_blog_posts(
                &self.db,
                &self.event_bus,
                tenant_id,
                &effective_security,
                &blog.posts,
                &blog_category_map,
                locale,
            )
            .await?;
            report.blog_posts_created = created;
            report.skipped_existing += skipped;
        }

        // 4. Forum Categories & Topics
        if let Some(forum) = &blueprint.content.forum {
            let (forum_cat_map, cat_created, cat_skipped) =
                import_forum_categories(&self.db, tenant_id, &effective_security, &forum.categories, locale)
                    .await?;
            report.forum_categories_created = cat_created;
            report.skipped_existing += cat_skipped;

            let (topics_created, replies_created, topic_skipped) = import_forum_topics(
                &self.db,
                &self.event_bus,
                tenant_id,
                &effective_security,
                &forum.topics,
                &forum_cat_map,
                locale,
            )
            .await?;
            report.forum_topics_created = topics_created;
            report.forum_replies_created = replies_created;
            report.skipped_existing += topic_skipped;
        }

        // 5. Navigation
        if let Some(navigation) = &blueprint.content.navigation {
            let (created, skipped) =
                import_navigation(&self.db, tenant_id, &effective_security, &navigation.menus, locale).await?;
            report.menus_created = created;
            report.skipped_existing += skipped;
        }

        report.duration_ms = start_time.elapsed().as_millis();
        tracing::info!(
            blueprint = %blueprint.id,
            duration_ms = report.duration_ms,
            pages = report.pages_created,
            blog_posts = report.blog_posts_created,
            forum_topics = report.forum_topics_created,
            "Starter blueprint imported successfully"
        );

        Ok(report)
    }
}
