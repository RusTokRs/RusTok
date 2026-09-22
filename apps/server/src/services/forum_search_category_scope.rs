use async_trait::async_trait;
use rustok_api::{
    Permission, PortError, PortErrorKind, has_any_effective_permission, is_tenant_module_enabled,
};
use rustok_core::SecurityContext;
use rustok_forum::{
    ForumCategoryReadOperation, ForumCategoryReadTransport, ForumError,
    ForumSearchCategoryAudienceScopeService, SharedForumAudienceFactsPort,
    category_read_audience_port_context,
};
use rustok_search::{
    FORUM_SEARCH_SOURCE_MODULE, SharedStorefrontSearchCategoryScopePort,
    StorefrontSearchCategoryScopePort, StorefrontSearchCategoryScopeRequest,
    StorefrontSearchTransport,
};
use sea_orm::DatabaseConnection;

pub(crate) struct ServerForumSearchCategoryScopePort {
    db: DatabaseConnection,
    audience_facts: Option<SharedForumAudienceFactsPort>,
}

impl ServerForumSearchCategoryScopePort {
    pub(crate) fn shared(
        db: DatabaseConnection,
        audience_facts: Option<SharedForumAudienceFactsPort>,
    ) -> SharedStorefrontSearchCategoryScopePort {
        std::sync::Arc::new(Self { db, audience_facts })
    }

    fn service(&self) -> ForumSearchCategoryAudienceScopeService {
        match self.audience_facts.clone() {
            Some(facts) => {
                ForumSearchCategoryAudienceScopeService::with_audience_facts(self.db.clone(), facts)
            }
            None => ForumSearchCategoryAudienceScopeService::new(self.db.clone()),
        }
    }
}

#[async_trait]
impl StorefrontSearchCategoryScopePort for ServerForumSearchCategoryScopePort {
    async fn expand_forum_category_scope(
        &self,
        request: StorefrontSearchCategoryScopeRequest,
    ) -> Result<Vec<uuid::Uuid>, PortError> {
        if request.tenant_id.is_nil() {
            return Err(PortError::validation(
                "forum.search_category_scope.tenant_required",
                "Forum Search category scope requires a tenant",
            ));
        }
        if !is_explicit_forum_only_source_scope(&request.source_modules) {
            return Err(PortError::validation(
                "forum.search_category_scope.forum_only_required",
                "Forum category expansion requires an explicit Forum-only source scope",
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
        let result = if let Some(auth) = request.auth.as_ref().filter(|auth| {
            has_any_effective_permission(&auth.permissions, &[Permission::FORUM_CATEGORIES_LIST])
        }) {
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

fn is_explicit_forum_only_source_scope(source_modules: &[String]) -> bool {
    source_modules.len() == 1 && source_modules[0] == FORUM_SEARCH_SOURCE_MODULE
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

#[cfg(test)]
mod tests {
    use super::is_explicit_forum_only_source_scope;

    #[test]
    fn only_exact_forum_source_scope_is_admitted() {
        assert!(is_explicit_forum_only_source_scope(&["forum".to_string()]));
        assert!(!is_explicit_forum_only_source_scope(&[]));
        assert!(!is_explicit_forum_only_source_scope(&[
            "forum".to_string(),
            "product".to_string(),
        ]));
        assert!(!is_explicit_forum_only_source_scope(&[
            "product".to_string(),
        ]));
    }
}
