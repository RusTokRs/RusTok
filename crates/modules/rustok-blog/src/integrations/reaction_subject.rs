use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{HostRuntimeContext, PortContext};
use rustok_reactions_api::{
    ReactionCatalog, ReactionKey, ReactionProviderError, ReactionProviderResult,
    ReactionSelectionPolicy, ReactionSourceSlug, ReactionSubjectAuthorization, ReactionSubjectKind,
    ReactionSubjectProvider, ReactionSubjectProviderFactory, ReactionSubjectRequest,
};
use sea_orm::DatabaseConnection;

use crate::BlogPostStatus;
use crate::services::{is_post_visible_for_channel, load_post_subject_snapshot};

pub const BLOG_REACTION_SOURCE: &str = "blog";
pub const BLOG_POST_REACTION_KIND: &str = "post";
pub const BLOG_REACTION_V1_KEY: &str = "like";

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
        let Some(snapshot) = load_post_subject_snapshot(
            &self.db,
            subject.tenant_id(),
            subject.subject_id(),
        )
        .await
        .map_err(owner_read_error)?
        else {
            return Ok(ReactionSubjectAuthorization::Unavailable);
        };

        if snapshot.status != BlogPostStatus::Published
            || !is_post_visible_for_channel(&snapshot.channel_slugs, context.channel.as_deref())
        {
            return Ok(ReactionSubjectAuthorization::Unavailable);
        }

        let current_revision = blog_post_revision(snapshot.version)?;
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

fn owner_read_error(_error: crate::BlogError) -> ReactionProviderError {
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
