//! Forum-owned public routes: redirects from merged, converted, or retired topic
//! routes.
//!
//! Topic slugs are locale-aware, so a redirect source is keyed by locale. The
//! canonical route of a topic is derived from its id by [`forum_topic_route`]
//! and is never stored. Every change publishes the same `CanonicalUrlChanged` /
//! `UrlAliasPurged` events that SEO consumes.

use chrono::Utc;
use rustok_content::normalize_locale_code;
use rustok_events::DomainEvent;
use rustok_outbox::TransactionalEventBus;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseTransaction, EntityTrait, QueryFilter,
    Set,
};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

use crate::entities::forum_topic_route;
use crate::error::{ForumError, ForumResult};

/// Every Forum topic route starts with this prefix. Redirect sources must match it.
pub const FORUM_ROUTE_PREFIX: &str = "/modules/forum?";

/// Canonical route of a topic, derived from its id.
pub fn forum_topic_route(topic_id: Uuid) -> String {
    format!("/modules/forum?topic={topic_id}")
}

/// Redirect row for one locale, as stored. Locale fallback is applied by the
/// caller through `rustok_content::resolve_by_locale`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForumTopicRedirect {
    pub locale: String,
    pub target_kind: String,
    pub target_id: Uuid,
}

#[derive(Clone)]
pub struct ForumTopicRouteOwner {
    event_bus: TransactionalEventBus,
}

impl ForumTopicRouteOwner {
    pub fn new(event_bus: TransactionalEventBus) -> Self {
        Self { event_bus }
    }

    /// Returns every locale row that redirects `source_route`.
    pub async fn find_redirects<C: ConnectionTrait>(
        conn: &C,
        tenant_id: Uuid,
        source_route: &str,
    ) -> ForumResult<Vec<ForumTopicRedirect>> {
        let rows = forum_topic_route::Entity::find()
            .filter(forum_topic_route::Column::TenantId.eq(tenant_id))
            .filter(forum_topic_route::Column::SourceRoute.eq(source_route))
            .all(conn)
            .await?;
        Ok(rows
            .into_iter()
            .map(|row| ForumTopicRedirect {
                locale: row.locale,
                target_kind: row.target_kind,
                target_id: row.target_id,
            })
            .collect())
    }

    /// Records a redirect from `source_route` in `locale` to a target. The
    /// target's canonical route in its own owner is passed in and published
    /// with the change. A redirect that already points at the same target is a
    /// no-op.
    #[allow(clippy::too_many_arguments)]
    pub async fn record_redirect_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        locale: &str,
        source_route: &str,
        target_kind: &str,
        target_id: Uuid,
        target_canonical_url: &str,
    ) -> ForumResult<()> {
        let locale = normalize_locale_code(locale)
            .ok_or_else(|| ForumError::Validation("forum route locale must not be empty".into()))?;
        if !source_route.starts_with(FORUM_ROUTE_PREFIX) {
            return Err(ForumError::Validation(format!(
                "Forum redirect source must start with `{FORUM_ROUTE_PREFIX}`"
            )));
        }
        if source_route.len() > 512 {
            return Err(ForumError::Validation(
                "Forum redirect source must be <= 512 chars".into(),
            ));
        }
        let now = Utc::now();
        let key = (tenant_id, locale.clone(), source_route.to_string());
        let existing = forum_topic_route::Entity::find_by_id(key).one(txn).await?;

        match existing {
            Some(existing)
                if existing.target_kind == target_kind && existing.target_id == target_id =>
            {
                return Ok(());
            }
            Some(existing) => {
                // The source moves to a new target. The displaced target loses
                // this route, and that loss is published.
                self.publish_purged(
                    txn,
                    tenant_id,
                    actor_id,
                    &existing.target_kind,
                    existing.target_id,
                    &locale,
                    vec![source_route.to_string()],
                )
                .await?;
                let mut active: forum_topic_route::ActiveModel = existing.into();
                active.target_kind = Set(target_kind.to_string());
                active.target_id = Set(target_id);
                active.updated_at = Set(now.into());
                active.update(txn).await?;
            }
            None => {
                forum_topic_route::ActiveModel {
                    tenant_id: Set(tenant_id),
                    locale: Set(locale.clone()),
                    source_route: Set(source_route.to_string()),
                    target_kind: Set(target_kind.to_string()),
                    target_id: Set(target_id),
                    created_at: Set(now.into()),
                    updated_at: Set(now.into()),
                }
                .insert(txn)
                .await?;
            }
        }

        self.event_bus
            .publish_in_tx(
                txn,
                tenant_id,
                actor_id,
                DomainEvent::CanonicalUrlChanged {
                    target_id,
                    target_kind: target_kind.to_string(),
                    locale,
                    new_canonical_url: target_canonical_url.to_string(),
                    old_urls: vec![source_route.to_string()],
                },
            )
            .await?;
        Ok(())
    }

    /// Removes every redirect that points at `target_kind:target_id` (a deleted
    /// topic, a converted post) and reports each removed source through
    /// `UrlAliasPurged`, per locale.
    pub async fn remove_redirects_to_target_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        target_kind: &str,
        target_id: Uuid,
    ) -> ForumResult<()> {
        let rows = forum_topic_route::Entity::find()
            .filter(forum_topic_route::Column::TenantId.eq(tenant_id))
            .filter(forum_topic_route::Column::TargetKind.eq(target_kind))
            .filter(forum_topic_route::Column::TargetId.eq(target_id))
            .all(txn)
            .await?;
        if rows.is_empty() {
            return Ok(());
        }

        forum_topic_route::Entity::delete_many()
            .filter(forum_topic_route::Column::TenantId.eq(tenant_id))
            .filter(forum_topic_route::Column::TargetKind.eq(target_kind))
            .filter(forum_topic_route::Column::TargetId.eq(target_id))
            .exec(txn)
            .await?;

        let mut by_locale: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for row in rows {
            by_locale
                .entry(row.locale)
                .or_default()
                .insert(row.source_route);
        }
        for (locale, urls) in by_locale {
            self.publish_purged(
                txn,
                tenant_id,
                actor_id,
                target_kind,
                target_id,
                &locale,
                urls.into_iter().collect(),
            )
            .await?;
        }
        Ok(())
    }

    /// Purges the derived canonical route of a topic in each locale the topic
    /// had. Called when the topic leaves Forum.
    pub async fn purge_topic_canonical_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        topic_id: Uuid,
        locales: &[String],
    ) -> ForumResult<()> {
        let canonical = forum_topic_route(topic_id);
        for locale in locales {
            let locale = normalize_locale_code(locale).ok_or_else(|| {
                ForumError::Validation("forum route locale must not be empty".into())
            })?;
            self.publish_purged(
                txn,
                tenant_id,
                actor_id,
                "forum_topic",
                topic_id,
                &locale,
                vec![canonical.clone()],
            )
            .await?;
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    async fn publish_purged(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        target_kind: &str,
        target_id: Uuid,
        locale: &str,
        urls: Vec<String>,
    ) -> ForumResult<()> {
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
                    locale: locale.to_string(),
                    urls,
                },
            )
            .await?;
        Ok(())
    }
}
