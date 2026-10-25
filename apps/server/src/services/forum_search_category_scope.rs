use async_trait::async_trait;
use rustok_api::{PortError, PortErrorKind, SharedStaticModuleSettingsReader, is_tenant_module_enabled};
use rustok_core::SecurityContext;
use rustok_forum::{
    ForumCategoryReadOperation, ForumCategoryReadTransport, ForumError,
    ForumSearchCategoryAudienceScopeService, ForumSettingsProviders, SharedForumAudienceFactsPort,
    category_read_audience_port_context,
};
use rustok_search::{
    FORUM_SEARCH_SOURCE_MODULE, SharedStorefrontSearchCategoryScopePort,
    StorefrontSearchCategoryScopePort, StorefrontSearchCategoryScopeRequest,
    StorefrontSearchTransport, scope_is_exactly,
};
use sea_orm::DatabaseConnection;

pub(crate) struct ServerForumSearchCategoryScopePort {
    db: DatabaseConnection,
    audience_facts: Option<SharedForumAudienceFactsPort>,
    settings_reader: Option<SharedStaticModuleSettingsReader>,
}

impl ServerForumSearchCategoryScopePort {
    pub(crate) fn shared(
        db: DatabaseConnection,
        audience_facts: Option<SharedForumAudienceFactsPort>,
        settings_reader: Option<SharedStaticModuleSettingsReader>,
    ) -> SharedStorefrontSearchCategoryScopePort {
        std::sync::Arc::new(Self { db, audience_facts, settings_reader })
    }

    fn service(&self) -> ForumSearchCategoryAudienceScopeService {
        let service = match self.audience_facts.clone() {
            Some(facts) => {
                ForumSearchCategoryAudienceScopeService::with_audience_facts(self.db.clone(), facts)
            }
            None => ForumSearchCategoryAudienceScopeService::new(self.db.clone()),
        };
        match self.settings_reader.clone() {
            Some(reader) => service.with_settings_providers(
                ForumSettingsProviders::default().with_static_reader(reader),
            ),
            None => service,
        }
    }
}

#[async_trait]
impl StorefrontSearchCategoryScopePort for ServerForumSearchCategoryScopePort {
    fn source_module(&self) -> &'static str {
        FORUM_SEARCH_SOURCE_MODULE
    }

    async fn expand_category_scope(
        &self,
        request: StorefrontSearchCategoryScopeRequest,
    ) -> Result<Vec<uuid::Uuid>, PortError> {
        if request.tenant_id.is_nil() {
            return Err(PortError::validation(
                "forum.search_category_scope.tenant_required",
                "Forum Search category scope requires a tenant",
            ));
        }
        // The resolver only consults a port when the request scope is exactly the
        // module the port declares; enforcing the same rule here keeps a direct caller
        // from expanding Forum's tree into a mixed or foreign search.
        if !scope_is_exactly(&request.source_modules, self.source_module()) {
            return Err(PortError::validation(
                "forum.search_category_scope.own_scope_required",
                "Forum category expansion requires a Forum-only source scope",
            ));
        }
        if request.category_ids.is_empty() {
            return Ok(Vec::new());
        }
        let locale = request.locale.trim();
        if locale.is_empty() {
            return Err(PortError::validation(
                "forum.search_category_scope.locale_required",
                "Forum Search category scope requires a locale",
            ));
        }

        let forum_enabled =
            is_tenant_module_enabled(&self.db, request.tenant_id, FORUM_SEARCH_SOURCE_MODULE)
                .await
                .map_err(|error| {
                    tracing::error!(
                        tenant_id = %request.tenant_id,
                        error = ?error,
                        "Failed to resolve tenant Forum module state for storefront Search"
                    );
                    PortError::unavailable(
                        "forum.search_category_scope.module_state_unavailable",
                        "Forum Search category scope is temporarily unavailable",
                    )
                })?;
        if !forum_enabled {
            return Err(PortError::not_found(
                "forum.search_category_scope.unavailable",
                "Forum category scope is unavailable",
            ));
        }

        let service = self.service();
        let fallback_locale = request.fallback_locale.as_deref();
        // A signed-in principal must never silently become a public viewer when
        // its Forum permissions are absent; the authenticated owner decides.
        let result = if let Some(auth) = request.auth.as_ref() {
            let transport = match request.transport {
                StorefrontSearchTransport::Graphql => ForumCategoryReadTransport::Graphql,
                StorefrontSearchTransport::NativeServer => ForumCategoryReadTransport::NativeServer,
            };
            let context = category_read_audience_port_context(
                transport,
                ForumCategoryReadOperation::CategoryTree,
                request.tenant_id,
                auth,
                request.request_context.as_ref(),
                locale,
            )
            .map_err(map_forum_error)?;
            let security =
                SecurityContext::from_permission_snapshot(Some(auth.user_id), &auth.permissions);
            service
                .expand_authenticated_visible_subtrees(
                    request.tenant_id,
                    security,
                    context,
                    fallback_locale,
                    &request.category_ids,
                )
                .await
        } else {
            service
                .expand_public_visible_subtrees(
                    request.tenant_id,
                    locale,
                    fallback_locale,
                    &request.category_ids,
                )
                .await
        }
        .map_err(map_forum_error)?;

        Ok(result.expanded_category_ids)
    }
}

fn map_forum_error(error: ForumError) -> PortError {
    let stable_code = error.stable_code().to_ascii_lowercase();
    let retryable = error.is_retryable();
    let public_message = error.to_string();

    match error {
        ForumError::Validation(message) => PortError::validation(stable_code, message),
        ForumError::CategoryNotFound(_) => {
            PortError::not_found(stable_code, "Forum category scope is unavailable")
        }
        ForumError::Forbidden(_) => {
            PortError::forbidden(stable_code, "Forum category scope is unavailable")
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
            "Forum category scope could not be resolved safely",
        ),
    }
}
