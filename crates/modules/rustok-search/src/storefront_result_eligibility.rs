use std::collections::HashSet;
use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{AuthContext, PortError, RequestContext};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::storefront_category_scope::StorefrontSearchTransport;

pub const MAX_FORUM_SEARCH_RESULT_CANDIDATES: usize = 100;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StorefrontSearchResultCandidateKind {
    ForumTopic,
    ForumReply,
}

impl StorefrontSearchResultCandidateKind {
    pub fn from_entity_type(value: &str) -> Option<Self> {
        match value {
            "forum_topic" => Some(Self::ForumTopic),
            "forum_reply" => Some(Self::ForumReply),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct StorefrontSearchResultCandidate {
    pub document_id: Uuid,
    pub kind: StorefrontSearchResultCandidateKind,
}

#[derive(Clone)]
pub struct StorefrontSearchResultEligibilityRequest {
    pub tenant_id: Uuid,
    pub locale: String,
    pub candidates: Vec<StorefrontSearchResultCandidate>,
    pub auth: Option<AuthContext>,
    pub request_context: Option<RequestContext>,
    pub transport: StorefrontSearchTransport,
}

#[async_trait]
pub trait StorefrontSearchResultEligibilityPort: Send + Sync {
    async fn filter_forum_result_candidates(
        &self,
        request: StorefrontSearchResultEligibilityRequest,
    ) -> Result<Vec<StorefrontSearchResultCandidate>, PortError>;
}

pub type SharedStorefrontSearchResultEligibilityPort =
    Arc<dyn StorefrontSearchResultEligibilityPort>;

pub async fn resolve_storefront_search_result_candidates(
    port: Option<SharedStorefrontSearchResultEligibilityPort>,
    request: StorefrontSearchResultEligibilityRequest,
) -> Result<Vec<StorefrontSearchResultCandidate>, PortError> {
    if request.candidates.len() > MAX_FORUM_SEARCH_RESULT_CANDIDATES {
        return Err(PortError::validation(
            "forum.search_result_eligibility.candidate_limit_exceeded",
            format!(
                "Forum Search result eligibility accepts at most {MAX_FORUM_SEARCH_RESULT_CANDIDATES} candidates"
            ),
        ));
    }
    let port = port.ok_or_else(|| {
        PortError::unavailable(
            "forum.search_result_eligibility.owner_unavailable",
            "Forum Search result eligibility is temporarily unavailable",
        )
    })?;
    if request.candidates.is_empty() {
        return Ok(Vec::new());
    }
    if request.tenant_id.is_nil() {
        return Err(PortError::validation(
            "forum.search_result_eligibility.tenant_required",
            "Forum Search result eligibility requires a tenant",
        ));
    }
    if request.locale.trim().is_empty() {
        return Err(PortError::validation(
            "forum.search_result_eligibility.locale_required",
            "Forum Search result eligibility requires a locale",
        ));
    }
    if request
        .candidates
        .iter()
        .any(|candidate| candidate.document_id.is_nil())
    {
        return Err(PortError::validation(
            "forum.search_result_eligibility.document_id_required",
            "Forum Search result eligibility requires non-nil document identifiers",
        ));
    }

    let requested = request.candidates.iter().copied().collect::<HashSet<_>>();
    let allowed = port.filter_forum_result_candidates(request).await?;
    if allowed.len() > MAX_FORUM_SEARCH_RESULT_CANDIDATES {
        return Err(PortError::invariant_violation(
            "forum.search_result_eligibility.owner_limit_exceeded",
            "Forum Search result eligibility owner exceeded the candidate limit",
        ));
    }

    let mut seen = HashSet::new();
    for candidate in &allowed {
        if !requested.contains(candidate) || !seen.insert(*candidate) {
            return Err(PortError::invariant_violation(
                "forum.search_result_eligibility.owner_scope_invalid",
                "Forum Search result eligibility owner returned an invalid candidate scope",
            ));
        }
    }
    Ok(allowed)
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    use super::*;

    struct CountingPort {
        calls: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl StorefrontSearchResultEligibilityPort for CountingPort {
        async fn filter_forum_result_candidates(
            &self,
            request: StorefrontSearchResultEligibilityRequest,
        ) -> Result<Vec<StorefrontSearchResultCandidate>, PortError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(request.candidates)
        }
    }

    fn candidate(kind: StorefrontSearchResultCandidateKind) -> StorefrontSearchResultCandidate {
        StorefrontSearchResultCandidate {
            document_id: Uuid::new_v4(),
            kind,
        }
    }

    fn request(
        candidates: Vec<StorefrontSearchResultCandidate>,
    ) -> StorefrontSearchResultEligibilityRequest {
        StorefrontSearchResultEligibilityRequest {
            tenant_id: Uuid::new_v4(),
            locale: "en".to_string(),
            candidates,
            auth: None,
            request_context: None,
            transport: StorefrontSearchTransport::Graphql,
        }
    }

    #[tokio::test]
    async fn non_empty_scope_requires_owner_port() {
        let error = resolve_storefront_search_result_candidates(
            None,
            request(vec![candidate(
                StorefrontSearchResultCandidateKind::ForumTopic,
            )]),
        )
        .await
        .expect_err("explicit Forum scope must fail closed without the owner");

        assert_eq!(error.kind, rustok_api::PortErrorKind::Unavailable);
    }

    #[tokio::test]
    async fn empty_scope_still_requires_owner_composition() {
        let error = resolve_storefront_search_result_candidates(None, request(Vec::new()))
            .await
            .expect_err("explicit Forum scope must require its owner even without candidates");

        assert_eq!(error.kind, rustok_api::PortErrorKind::Unavailable);
    }

    #[tokio::test]
    async fn empty_scope_skips_owner_call() {
        let calls = Arc::new(AtomicUsize::new(0));
        let port: SharedStorefrontSearchResultEligibilityPort = Arc::new(CountingPort {
            calls: calls.clone(),
        });

        let allowed = resolve_storefront_search_result_candidates(Some(port), request(Vec::new()))
            .await
            .expect("empty candidate scope should remain empty");

        assert!(allowed.is_empty());
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }
}
