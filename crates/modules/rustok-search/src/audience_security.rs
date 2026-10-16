/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{AuthContext, PortError, RequestContext};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::storefront_category_scope::StorefrontSearchTransport;
use crate::storefront_result_eligibility::{
    StorefrontSearchResultCandidate, StorefrontSearchResultCandidateKind,
    StorefrontSearchResultEligibilityPort, StorefrontSearchResultEligibilityRequest,
};

pub const MAX_SEARCH_AUDIENCE_CANDIDATES: usize = 100;

/// Neutral candidate descriptor representing one document returned by search engine.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SearchCandidateDescriptor {
    pub document_id: Uuid,
    pub source_module: String,
    pub entity_type: String,
}

impl SearchCandidateDescriptor {
    pub fn new(
        document_id: Uuid,
        source_module: impl Into<String>,
        entity_type: impl Into<String>,
    ) -> Self {
        Self {
            document_id,
            source_module: source_module.into(),
            entity_type: entity_type.into(),
        }
    }
}

/// Request passed to a domain owner to evaluate real-time candidate eligibility.
#[derive(Clone, Debug)]
pub struct SearchAudienceRequest {
    pub tenant_id: Uuid,
    pub locale: String,
    pub candidates: Vec<SearchCandidateDescriptor>,
    pub auth: Option<AuthContext>,
    pub request_context: Option<RequestContext>,
    pub transport: StorefrontSearchTransport,
}

/// Generic Service Provider Interface (SPI) for module-owned search audience security.
///
/// Any domain module with audience, private categories, or dynamic visibility
/// implements this port without requiring changes to `rustok-search`.
#[async_trait]
pub trait SearchAudiencePolicyPort: Send + Sync {
    /// The source module this audience policy applies to (e.g. "forum", "events", "courses").
    fn source_module(&self) -> &'static str;

    /// 1. Preflight discovery gate.
    /// Evaluated before running full-text search. If false, public search skips this source module
    /// before touching the index, preventing timing and facet oracle leaks.
    async fn public_discovery_allowed(&self, _tenant_id: Uuid) -> Result<bool, PortError> {
        Ok(true)
    }

    /// 2. Real-time batch candidate post-authorization.
    /// Evaluates raw candidate IDs against current domain business rules and returns
    /// the subset of allowed document IDs.
    async fn filter_eligible_candidates(
        &self,
        request: SearchAudienceRequest,
    ) -> Result<Vec<Uuid>, PortError>;
}

pub type SharedSearchAudiencePolicyPort = Arc<dyn SearchAudiencePolicyPort>;

/// Registry holding active audience policy providers for registered source modules.
#[derive(Clone, Default)]
pub struct SearchAudienceRegistry {
    policies: BTreeMap<String, SharedSearchAudiencePolicyPort>,
}

impl SearchAudienceRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register<P>(&mut self, policy: P) -> Result<(), PortError>
    where
        P: SearchAudiencePolicyPort + 'static,
    {
        let source_module = normalize_source_module(policy.source_module())?;
        if self.policies.contains_key(&source_module) {
            return Err(PortError::validation(
                "search.audience_registry.duplicate_source_module",
                format!("Search audience policy `{source_module}` is already registered"),
            ));
        }
        self.policies.insert(source_module, Arc::new(policy));
        Ok(())
    }

    pub fn with_policy<P>(mut self, policy: P) -> Self
    where
        P: SearchAudiencePolicyPort + 'static,
    {
        let source_module = normalize_source_module(policy.source_module())
            .expect("valid source module for search audience policy");
        self.policies.insert(source_module, Arc::new(policy));
        self
    }

    pub fn get(&self, source_module: &str) -> Option<SharedSearchAudiencePolicyPort> {
        let source_module = normalize_source_module(source_module).ok()?;
        self.policies.get(&source_module).cloned()
    }

    pub fn source_modules(&self) -> Vec<String> {
        self.policies.keys().cloned().collect()
    }
}

/// Backward-compatibility adapter that adapts the legacy `StorefrontSearchResultEligibilityPort`
/// into the generic `SearchAudiencePolicyPort`.
pub struct LegacyForumAudienceAdapter {
    port: Arc<dyn StorefrontSearchResultEligibilityPort>,
}

impl LegacyForumAudienceAdapter {
    pub fn new(port: Arc<dyn StorefrontSearchResultEligibilityPort>) -> Self {
        Self { port }
    }
}

#[async_trait]
impl SearchAudiencePolicyPort for LegacyForumAudienceAdapter {
    fn source_module(&self) -> &'static str {
        "forum"
    }

    async fn public_discovery_allowed(&self, tenant_id: Uuid) -> Result<bool, PortError> {
        self.port.public_forum_discovery_allowed(tenant_id).await
    }

    async fn filter_eligible_candidates(
        &self,
        request: SearchAudienceRequest,
    ) -> Result<Vec<Uuid>, PortError> {
        let forum_candidates = request
            .candidates
            .into_iter()
            .filter_map(|candidate| {
                let kind = StorefrontSearchResultCandidateKind::from_entity_type(
                    &candidate.entity_type,
                )?;
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

/// Evaluates candidates across all registered audience policies.
/// Candidates belonging to modules without registered audience policies pass through as public.
pub async fn resolve_audience_eligible_candidates(
    registry: &SearchAudienceRegistry,
    tenant_id: Uuid,
    locale: &str,
    candidates: &[SearchCandidateDescriptor],
    auth: Option<AuthContext>,
    request_context: Option<RequestContext>,
    transport: StorefrontSearchTransport,
) -> Result<HashSet<Uuid>, PortError> {
    if candidates.is_empty() {
        return Ok(HashSet::new());
    }

    let mut allowed_ids = HashSet::with_capacity(candidates.len());
    let mut partitioned: HashMap<String, Vec<SearchCandidateDescriptor>> = HashMap::new();

    for candidate in candidates {
        partitioned
            .entry(candidate.source_module.clone())
            .or_default()
            .push(candidate.clone());
    }

    for (source_module, module_candidates) in partitioned {
        if let Some(policy) = registry.get(&source_module) {
            if module_candidates.len() > MAX_SEARCH_AUDIENCE_CANDIDATES {
                return Err(PortError::validation(
                    "search.audience.candidate_limit_exceeded",
                    format!(
                        "Search audience policy for `{source_module}` accepts at most {MAX_SEARCH_AUDIENCE_CANDIDATES} candidates"
                    ),
                ));
            }

            let request = SearchAudienceRequest {
                tenant_id,
                locale: locale.to_string(),
                candidates: module_candidates,
                auth: auth.clone(),
                request_context: request_context.clone(),
                transport,
            };

            let module_allowed = policy.filter_eligible_candidates(request).await?;
            allowed_ids.extend(module_allowed);
        } else {
            // Unconstrained public source modules (e.g. blog, content, pages) are public by default.
            for candidate in module_candidates {
                allowed_ids.insert(candidate.document_id);
            }
        }
    }

    Ok(allowed_ids)
}

fn normalize_source_module(value: &str) -> Result<String, PortError> {
    let normalized = value.trim().to_ascii_lowercase();
    if normalized.is_empty()
        || normalized.len() > 64
        || !normalized.chars().all(|ch| {
            ch.is_ascii_lowercase() || ch.is_ascii_digit() || matches!(ch, '-' | '_')
        })
    {
        return Err(PortError::validation(
            "search.audience_registry.invalid_source_module",
            format!("Invalid Search audience source module `{value}`"),
        ));
    }
    Ok(normalized)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockEventsPolicy {
        allowed_id: Uuid,
    }

    #[async_trait]
    impl SearchAudiencePolicyPort for MockEventsPolicy {
        fn source_module(&self) -> &'static str {
            "events"
        }

        async fn public_discovery_allowed(&self, _tenant_id: Uuid) -> Result<bool, PortError> {
            Ok(true)
        }

        async fn filter_eligible_candidates(
            &self,
            request: SearchAudienceRequest,
        ) -> Result<Vec<Uuid>, PortError> {
            Ok(request
                .candidates
                .into_iter()
                .filter(|c| c.document_id == self.allowed_id)
                .map(|c| c.document_id)
                .collect())
        }
    }

    #[tokio::test]
    async fn multi_module_candidate_authorization() {
        let allowed_event = Uuid::new_v4();
        let denied_event = Uuid::new_v4();
        let blog_post = Uuid::new_v4();

        let mut registry = SearchAudienceRegistry::new();
        registry
            .register(MockEventsPolicy {
                allowed_id: allowed_event,
            })
            .unwrap();

        let candidates = vec![
            SearchCandidateDescriptor::new(allowed_event, "events", "event_announcement"),
            SearchCandidateDescriptor::new(denied_event, "events", "event_announcement"),
            SearchCandidateDescriptor::new(blog_post, "blog", "blog_post"),
        ];

        let allowed = resolve_audience_eligible_candidates(
            &registry,
            Uuid::new_v4(),
            "en",
            &candidates,
            None,
            None,
            StorefrontSearchTransport::Graphql,
        )
        .await
        .expect("authorized candidates");

        assert!(allowed.contains(&allowed_event));
        assert!(!allowed.contains(&denied_event));
        assert!(allowed.contains(&blog_post)); // Blog has no policy, so it passes as public
    }
}
