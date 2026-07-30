use async_trait::async_trait;
use rustok_api::{
    Permission, PortError, PortErrorKind, has_any_effective_permission,
    is_tenant_module_enabled,
};
use rustok_core::SecurityContext;
use rustok_forum::{
    ForumCategoryReadOperation, ForumCategoryReadTransport, ForumError,
    ForumSearchResultCandidate, ForumSearchResultCandidateKind,
    ForumSearchResultEligibilityService, SharedForumAudienceFactsPort,
    category_read_audience_port_context,
};
use rustok_search::{
    FORUM_SEARCH_SOURCE_MODULE, SharedStorefrontSearchResultEligibilityPort,
    StorefrontSearchResultCandidate, StorefrontSearchResultCandidateKind,
    StorefrontSearchResultEligibilityPort, StorefrontSearchResultEligibilityRequest,
    StorefrontSearchTransport,
};
use sea_orm::DatabaseConnection;

pub(crate) struct ServerForumSearchResultEligibilityPort {
    db: DatabaseConnection,
    audience_facts: Option<SharedForumAudienceFactsPort>,
}

impl ServerForumSearchResultEligibilityPort {
    pub(crate) fn shared(
        db: DatabaseConnection,
        audience_facts: Option<SharedForumAudienceFactsPort>,
    ) -> SharedStorefrontSearchResultEligibilityPort {
        std::sync::Arc::new(Self { db, audience_facts })
    }

    fn service(&self) -> ForumSearchResultEligibilityService {
        match self.audience_facts.clone() {
            Some(facts) => {
                ForumSearchResultEligibilityService::with_audience_facts(self.db.clone(), facts)
            }
            None => ForumSearchResultEligibilityService::new(self.db.clone()),
        }
    }
}

#[async_trait]
impl StorefrontSearchResultEligibilityPort for ServerForumSearchResultEligibilityPort {
    async fn filter_forum_result_candidates(
        &self,
        request: StorefrontSearchResultEligibilityRequest,
    ) -> Result<Vec<StorefrontSearchResultCandidate>, PortError> {
        if request.candidates.is_empty() {
            return Ok(Vec::new());
        }
        ensure_forum_enabled(&self.db, request.tenant_id).await?;
        let locale = request.locale.trim();
        if locale.is_empty() {
            return Err(PortError::validation(
                "forum.search_result_eligibility.locale_required",
                "Forum Search result eligibility requires a locale",
            ));
        }
        let forum_candidates = request
            .candidates
            .iter()
            .copied()
            .map(to_forum_candidate)
            .collect::<Vec<_>>();
        let service = self.service();
        let allowed = if let Some(auth) = request.auth.as_ref().filter(|auth| {
            has_any_effective_permission(
                &auth.permissions,
                &[Permission::FORUM_CATEGORIES_LIST],
            )
        }) {
            let transport = match request.transport {
                StorefrontSearchTransport::Graphql => ForumCategoryReadTransport::Graphql,
                StorefrontSearchTransport::NativeServer => {
                    ForumCategoryReadTransport::NativeServer
                }
            };
            let context = category_read_audience_port_context(
                transport,
                ForumCategoryReadOperation::SearchResultEligibility,
                request.tenant_id,
                auth,
                request.request_context.as_ref(),
                locale,
            )
            .map_err(map_forum_error)?;
            let security = SecurityContext::from_permission_snapshot(
                Some(auth.user_id),
                &auth.permissions,
            );
            service
                .filter_authenticated_storefront_visible(
                    request.tenant_id,
                    security,
                    context,
                    &forum_candidates,
                )
                .await
        } else {
            let channel_slug = request
                .request_context
                .as_ref()
                .and_then(|context| context.channel_slug.as_deref());
            service
                .filter_public_storefront_visible(
                    request.tenant_id,
                    channel_slug,
                    &forum_candidates,
                )
                .await
        }
        .map_err(map_forum_error)?;

        Ok(allowed.into_iter().map(from_forum_candidate).collect())
    }
}

async fn ensure_forum_enabled(
    db: &DatabaseConnection,
    tenant_id: uuid::Uuid,
) -> Result<(), PortError> {
    let enabled = is_tenant_module_enabled(db, tenant_id, FORUM_SEARCH_SOURCE_MODULE)
        .await
        .map_err(|error| {
            tracing::error!(
                tenant_id = %tenant_id,
                error = ?error,
                "Failed to resolve tenant Forum module state for Search result eligibility"
            );
            PortError::unavailable(
                "forum.search_result_eligibility.module_state_unavailable",
                "Forum Search result eligibility is temporarily unavailable",
            )
        })?;
    if enabled {
        Ok(())
    } else {
        Err(PortError::not_found(
            "forum.search_result_eligibility.unavailable",
            "Forum Search result eligibility is unavailable",
        ))
    }
}

fn to_forum_candidate(candidate: StorefrontSearchResultCandidate) -> ForumSearchResultCandidate {
    ForumSearchResultCandidate {
        document_id: candidate.document_id,
        kind: match candidate.kind {
            StorefrontSearchResultCandidateKind::ForumTopic => {
                ForumSearchResultCandidateKind::Topic
            }
            StorefrontSearchResultCandidateKind::ForumReply => {
                ForumSearchResultCandidateKind::Reply
            }
        },
    }
}

fn from_forum_candidate(candidate: ForumSearchResultCandidate) -> StorefrontSearchResultCandidate {
    StorefrontSearchResultCandidate {
        document_id: candidate.document_id,
        kind: match candidate.kind {
            ForumSearchResultCandidateKind::Topic => {
                StorefrontSearchResultCandidateKind::ForumTopic
            }
            ForumSearchResultCandidateKind::Reply => {
                StorefrontSearchResultCandidateKind::ForumReply
            }
        },
    }
}

fn map_forum_error(error: ForumError) -> PortError {
    let stable_code = error.stable_code().to_ascii_lowercase();
    let retryable = error.is_retryable();
    let public_message = error.to_string();

    match error {
        ForumError::Validation(message) => PortError::validation(stable_code, message),
        ForumError::CategoryNotFound(_)
        | ForumError::TopicNotFound(_)
        | ForumError::ReplyNotFound(_) => PortError::not_found(
            stable_code,
            "Forum Search result is unavailable",
        ),
        ForumError::Forbidden(_) => PortError::forbidden(
            stable_code,
            "Forum Search result is unavailable",
        ),
        ForumError::CapabilityUnavailable { .. }
        | ForumError::CapabilityFailure { .. }
        | ForumError::Database(_)
        | ForumError::Content(_)
        | ForumError::Internal(_) => PortError::new(
            PortErrorKind::Unavailable,
            stable_code,
            public_message,
            retryable,
        ),
        _ => PortError::invariant_violation(
            stable_code,
            "Forum Search result eligibility could not be resolved safely",
        ),
    }
}
