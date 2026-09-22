use std::collections::{HashMap, HashSet};

use rustok_api::PortContext;
use rustok_core::SecurityContext;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use uuid::Uuid;

use crate::audience::SharedForumAudienceFactsPort;
use crate::entities::forum_reply;
use crate::error::{ForumError, ForumResult};
use crate::state_machine::ReplyStatus;

use super::topic_audience_visibility::{
    ForumTopicAudienceViewer, ForumTopicAudienceVisibilityService,
};

pub const MAX_FORUM_SEARCH_RESULT_ELIGIBILITY_CANDIDATES: usize = 100;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ForumSearchResultCandidateKind {
    Topic,
    Reply,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ForumSearchResultCandidate {
    pub document_id: Uuid,
    pub kind: ForumSearchResultCandidateKind,
}

/// Exact Forum owner for topic/reply Search result eligibility.
///
/// Reply candidates are revalidated as currently approved and inherit the exact
/// storefront visibility decision of their parent topic. Missing, stale, closed,
/// route-channel-denied, category-denied, and richer-audience-denied candidates
/// are omitted without exposing which owner predicate rejected them.
pub struct ForumSearchResultEligibilityService {
    db: DatabaseConnection,
    visibility: ForumTopicAudienceVisibilityService,
}

impl ForumSearchResultEligibilityService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self::with_optional_audience_facts(db, None)
    }

    pub fn with_audience_facts(
        db: DatabaseConnection,
        facts_port: SharedForumAudienceFactsPort,
    ) -> Self {
        Self::with_optional_audience_facts(db, Some(facts_port))
    }

    fn with_optional_audience_facts(
        db: DatabaseConnection,
        facts_port: Option<SharedForumAudienceFactsPort>,
    ) -> Self {
        Self {
            visibility: ForumTopicAudienceVisibilityService::new(db.clone(), facts_port),
            db,
        }
    }

    pub async fn filter_public_storefront_visible(
        &self,
        tenant_id: Uuid,
        channel_slug: Option<&str>,
        candidates: &[ForumSearchResultCandidate],
    ) -> ForumResult<Vec<ForumSearchResultCandidate>> {
        self.filter_visible(
            tenant_id,
            ForumTopicAudienceViewer::public(),
            channel_slug,
            candidates,
        )
        .await
    }

    pub async fn filter_authenticated_storefront_visible(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        context: PortContext,
        candidates: &[ForumSearchResultCandidate],
    ) -> ForumResult<Vec<ForumSearchResultCandidate>> {
        let channel_slug = context.channel.clone();
        let viewer = ForumTopicAudienceViewer::authenticated(security, context)?;
        self.filter_visible(tenant_id, viewer, channel_slug.as_deref(), candidates)
            .await
    }

    async fn filter_visible(
        &self,
        tenant_id: Uuid,
        viewer: ForumTopicAudienceViewer,
        channel_slug: Option<&str>,
        candidates: &[ForumSearchResultCandidate],
    ) -> ForumResult<Vec<ForumSearchResultCandidate>> {
        validate_candidates(tenant_id, candidates)?;
        if candidates.is_empty() {
            return Ok(Vec::new());
        }

        let reply_ids = candidates
            .iter()
            .filter_map(|candidate| match candidate.kind {
                ForumSearchResultCandidateKind::Reply => Some(candidate.document_id),
                ForumSearchResultCandidateKind::Topic => None,
            })
            .collect::<HashSet<_>>();
        let reply_topics = self
            .load_approved_reply_topics(tenant_id, &reply_ids)
            .await?;

        let mut topic_ids = candidates
            .iter()
            .filter_map(|candidate| match candidate.kind {
                ForumSearchResultCandidateKind::Topic => Some(candidate.document_id),
                ForumSearchResultCandidateKind::Reply => {
                    reply_topics.get(&candidate.document_id).copied()
                }
            })
            .collect::<HashSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        topic_ids.sort_unstable();

        let mut visible_topics = HashSet::with_capacity(topic_ids.len());
        for topic_id in topic_ids {
            if self
                .visibility
                .is_topic_visible(tenant_id, topic_id, channel_slug, &viewer)
                .await?
            {
                visible_topics.insert(topic_id);
            }
        }

        let mut allowed = Vec::new();
        let mut seen = HashSet::new();
        for candidate in candidates {
            let is_allowed = match candidate.kind {
                ForumSearchResultCandidateKind::Topic => {
                    visible_topics.contains(&candidate.document_id)
                }
                ForumSearchResultCandidateKind::Reply => reply_topics
                    .get(&candidate.document_id)
                    .is_some_and(|topic_id| visible_topics.contains(topic_id)),
            };
            if is_allowed && seen.insert(*candidate) {
                allowed.push(*candidate);
            }
        }
        Ok(allowed)
    }

    async fn load_approved_reply_topics(
        &self,
        tenant_id: Uuid,
        reply_ids: &HashSet<Uuid>,
    ) -> ForumResult<HashMap<Uuid, Uuid>> {
        if reply_ids.is_empty() {
            return Ok(HashMap::new());
        }
        let rows = forum_reply::Entity::find()
            .filter(forum_reply::Column::TenantId.eq(tenant_id))
            .filter(forum_reply::Column::Id.is_in(reply_ids.iter().copied()))
            .filter(forum_reply::Column::Status.eq(ReplyStatus::Approved))
            .all(&self.db)
            .await
            .map_err(ForumError::from)?;

        Ok(rows
            .into_iter()
            .map(|reply| (reply.id, reply.topic_id))
            .collect())
    }
}

fn validate_candidates(
    tenant_id: Uuid,
    candidates: &[ForumSearchResultCandidate],
) -> ForumResult<()> {
    if tenant_id.is_nil() {
        return Err(ForumError::Validation(
            "Forum Search result eligibility requires a tenant".to_string(),
        ));
    }
    if candidates.len() > MAX_FORUM_SEARCH_RESULT_ELIGIBILITY_CANDIDATES {
        return Err(ForumError::Validation(format!(
            "Forum Search result eligibility accepts at most {MAX_FORUM_SEARCH_RESULT_ELIGIBILITY_CANDIDATES} candidates"
        )));
    }
    if candidates
        .iter()
        .any(|candidate| candidate.document_id.is_nil())
    {
        return Err(ForumError::Validation(
            "Forum Search result eligibility requires non-nil document identifiers".to_string(),
        ));
    }
    Ok(())
}
