use std::time::Duration;

use rustok_api::{AuthContext, PortContext, RequestContext};
use uuid::Uuid;

use crate::error::{ForumError, ForumResult};

const FORUM_CATEGORY_READ_FACTS_DEADLINE: Duration = Duration::from_secs(5);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ForumCategoryReadTransport {
    Graphql,
    NativeServer,
    Rest,
}

impl ForumCategoryReadTransport {
    const fn label(self) -> &'static str {
        match self {
            Self::Graphql => "graphql",
            Self::NativeServer => "native",
            Self::Rest => "rest",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ForumCategoryReadOperation {
    CategoryList,
    SelectedCategory,
    CategoryTree,
    SearchResultEligibility,
}

impl ForumCategoryReadOperation {
    const fn label(self) -> &'static str {
        match self {
            Self::CategoryList => "category-list",
            Self::SelectedCategory => "selected-category",
            Self::CategoryTree => "category-tree",
            Self::SearchResultEligibility => "search-result-eligibility",
        }
    }
}

/// Builds the read-only owner-facts context used by richer category audience reads.
/// Tenant, actor, claims, route channel, session identity and locale come only
/// from trusted transport extensions rather than request DTO fields.
pub fn category_read_audience_port_context(
    transport: ForumCategoryReadTransport,
    operation: ForumCategoryReadOperation,
    tenant_id: Uuid,
    auth: &AuthContext,
    request: Option<&RequestContext>,
    effective_locale: &str,
) -> ForumResult<PortContext> {
    if tenant_id.is_nil() || auth.tenant_id != tenant_id {
        return Err(ForumError::Validation(
            "Forum category-read authenticated tenant does not match the requested tenant"
                .to_string(),
        ));
    }

    if let Some(request) = request {
        if request.tenant_id != tenant_id {
            return Err(ForumError::Validation(
                "Forum category-read request tenant does not match the requested tenant"
                    .to_string(),
            ));
        }
        if request.user_id != Some(auth.user_id) {
            return Err(ForumError::Validation(
                "Forum category-read request actor does not match the authenticated user"
                    .to_string(),
            ));
        }
    }

    let locale = effective_locale.trim();
    if locale.is_empty() {
        return Err(ForumError::Validation(
            "Forum category-read request locale is unavailable".to_string(),
        ));
    }

    let mut context = PortContext::new(
        tenant_id.to_string(),
        auth.port_actor(),
        locale,
        format!(
            "forum-{}-{}-{}-{}",
            transport.label(),
            operation.label(),
            auth.session_id,
            Uuid::new_v4()
        ),
    )
    .with_deadline(FORUM_CATEGORY_READ_FACTS_DEADLINE);

    for permission in &auth.permissions {
        context = context.with_claim(permission.to_string());
    }
    if let Some(channel_slug) = request
        .and_then(|request| request.channel_slug.as_deref())
        .map(str::trim)
        .filter(|slug| !slug.is_empty())
    {
        context = context.with_channel(channel_slug.to_string());
    }

    Ok(context)
}

#[cfg(test)]
mod tests {
    use rustok_api::{Permission, PortActorKind};

    use super::*;

    fn auth(tenant_id: Uuid, user_id: Uuid) -> AuthContext {
        AuthContext {
            user_id,
            session_id: Uuid::new_v4(),
            tenant_id,
            permissions: vec![Permission::FORUM_CATEGORIES_LIST],
            client_id: None,
            scopes: Vec::new(),
            grant_type: "direct".to_string(),
        }
    }

    fn request(tenant_id: Uuid, user_id: Uuid) -> RequestContext {
        RequestContext {
            tenant_id,
            user_id: Some(user_id),
            channel_id: Some(Uuid::new_v4()),
            channel_slug: Some("members".to_string()),
            channel_resolution_source: None,
            locale: "ru-RU".to_string(),
        }
    }

    #[test]
    fn exact_context_uses_authenticated_identity_deadline_claims_locale_and_channel() {
        let tenant_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let auth = auth(tenant_id, user_id);
        let request = request(tenant_id, user_id);

        let context = category_read_audience_port_context(
            ForumCategoryReadTransport::NativeServer,
            ForumCategoryReadOperation::SearchResultEligibility,
            tenant_id,
            &auth,
            Some(&request),
            "de-DE",
        )
        .expect("trusted native context should compose");

        assert_eq!(context.tenant_id, tenant_id.to_string());
        assert_eq!(context.actor.kind, PortActorKind::User);
        assert_eq!(context.actor.id, user_id.to_string());
        assert_eq!(context.locale, "de-DE");
        assert_eq!(context.channel.as_deref(), Some("members"));
        assert_eq!(context.deadline_ms, Some(5_000));
        assert_eq!(
            context.claims,
            vec![Permission::FORUM_CATEGORIES_LIST.to_string()]
        );
        assert!(
            context
                .correlation_id
                .starts_with("forum-native-search-result-eligibility-")
        );
    }

    #[test]
    fn mismatched_transport_identity_fails_before_owner_facts() {
        let tenant_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let auth = auth(tenant_id, user_id);

        assert!(matches!(
            category_read_audience_port_context(
                ForumCategoryReadTransport::Graphql,
                ForumCategoryReadOperation::SelectedCategory,
                Uuid::new_v4(),
                &auth,
                None,
                "en",
            ),
            Err(ForumError::Validation(message)) if message.contains("authenticated tenant")
        ));

        let foreign_user_request = request(tenant_id, Uuid::new_v4());
        assert!(matches!(
            category_read_audience_port_context(
                ForumCategoryReadTransport::Rest,
                ForumCategoryReadOperation::CategoryList,
                tenant_id,
                &auth,
                Some(&foreign_user_request),
                "en",
            ),
            Err(ForumError::Validation(message)) if message.contains("request actor")
        ));
    }
}
