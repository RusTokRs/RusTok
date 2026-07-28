use std::future::Future;
use std::time::Duration;

use rustok_api::{AuthContext, PortContext, RequestContext};
use uuid::Uuid;

use crate::audience::SharedForumAudienceFactsPort;
use crate::error::{ForumError, ForumResult};

const FORUM_MODERATION_FACTS_DEADLINE: Duration = Duration::from_secs(5);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ForumModerationTransport {
    Graphql,
    Rest,
}

impl ForumModerationTransport {
    const fn label(self) -> &'static str {
        match self {
            Self::Graphql => "graphql",
            Self::Rest => "rest",
        }
    }
}

#[derive(Clone)]
pub(crate) struct ForumModerationTransportScopeData {
    audience_facts: Option<SharedForumAudienceFactsPort>,
    context: PortContext,
}

impl ForumModerationTransportScopeData {
    pub(crate) fn new(
        audience_facts: Option<SharedForumAudienceFactsPort>,
        context: PortContext,
    ) -> Self {
        Self {
            audience_facts,
            context,
        }
    }
}

tokio::task_local! {
    static CURRENT_FORUM_MODERATION_TRANSPORT_SCOPE: ForumModerationTransportScopeData;
}

pub(crate) async fn with_forum_moderation_transport_scope<F>(
    scope: ForumModerationTransportScopeData,
    future: F,
) -> F::Output
where
    F: Future,
{
    CURRENT_FORUM_MODERATION_TRANSPORT_SCOPE
        .scope(scope, future)
        .await
}

pub(crate) fn current_moderation_audience_facts() -> Option<SharedForumAudienceFactsPort> {
    CURRENT_FORUM_MODERATION_TRANSPORT_SCOPE
        .try_with(|scope| scope.audience_facts.clone())
        .ok()
        .flatten()
}

pub(crate) fn current_moderation_audience_context() -> Option<PortContext> {
    CURRENT_FORUM_MODERATION_TRANSPORT_SCOPE
        .try_with(|scope| scope.context.clone())
        .ok()
}

/// Builds the exact authenticated caller context used by moderation audience facts.
///
/// Tenant and actor identity come only from authenticated transport extensions.
/// Request DTOs and GraphQL arguments cannot select either identity. An optional
/// middleware request snapshot must agree with the authenticated principal before
/// any Forum owner lookup or optional facts-provider access can occur.
pub(crate) fn moderation_audience_port_context(
    transport: ForumModerationTransport,
    tenant_id: Uuid,
    auth: &AuthContext,
    request: Option<&RequestContext>,
    fallback_locale: &str,
) -> ForumResult<PortContext> {
    if tenant_id.is_nil() || auth.tenant_id != tenant_id {
        return Err(ForumError::Validation(
            "Forum moderation authenticated tenant does not match the requested tenant"
                .to_string(),
        ));
    }

    if let Some(request) = request {
        if request.tenant_id != tenant_id {
            return Err(ForumError::Validation(
                "Forum moderation request tenant does not match the requested tenant".to_string(),
            ));
        }
        if request.user_id != Some(auth.user_id) {
            return Err(ForumError::Validation(
                "Forum moderation request actor does not match the authenticated user".to_string(),
            ));
        }
    }

    let locale = request
        .map(|request| request.locale.trim())
        .filter(|locale| !locale.is_empty())
        .unwrap_or_else(|| fallback_locale.trim());
    if locale.is_empty() {
        return Err(ForumError::Validation(
            "Forum moderation request locale is unavailable".to_string(),
        ));
    }

    let mut context = PortContext::new(
        tenant_id.to_string(),
        auth.port_actor(),
        locale,
        format!(
            "forum-{}-moderation-{}-{}",
            transport.label(),
            auth.session_id,
            Uuid::new_v4()
        ),
    )
    .with_deadline(FORUM_MODERATION_FACTS_DEADLINE);

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
            permissions: vec![
                Permission::FORUM_TOPICS_UPDATE,
                Permission::FORUM_TOPICS_MODERATE,
            ],
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
            channel_slug: Some("moderators".to_string()),
            channel_resolution_source: None,
            locale: "ru-RU".to_string(),
        }
    }

    #[test]
    fn exact_rest_context_uses_authenticated_identity_deadline_claims_and_channel() {
        let tenant_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let auth = auth(tenant_id, user_id);
        let request = request(tenant_id, user_id);

        let context = moderation_audience_port_context(
            ForumModerationTransport::Rest,
            tenant_id,
            &auth,
            Some(&request),
            "en",
        )
        .expect("trusted REST moderation context should compose");

        assert_eq!(context.tenant_id, tenant_id.to_string());
        assert_eq!(context.actor.kind, PortActorKind::User);
        assert_eq!(context.actor.id, user_id.to_string());
        assert_eq!(context.locale, "ru-RU");
        assert_eq!(context.channel.as_deref(), Some("moderators"));
        assert_eq!(context.deadline_ms, Some(5_000));
        assert_eq!(
            context.claims,
            vec![
                Permission::FORUM_TOPICS_UPDATE.to_string(),
                Permission::FORUM_TOPICS_MODERATE.to_string(),
            ]
        );
        assert!(context.correlation_id.starts_with("forum-rest-moderation-"));
    }

    #[test]
    fn graphql_context_without_request_snapshot_uses_authenticated_fallback() {
        let tenant_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let auth = auth(tenant_id, user_id);

        let context = moderation_audience_port_context(
            ForumModerationTransport::Graphql,
            tenant_id,
            &auth,
            None,
            "en",
        )
        .expect("GraphQL moderation context should use authenticated fallback");

        assert_eq!(context.locale, "en");
        assert!(context.channel.is_none());
        assert!(context
            .correlation_id
            .starts_with("forum-graphql-moderation-"));
    }

    #[test]
    fn mismatched_transport_identity_fails_before_owner_access() {
        let tenant_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let auth = auth(tenant_id, user_id);

        assert!(matches!(
            moderation_audience_port_context(
                ForumModerationTransport::Graphql,
                Uuid::new_v4(),
                &auth,
                None,
                "en",
            ),
            Err(ForumError::Validation(message)) if message.contains("authenticated tenant")
        ));

        let foreign_tenant = request(Uuid::new_v4(), user_id);
        assert!(matches!(
            moderation_audience_port_context(
                ForumModerationTransport::Rest,
                tenant_id,
                &auth,
                Some(&foreign_tenant),
                "en",
            ),
            Err(ForumError::Validation(message)) if message.contains("request tenant")
        ));

        let foreign_actor = request(tenant_id, Uuid::new_v4());
        assert!(matches!(
            moderation_audience_port_context(
                ForumModerationTransport::Rest,
                tenant_id,
                &auth,
                Some(&foreign_actor),
                "en",
            ),
            Err(ForumError::Validation(message)) if message.contains("request actor")
        ));
    }
}
