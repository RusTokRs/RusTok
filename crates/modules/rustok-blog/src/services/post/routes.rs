//! Blog-owned public routes: redirects from retired or converted routes.
//!
//! Blog slugs are global, so a redirect source is locale-neutral and unique per
//! tenant. The canonical route of a post is derived from `blog_posts.slug` by
//! [`canonical_post_route`] and is never stored. Every change publishes the same
//! `CanonicalUrlChanged` / `UrlAliasPurged` events that SEO consumes.

use chrono::Utc;
use rustok_events::DomainEvent;
use rustok_outbox::TransactionalEventBus;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseTransaction, EntityTrait,
    QueryFilter, Set,
};
use uuid::Uuid;

use super::{CANONICAL_POST_ROUTE_LOCALE, canonical_post_route};
use crate::entities::blog_post_route;
use crate::error::{BlogError, BlogResult};

/// Every Blog route starts with this prefix. Redirect sources must match it.
pub const BLOG_ROUTE_PREFIX: &str = "/modules/blog?";

/// Resolved redirect owned by Blog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlogPostRedirect {
    pub target_kind: String,
    pub target_id: Uuid,
}

#[derive(Clone)]
pub struct BlogPostRouteOwner {
    event_bus: TransactionalEventBus,
}

impl BlogPostRouteOwner {
    pub fn new(event_bus: TransactionalEventBus) -> Self {
        Self { event_bus }
    }

    /// Looks up a redirect whose source is exactly `route`.
    pub async fn find_redirect<C: ConnectionTrait>(
        conn: &C,
        tenant_id: Uuid,
        route: &str,
    ) -> BlogResult<Option<BlogPostRedirect>> {
        let row = blog_post_route::Entity::find_by_id((tenant_id, route.to_string()))
            .one(conn)
            .await
            .map_err(BlogError::from)?;
        Ok(row.map(|row| BlogPostRedirect {
            target_kind: row.target_kind,
            target_id: row.target_id,
        }))
    }

    /// Frees the redirect held at the canonical route of `slug`, so a post can
    /// take that slug. Every released redirect is reported through
    /// `UrlAliasPurged`. Callers check slug uniqueness before they claim it.
    pub async fn release_slug_route_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        slug: &str,
    ) -> BlogResult<()> {
        let route = canonical_post_route(slug);
        let rows = blog_post_route::Entity::find()
            .filter(blog_post_route::Column::TenantId.eq(tenant_id))
            .filter(blog_post_route::Column::SourceRoute.eq(route.clone()))
            .all(txn)
            .await
            .map_err(BlogError::from)?;

        for row in rows {
            blog_post_route::Entity::delete_by_id((tenant_id, row.source_route.clone()))
                .exec(txn)
                .await
                .map_err(BlogError::from)?;
            self.publish_purged(
                txn,
                tenant_id,
                actor_id,
                &row.target_kind,
                row.target_id,
                vec![route.clone()],
            )
            .await
            .map_err(BlogError::from)?;
        }
        Ok(())
    }

    /// Records that `retired_route` (the previous slug of `post_id`) now
    /// redirects to the post's canonical route, and publishes the change.
    pub async fn redirect_retired_slug_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        post_id: Uuid,
        retired_slug: &str,
        current_slug: &str,
    ) -> BlogResult<()> {
        let retired_route = canonical_post_route(retired_slug);
        let canonical_route = canonical_post_route(current_slug);
        if retired_route == canonical_route {
            return Ok(());
        }
        let changed = self
            .upsert_redirect_in_tx(txn, tenant_id, actor_id, &retired_route, "blog_post", post_id)
            .await?;
        if changed {
            self.event_bus
                .publish_in_tx(
                    txn,
                    tenant_id,
                    actor_id,
                    DomainEvent::CanonicalUrlChanged {
                        target_id: post_id,
                        target_kind: "blog_post".to_string(),
                        locale: CANONICAL_POST_ROUTE_LOCALE.to_string(),
                        new_canonical_url: canonical_route,
                        old_urls: vec![retired_route],
                    },
                )
                .await
                .map_err(BlogError::from)?;
        }
        Ok(())
    }

    /// Records a redirect from a Blog source route to any target, for example a
    /// post demoted to a forum topic. `target_canonical_url` is the canonical
    /// route of the target in its own owner, and it is published with the change.
    pub async fn redirect_source_route_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        source_route: &str,
        target_kind: &str,
        target_id: Uuid,
        target_canonical_url: &str,
    ) -> BlogResult<()> {
        let changed = self
            .upsert_redirect_in_tx(txn, tenant_id, actor_id, source_route, target_kind, target_id)
            .await?;
        if changed {
            self.event_bus
                .publish_in_tx(
                    txn,
                    tenant_id,
                    actor_id,
                    DomainEvent::CanonicalUrlChanged {
                        target_id,
                        target_kind: target_kind.to_string(),
                        locale: CANONICAL_POST_ROUTE_LOCALE.to_string(),
                        new_canonical_url: target_canonical_url.to_string(),
                        old_urls: vec![source_route.to_string()],
                    },
                )
                .await
                .map_err(BlogError::from)?;
        }
        Ok(())
    }

    /// Removes every redirect that points at a deleted or converted post and
    /// purges its canonical route. Used when the post leaves Blog.
    pub async fn remove_post_routes_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        post_id: Uuid,
        current_slug: &str,
    ) -> BlogResult<()> {
        let rows = blog_post_route::Entity::find()
            .filter(blog_post_route::Column::TenantId.eq(tenant_id))
            .filter(blog_post_route::Column::TargetKind.eq("blog_post"))
            .filter(blog_post_route::Column::TargetId.eq(post_id))
            .all(txn)
            .await
            .map_err(BlogError::from)?;

        let mut urls = vec![canonical_post_route(current_slug)];
        for row in &rows {
            urls.push(row.source_route.clone());
        }
        urls.sort();
        urls.dedup();

        blog_post_route::Entity::delete_many()
            .filter(blog_post_route::Column::TenantId.eq(tenant_id))
            .filter(blog_post_route::Column::TargetKind.eq("blog_post"))
            .filter(blog_post_route::Column::TargetId.eq(post_id))
            .exec(txn)
            .await
            .map_err(BlogError::from)?;

        self.publish_purged(txn, tenant_id, actor_id, "blog_post", post_id, urls)
            .await
    }

    async fn upsert_redirect_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        source_route: &str,
        target_kind: &str,
        target_id: Uuid,
    ) -> BlogResult<bool> {
        if !source_route.starts_with(BLOG_ROUTE_PREFIX) {
            return Err(BlogError::validation(format!(
                "Blog redirect source must start with `{BLOG_ROUTE_PREFIX}`"
            )));
        }
        if source_route.len() > 512 {
            return Err(BlogError::validation("Blog redirect source must be <= 512 chars"));
        }
        let now = Utc::now();
        let existing = blog_post_route::Entity::find_by_id((tenant_id, source_route.to_string()))
            .one(txn)
            .await
            .map_err(BlogError::from)?;

        match existing {
            Some(existing) if existing.target_kind == target_kind && existing.target_id == target_id => {
                return Ok(false);
            }
            Some(existing) => {
                // The source route moves to a new target. The displaced target
                // loses this route, and that loss is published.
                self.publish_purged(
                    txn,
                    tenant_id,
                    actor_id,
                    &existing.target_kind,
                    existing.target_id,
                    vec![source_route.to_string()],
                )
                .await?;
                let mut active: blog_post_route::ActiveModel = existing.into();
                active.target_kind = Set(target_kind.to_string());
                active.target_id = Set(target_id);
                active.updated_at = Set(now.into());
                active.update(txn).await.map_err(BlogError::from)?;
            }
            None => {
                blog_post_route::ActiveModel {
                    tenant_id: Set(tenant_id),
                    source_route: Set(source_route.to_string()),
                    target_kind: Set(target_kind.to_string()),
                    target_id: Set(target_id),
                    created_at: Set(now.into()),
                    updated_at: Set(now.into()),
                }
                .insert(txn)
                .await
                .map_err(BlogError::from)?;
            }
        }
        Ok(true)
    }

    async fn publish_purged(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        target_kind: &str,
        target_id: Uuid,
        urls: Vec<String>,
    ) -> BlogResult<()> {
        if urls.is_empty() {
            return Ok(());
        }
        self.event_bus
            .publish_in_tx(
                txn,
                tenant_id,
                actor_id,
                DomainEvent::UrlAliasPurged {
                    target_id,
                    target_kind: target_kind.to_string(),
                    locale: CANONICAL_POST_ROUTE_LOCALE.to_string(),
                    urls,
                },
            )
            .await
            .map_err(BlogError::from)?;
        Ok(())
    }
}
