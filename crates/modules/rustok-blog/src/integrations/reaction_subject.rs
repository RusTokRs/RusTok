use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{HostRuntimeContext, PortContext};
use rustok_reactions_api::{
    ReactionCatalog, ReactionKey, ReactionProviderError, ReactionProviderResult,
    ReactionSelectionPolicy, ReactionSourceSlug, ReactionSubjectAuthorization, ReactionSubjectKind,
    ReactionSubjectProvider, ReactionSubjectProviderFactory, ReactionSubjectRequest,
};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder};

use crate::entities::{blog_post, blog_post_channel_visibility};
use crate::services::is_post_visible_for_channel;

pub const BLOG_REACTION_SOURCE: &str = "blog";
pub const BLOG_POST_REACTION_KIND: &str = "post";
pub const BLOG_REACTION_V1_KEY: &str = "like";
const BLOG_POST_PUBLISHED_STATUS: &str = "published";

#[derive(Clone, Default)]
pub struct BlogReactionSubjectProviderFactory;

impl ReactionSubjectProviderFactory for BlogReactionSubjectProviderFactory {
    fn source(&self) -> ReactionSourceSlug {
        blog_reaction_source()
    }

    fn build(
        &self,
        host: &HostRuntimeContext,
    ) -> ReactionProviderResult<Arc<dyn ReactionSubjectProvider>> {
        Ok(Arc::new(BlogReactionSubjectProvider::new(host.db_clone())))
    }
}

#[derive(Clone)]
struct BlogReactionSubjectProvider {
    db: DatabaseConnection,
}

impl BlogReactionSubjectProvider {
    fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    async fn authorize_post(
        &self,
        context: &PortContext,
        request: &ReactionSubjectRequest,
    ) -> ReactionProviderResult<ReactionSubjectAuthorization> {
        let subject = &request.subject;
        let Some(post) = blog_post::Entity::find()
            .filter(blog_post::Column::TenantId.eq(subject.tenant_id()))
            .filter(blog_post::Column::Id.eq(subject.subject_id()))
            .one(&self.db)
            .await
            .map_err(database_error)?
        else {
            return Ok(ReactionSubjectAuthorization::Unavailable);
        };

        if post.status != BLOG_POST_PUBLISHED_STATUS {
            return Ok(ReactionSubjectAuthorization::Unavailable);
        }

        let channel_slugs = blog_post_channel_visibility::Entity::find()
            .filter(blog_post_channel_visibility::Column::TenantId.eq(subject.tenant_id()))
            .filter(blog_post_channel_visibility::Column::PostId.eq(post.id))
            .order_by_asc(blog_post_channel_visibility::Column::ChannelSlug)
            .all(&self.db)
            .await
            .map_err(database_error)?
            .into_iter()
            .map(|row| row.channel_slug)
            .collect::<Vec<_>>();
        if !is_post_visible_for_channel(&channel_slugs, context.channel.as_deref()) {
            return Ok(ReactionSubjectAuthorization::Unavailable);
        }

        let current_revision = blog_post_revision(post.version)?;
        if subject.subject_revision() != current_revision {
            return Err(ReactionProviderError::Conflict);
        }

        Ok(ReactionSubjectAuthorization::Allowed {
            canonical_subject: subject.clone(),
            catalog: blog_reaction_catalog_v1()?,
        })
    }
}

#[async_trait]
impl ReactionSubjectProvider for BlogReactionSubjectProvider {
    fn source(&self) -> ReactionSourceSlug {
        blog_reaction_source()
    }

    fn display_name(&self) -> &'static str {
        "Blog"
    }

    fn supported_kinds(&self) -> Vec<ReactionSubjectKind> {
        vec![blog_post_reaction_kind()]
    }

    async fn authorize(
        &self,
        context: PortContext,
        request: ReactionSubjectRequest,
    ) -> ReactionProviderResult<ReactionSubjectAuthorization> {
        request
            .validate()
            .map_err(|_| ReactionProviderError::InvalidRequest)?;
        let tenant_id = uuid::Uuid::parse_str(&context.tenant_id)
            .map_err(|_| ReactionProviderError::InvalidRequest)?;
        if tenant_id != request.subject.tenant_id()
            || request.subject.source().as_str() != BLOG_REACTION_SOURCE
        {
            return Err(ReactionProviderError::InvalidRequest);
        }

        match request.subject.kind().as_str() {
            BLOG_POST_REACTION_KIND => self.authorize_post(&context, &request).await,
            _ => Err(ReactionProviderError::InvalidRequest),
        }
    }
}

fn blog_post_revision(version: i32) -> ReactionProviderResult<u64> {
    u64::try_from(version)
        .ok()
        .filter(|revision| *revision > 0)
        .ok_or(ReactionProviderError::Internal { retryable: false })
}

fn blog_reaction_catalog_v1() -> ReactionProviderResult<ReactionCatalog> {
    ReactionCatalog::try_new(
        ReactionSelectionPolicy::Single,
        vec![
            ReactionKey::new(BLOG_REACTION_V1_KEY)
                .map_err(|_| ReactionProviderError::Internal { retryable: false })?,
        ],
    )
    .map_err(|_| ReactionProviderError::Internal { retryable: false })
}

fn blog_reaction_source() -> ReactionSourceSlug {
    ReactionSourceSlug::new(BLOG_REACTION_SOURCE)
        .expect("Blog reaction source constant must remain valid")
}

fn blog_post_reaction_kind() -> ReactionSubjectKind {
    ReactionSubjectKind::new(BLOG_POST_REACTION_KIND)
        .expect("Blog post reaction kind constant must remain valid")
}

fn database_error(_error: sea_orm::DbErr) -> ReactionProviderError {
    ReactionProviderError::Internal { retryable: true }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blog_catalog_is_single_like() {
        let catalog = blog_reaction_catalog_v1().expect("fixed catalog should be valid");
        assert_eq!(catalog.selection(), ReactionSelectionPolicy::Single);
        assert_eq!(catalog.keys().len(), 1);
        assert_eq!(catalog.keys()[0].as_str(), BLOG_REACTION_V1_KEY);
    }

    #[test]
    fn blog_post_revision_requires_positive_owner_version() {
        assert_eq!(blog_post_revision(1).expect("initial version"), 1);
        assert_eq!(blog_post_revision(42).expect("advanced version"), 42);
        assert!(blog_post_revision(0).is_err());
        assert!(blog_post_revision(-1).is_err());
    }
}
