use async_trait::async_trait;
use rustok_api::{PortError, PortErrorKind, SharedStaticModuleSettingsReader, is_tenant_module_enabled};
use rustok_core::SecurityContext;
use rustok_forum::{
    ForumCategoryReadOperation, ForumCategoryReadTransport, ForumError, ForumSearchResultCandidate,
    ForumSearchResultCandidateKind, ForumSearchResultEligibilityService, ForumSettingsProviders,
    SharedForumAudienceFactsPort, category_read_audience_port_context,
};
use rustok_search::{
    FORUM_SEARCH_SOURCE_MODULE, SearchAudiencePolicyPort, SearchAudienceRequest,
    StorefrontSearchTransport,
};
use sea_orm::DatabaseConnection;

#[derive(Clone)]
pub(crate) struct ServerForumSearchResultEligibilityPort {
    db: DatabaseConnection,
    audience_facts: Option<SharedForumAudienceFactsPort>,
    settings_reader: Option<SharedStaticModuleSettingsReader>,
}

impl ServerForumSearchResultEligibilityPort {
    pub(crate) fn new(
        db: DatabaseConnection,
        audience_facts: Option<SharedForumAudienceFactsPort>,
        settings_reader: Option<SharedStaticModuleSettingsReader>,
    ) -> Self {
        Self { db, audience_facts, settings_reader }
    }

    fn service(&self) -> ForumSearchResultEligibilityService {
        let service = match self.audience_facts.clone() {
            Some(facts) => {
                ForumSearchResultEligibilityService::with_audience_facts(self.db.clone(), facts)
            }
            None => ForumSearchResultEligibilityService::new(self.db.clone()),
        };
        match self.settings_reader.clone() {
            Some(reader) => service.with_settings_providers(
                ForumSettingsProviders::default().with_static_reader(reader),
            ),
            None => service,
        }
    }

    pub(crate) async fn public_forum_discovery_allowed(
        &self,
        tenant_id: uuid::Uuid,
    ) -> Result<bool, PortError> {
        ensure_forum_enabled(&self.db, tenant_id).await?;
        self.service()
            .public_discovery_allowed(tenant_id)
            .await
            .map_err(map_forum_error)
    }

    pub(crate) async fn filter_forum_result_candidates(
        &self,
        request: SearchAudienceRequest,
    ) -> Result<Vec<uuid::Uuid>, PortError> {
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
        if request
            .auth
            .as_ref()
            .is_some_and(|auth| auth.tenant_id != request.tenant_id)
        {
            return Err(PortError::validation(
                "forum.search_result_eligibility.auth_tenant_mismatch",
                "Forum Search result eligibility auth tenant does not match the request",
            ));
        }
        if let Some(context) = request.request_context.as_ref() {
            if context.tenant_id != request.tenant_id {
                return Err(PortError::validation(
                    "forum.search_result_eligibility.request_tenant_mismatch",
                    "Forum Search result eligibility request tenant does not match the request",
                ));
            }
            if let Some(auth) = request.auth.as_ref()
                && context.user_id != Some(auth.user_id)
            {
                return Err(PortError::validation(
                    "forum.search_result_eligibility.request_actor_mismatch",
                    "Forum Search result eligibility request actor does not match auth",
                ));
            }
        }
        let forum_candidates = request
            .candidates
            .iter()
            .filter_map(to_forum_candidate)
            .collect::<Vec<_>>();
        let service = self.service();
        // Do not downgrade an authenticated Search caller to the public audience
        // merely because its Forum permission snapshot is narrower.
        let allowed = if let Some(auth) = request.auth.as_ref() {
            let transport = match request.transport {
                StorefrontSearchTransport::Graphql => ForumCategoryReadTransport::Graphql,
                StorefrontSearchTransport::NativeServer => ForumCategoryReadTransport::NativeServer,
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
            let security =
                SecurityContext::from_permission_snapshot(Some(auth.user_id), &auth.permissions);
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

        Ok(allowed
            .into_iter()
            .map(|candidate| candidate.document_id)
            .collect())
    }
}

#[async_trait]
impl SearchAudiencePolicyPort for ServerForumSearchResultEligibilityPort {
    fn source_module(&self) -> &'static str {
        FORUM_SEARCH_SOURCE_MODULE
    }

    async fn public_discovery_allowed(&self, tenant_id: uuid::Uuid) -> Result<bool, PortError> {
        self.public_forum_discovery_allowed(tenant_id).await
    }

    async fn filter_eligible_candidates(
        &self,
        request: SearchAudienceRequest,
    ) -> Result<Vec<uuid::Uuid>, PortError> {
        self.filter_forum_result_candidates(request).await
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

fn to_forum_candidate(
    candidate: &rustok_search::SearchCandidateDescriptor,
) -> Option<ForumSearchResultCandidate> {
    let kind = match candidate.entity_type.as_str() {
        "forum_category" => ForumSearchResultCandidateKind::Category,
        "forum_topic" => ForumSearchResultCandidateKind::Topic,
        "forum_reply" => ForumSearchResultCandidateKind::Reply,
        // A document outside the Forum entity vocabulary is not this owner's to
        // authorize; the neutral executor only sends candidates of its own module.
        _ => return None,
    };
    Some(ForumSearchResultCandidate {
        document_id: candidate.document_id,
        kind,
    })
}

fn map_forum_error(error: ForumError) -> PortError {
    let stable_code = error.stable_code().to_ascii_lowercase();
    let retryable = error.is_retryable();
    let public_message = error.to_string();

    match error {
        ForumError::Validation(message) => PortError::validation(stable_code, message),
        ForumError::CategoryNotFound(_)
        | ForumError::TopicNotFound(_)
        | ForumError::ReplyNotFound(_) => {
            PortError::not_found(stable_code, "Forum Search result is unavailable")
        }
        ForumError::Forbidden(_) => {
            PortError::forbidden(stable_code, "Forum Search result is unavailable")
        }
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
