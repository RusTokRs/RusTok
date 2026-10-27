use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{AuthContext, PortError, RequestContext};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::storefront_category_scope::StorefrontSearchTransport;

pub const MAX_FORUM_SEARCH_RESULT_CANDIDATES: usize = 100;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StorefrontSearchResultCandidateKind {
    ForumCategory,
    ForumTopic,
    ForumReply,
}

impl StorefrontSearchResultCandidateKind {
    pub fn from_entity_type(value: &str) -> Option<Self> {
        match value {
            "forum_category" => Some(Self::ForumCategory),
            "forum_topic" => Some(Self::ForumTopic),
            "forum_reply" => Some(Self::ForumReply),
            _ => None,
        }
    }

    pub fn entity_type(&self) -> &'static str {
        match self {
            Self::ForumCategory => "forum_category",
            Self::ForumTopic => "forum_topic",
            Self::ForumReply => "forum_reply",
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

#[deprecated(
    since = "0.2.0",
    note = "Implement SearchAudiencePolicyPort and register in SearchAudienceRegistry instead."
)]
#[async_trait]
pub trait StorefrontSearchResultEligibilityPort: Send + Sync {
    /// Owner gate before raw candidate collection. A private tenant with more
    /// than 100 stale indexed hits should return an empty public page, not a
    /// candidate-limit error (and must not execute query rules on raw hits).
    /// No implicit allow: ports that do not support this preflight fail closed.
    async fn public_forum_discovery_allowed(&self, _tenant_id: Uuid) -> Result<bool, PortError> {
        Err(PortError::unavailable(
            "forum.search_result_eligibility.public_discovery_unavailable",
            "Forum public discovery is temporarily unavailable",
        ))
    }

    async fn filter_forum_result_candidates(
        &self,
        request: StorefrontSearchResultEligibilityRequest,
    ) -> Result<Vec<StorefrontSearchResultCandidate>, PortError>;
}

#[allow(deprecated)]
pub type SharedStorefrontSearchResultEligibilityPort =
    Arc<dyn StorefrontSearchResultEligibilityPort>;

#[deprecated(
    since = "0.2.0",
    note = "Use resolve_audience_eligible_candidates with SearchAudienceRegistry instead."
)]
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
    let candidate_map: HashMap<Uuid, StorefrontSearchResultCandidate> = request
        .candidates
        .iter()
        .map(|c| (c.document_id, *c))
        .collect();

    let adapter = StorefrontEligibilityAudiencePolicyAdapter { port };
    let mut registry = crate::audience_security::SearchAudienceRegistry::new();
    registry.register(adapter).map_err(|e| {
        PortError::invariant_violation(
            "forum.search_result_eligibility.registry_error",
            e.to_string(),
        )
    })?;

    let descriptors = request
        .candidates
        .iter()
        .map(|c| {
            crate::audience_security::SearchCandidateDescriptor::new(
                c.document_id,
                crate::FORUM_SEARCH_SOURCE_MODULE,
                c.kind.entity_type(),
            )
        })
        .collect::<Vec<_>>();

    let allowed_ids = crate::audience_security::resolve_audience_eligible_candidates(
        &registry,
        request.tenant_id,
        &request.locale,
        &descriptors,
        request.auth,
        request.request_context,
        request.transport,
    )
    .await
    .map_err(|e| {
        if e.code == "search.audience.owner_limit_exceeded" {
            PortError::invariant_violation(
                "forum.search_result_eligibility.owner_limit_exceeded",
                "Forum Search result eligibility owner exceeded the candidate limit",
            )
        } else if e.code == "search.audience.unrequested_candidate"
            || e.code == "search.audience.owner_scope_invalid"
            || e.code == "search.audience.duplicate_candidate"
        {
            PortError::invariant_violation(
                "forum.search_result_eligibility.owner_scope_invalid",
                "Forum Search result eligibility owner returned an invalid candidate scope",
            )
        } else {
            e
        }
    })?;

    let mut allowed = Vec::with_capacity(allowed_ids.len());
    let mut seen = HashSet::new();
    for id in &allowed_ids {
        if let Some(candidate) = candidate_map.get(id) {
            if !requested.contains(candidate) || !seen.insert(*candidate) {
                return Err(PortError::invariant_violation(
                    "forum.search_result_eligibility.owner_scope_invalid",
                    "Forum Search result eligibility owner returned an invalid candidate scope",
                ));
            }
            allowed.push(*candidate);
        } else {
            return Err(PortError::invariant_violation(
                "forum.search_result_eligibility.owner_scope_invalid",
                "Forum Search result eligibility owner returned an invalid candidate scope",
            ));
        }
    }
    Ok(allowed)
}

struct StorefrontEligibilityAudiencePolicyAdapter {
    port: SharedStorefrontSearchResultEligibilityPort,
}

#[allow(deprecated)]
#[async_trait]
impl crate::audience_security::SearchAudiencePolicyPort
    for StorefrontEligibilityAudiencePolicyAdapter
{
    fn source_module(&self) -> &'static str {
        crate::storefront_category_scope::FORUM_SEARCH_SOURCE_MODULE
    }

    async fn public_discovery_allowed(&self, tenant_id: Uuid) -> Result<bool, PortError> {
        self.port.public_forum_discovery_allowed(tenant_id).await
    }

    async fn filter_eligible_candidates(
        &self,
        request: crate::audience_security::SearchAudienceRequest,
    ) -> Result<Vec<Uuid>, PortError> {
        let forum_candidates = request
            .candidates
            .into_iter()
            .filter_map(|candidate| {
                let kind =
                    StorefrontSearchResultCandidateKind::from_entity_type(&candidate.entity_type)?;
                Some(StorefrontSearchResultCandidate {
                    document_id: candidate.document_id,
                    kind,
                })
            })
            .collect();

        let allowed = self
            .port
            .filter_forum_result_candidates(StorefrontSearchResultEligibilityRequest {
                tenant_id: request.tenant_id,
                locale: request.locale,
                candidates: forum_candidates,
                auth: request.auth,
                request_context: request.request_context,
                transport: request.transport,
            })
            .await?;

        Ok(allowed.into_iter().map(|item| item.document_id).collect())
    }
}

#[cfg(test)]
#[allow(deprecated)]
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
