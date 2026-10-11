//! Implementation of the `rustok_content::CanonicalRouteResolver` port.
//!
//! The route namespace selects the owner. Blog and Forum keep their own redirect
//! tables, and canonical routes are derived from owner identity. A route that no
//! owner recognises, or whose target is missing, resolves to `None`. Owner-table
//! errors are returned, and the resolver never falls back to another owner.

use async_trait::async_trait;
use rustok_blog::{
    BLOG_ROUTE_PREFIX, BlogPostRouteOwner, CANONICAL_POST_ROUTE_LOCALE, blog_post,
    canonical_post_route,
};
use rustok_content::{
    CanonicalRouteResolver, ContentError, ContentResult, ResolvedContentRoute,
    normalize_locale_code, normalize_route, resolve_by_locale,
};
use rustok_forum::forum_topic;
use rustok_forum::forum_topic_translation;
use rustok_forum::services::topic_routes::{
    FORUM_ROUTE_PREFIX, ForumTopicRouteOwner, forum_topic_route,
};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use uuid::Uuid;

use crate::errors::{blog_error_to_content_error, forum_error_to_content_error};

const BLOG_SLUG_PARAM: &str = "slug=";
const FORUM_TOPIC_PARAM: &str = "topic=";

pub struct OwnerCanonicalRouteResolver {
    db: DatabaseConnection,
}

impl OwnerCanonicalRouteResolver {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    async fn resolve_blog(
        &self,
        tenant_id: Uuid,
        route: &str,
    ) -> ContentResult<Option<ResolvedContentRoute>> {
        if let Some(slug) = route
            .strip_prefix(BLOG_ROUTE_PREFIX)
            .and_then(|query| query.strip_prefix(BLOG_SLUG_PARAM))
        {
            let live = blog_post::Entity::find()
                .filter(blog_post::Column::TenantId.eq(tenant_id))
                .filter(blog_post::Column::Slug.eq(slug))
                .one(&self.db)
                .await?;
            if let Some(post) = live {
                return Ok(Some(ResolvedContentRoute {
                    target_kind: "blog_post".to_string(),
                    target_id: post.id,
                    locale: CANONICAL_POST_ROUTE_LOCALE.to_string(),
                    matched_url: route.to_string(),
                    canonical_url: route.to_string(),
                    redirect_required: false,
                }));
            }
        }

        let Some(redirect) = BlogPostRouteOwner::find_redirect(&self.db, tenant_id, route)
            .await
            .map_err(blog_error_to_content_error)?
        else {
            return Ok(None);
        };
        let Some(canonical) = self
            .canonical_for_target(tenant_id, &redirect.target_kind, redirect.target_id)
            .await?
        else {
            return Ok(None);
        };
        Ok(Some(ResolvedContentRoute {
            target_kind: redirect.target_kind,
            target_id: redirect.target_id,
            locale: CANONICAL_POST_ROUTE_LOCALE.to_string(),
            matched_url: route.to_string(),
            canonical_url: canonical,
            redirect_required: true,
        }))
    }

    async fn resolve_forum(
        &self,
        tenant_id: Uuid,
        locale: &str,
        route: &str,
    ) -> ContentResult<Option<ResolvedContentRoute>> {
        if let Some(raw_id) = route
            .strip_prefix(FORUM_ROUTE_PREFIX)
            .and_then(|query| query.strip_prefix(FORUM_TOPIC_PARAM))
            && let Ok(topic_id) = Uuid::parse_str(raw_id)
        {
            let topic_exists = forum_topic::Entity::find_by_id(topic_id)
                .filter(forum_topic::Column::TenantId.eq(tenant_id))
                .one(&self.db)
                .await?
                .is_some();
            if topic_exists {
                let translations = forum_topic_translation::Entity::find()
                    .filter(forum_topic_translation::Column::TenantId.eq(tenant_id))
                    .filter(forum_topic_translation::Column::TopicId.eq(topic_id))
                    .all(&self.db)
                    .await?;
                let resolved = resolve_by_locale(&translations, locale, |translation| {
                    translation.locale.as_str()
                });
                if let Some(_translation) = resolved.item {
                    return Ok(Some(ResolvedContentRoute {
                        target_kind: "forum_topic".to_string(),
                        target_id: topic_id,
                        locale: resolved.effective_locale,
                        matched_url: route.to_string(),
                        canonical_url: route.to_string(),
                        redirect_required: false,
                    }));
                }
            }
        }

        let redirects = ForumTopicRouteOwner::find_redirects(&self.db, tenant_id, route)
            .await
            .map_err(forum_error_to_content_error)?;
        let resolved = resolve_by_locale(&redirects, locale, |redirect| redirect.locale.as_str());
        let Some(redirect) = resolved.item else {
            return Ok(None);
        };
        let Some(canonical) = self
            .canonical_for_target(tenant_id, &redirect.target_kind, redirect.target_id)
            .await?
        else {
            return Ok(None);
        };
        Ok(Some(ResolvedContentRoute {
            target_kind: redirect.target_kind.clone(),
            target_id: redirect.target_id,
            locale: resolved.effective_locale,
            matched_url: route.to_string(),
            canonical_url: canonical,
            redirect_required: true,
        }))
    }

    /// Canonical route of a live target, read from its owner. `None` means the
    /// target is missing, and the caller must not redirect to it.
    async fn canonical_for_target(
        &self,
        tenant_id: Uuid,
        target_kind: &str,
        target_id: Uuid,
    ) -> ContentResult<Option<String>> {
        match target_kind {
            "blog_post" => {
                let post = blog_post::Entity::find_by_id(target_id)
                    .filter(blog_post::Column::TenantId.eq(tenant_id))
                    .one(&self.db)
                    .await?;
                match post {
                    Some(post) => Ok(Some(canonical_post_route(&post.slug))),
                    None => missing_target(target_kind, target_id),
                }
            }
            "forum_topic" => {
                let exists = forum_topic::Entity::find_by_id(target_id)
                    .filter(forum_topic::Column::TenantId.eq(tenant_id))
                    .one(&self.db)
                    .await?
                    .is_some();
                if exists {
                    Ok(Some(forum_topic_route(target_id)))
                } else {
                    missing_target(target_kind, target_id)
                }
            }
            other => Err(ContentError::validation(format!(
                "unknown canonical route target kind `{other}`"
            ))),
        }
    }
}

fn missing_target(target_kind: &str, target_id: Uuid) -> ContentResult<Option<String>> {
    tracing::warn!(
        target_kind,
        %target_id,
        "canonical route redirects to a missing target; resolving as not found"
    );
    Ok(None)
}

#[async_trait]
impl CanonicalRouteResolver for OwnerCanonicalRouteResolver {
    async fn resolve_route(
        &self,
        tenant_id: Uuid,
        locale: &str,
        route: &str,
    ) -> ContentResult<Option<ResolvedContentRoute>> {
        let locale = normalize_locale_code(locale)
            .ok_or_else(|| ContentError::validation("locale must not be empty"))?;
        let route = normalize_route(route)?;

        if route.starts_with(BLOG_ROUTE_PREFIX) {
            return self.resolve_blog(tenant_id, &route).await;
        }
        if route.starts_with(FORUM_ROUTE_PREFIX) {
            return self.resolve_forum(tenant_id, &locale, &route).await;
        }
        Ok(None)
    }
}
