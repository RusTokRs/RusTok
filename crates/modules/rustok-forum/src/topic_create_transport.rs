use std::time::Duration;

use rustok_api::{AuthContext, PortContext, RequestContext};
use uuid::Uuid;

use crate::error::{ForumError, ForumResult};

const FORUM_TOPIC_CREATE_FACTS_DEADLINE: Duration = Duration::from_secs(5);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ForumTopicCreateTransport {
    Graphql,
    Rest,
}

impl ForumTopicCreateTransport {
    const fn label(self) -> &'static str {
        match self {
            Self::Graphql => "graphql",
            Self::Rest => "rest",
        }
    }
}

/// Build the exact read-only caller context used by topic-create audience facts.
///
/// Tenant and user identity come only from authenticated transport extensions.
/// Request DTOs never select either identity. When an HTTP request context is
/// available, it must agree with the authenticated principal before any owner
/// facts provider can be called.
pub(crate) fn topic_create_audience_port_context(
    transport: ForumTopicCreateTransport,
    tenant_id: Uuid,
    auth: &AuthContext,
    request: Option<&RequestContext>,
    fallback_locale: &str,
) -> ForumResult<PortContext> {
    if tenant_id.is_nil() || auth.tenant_id != tenant_id {
        return Err(ForumError::Validation(
            "Forum topic-create authenticated tenant does not match the requested tenant"
                .to_string(),
        ));
    }

    if let Some(request) = request {
        if request.tenant_id != tenant_id {
            return Err(ForumError::Validation(
                "Forum topic-create request tenant does not match the requested tenant".to_string(),
            ));
        }
        if request.user_id != Some(auth.user_id) {
            return Err(ForumError::Validation(
                "Forum topic-create request actor does not match the authenticated user"
                    .to_string(),
            ));
        }
    }

    let locale = request
        .map(|request| request.locale.trim())
        .filter(|locale| !locale.is_empty())
        .unwrap_or_else(|| fallback_locale.trim());
    if locale.is_empty() {
        return Err(ForumError::Validation(
            "Forum topic-create request locale is unavailable".to_string(),
        ));
    }

    let mut context = PortContext::new(
        tenant_id.to_string(),
        auth.port_actor(),
        locale,
        format!(
            "forum-{}-topic-create-{}-{}",
            transport.label(),
            auth.session_id,
            Uuid::new_v4()
        ),
    )
    .with_deadline(FORUM_TOPIC_CREATE_FACTS_DEADLINE);

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
            permissions: vec![Permission::FORUM_TOPICS_CREATE],
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
    fn exact_rest_context_uses_authenticated_identity_deadline_claims_and_channel() {
        let tenant_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let auth = auth(tenant_id, user_id);
        let request = request(tenant_id, user_id);

        let context = topic_create_audience_port_context(
            ForumTopicCreateTransport::Rest,
            tenant_id,
            &auth,
            Some(&request),
            "en",
        )
        .expect("trusted REST context should compose");

        assert_eq!(context.tenant_id, tenant_id.to_string());
        assert_eq!(context.actor.kind, PortActorKind::User);
        assert_eq!(context.actor.id, user_id.to_string());
        assert_eq!(context.locale, "ru-RU");
        assert_eq!(context.channel.as_deref(), Some("members"));
        assert_eq!(context.deadline_ms, Some(5_000));
        assert_eq!(
            context.claims,
            vec![Permission::FORUM_TOPICS_CREATE.to_string()]
        );
        assert!(
            context
                .correlation_id
                .starts_with("forum-rest-topic-create-")
        );
    }

    #[test]
    fn graphql_context_without_http_request_uses_authenticated_tenant_and_fallback_locale() {
        let tenant_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let auth = auth(tenant_id, user_id);

        let context = topic_create_audience_port_context(
            ForumTopicCreateTransport::Graphql,
            tenant_id,
            &auth,
            None,
            "en",
        )
        .expect("GraphQL connection context should compose without an HTTP request snapshot");

        assert_eq!(context.locale, "en");
        assert!(context.channel.is_none());
        assert!(
            context
                .correlation_id
                .starts_with("forum-graphql-topic-create-")
        );
    }

    #[test]
    fn mismatched_auth_request_tenant_or_user_fails_before_owner_facts() {
        let tenant_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let auth = auth(tenant_id, user_id);

        assert!(matches!(
            topic_create_audience_port_context(
                ForumTopicCreateTransport::Rest,
                Uuid::new_v4(),
                &auth,
                None,
                "en",
            ),
            Err(ForumError::Validation(message)) if message.contains("authenticated tenant")
        ));

        let foreign_tenant_request = request(Uuid::new_v4(), user_id);
        assert!(matches!(
            topic_create_audience_port_context(
                ForumTopicCreateTransport::Rest,
                tenant_id,
                &auth,
                Some(&foreign_tenant_request),
                "en",
            ),
            Err(ForumError::Validation(message)) if message.contains("request tenant")
        ));

        let foreign_user_request = request(tenant_id, Uuid::new_v4());
        assert!(matches!(
            topic_create_audience_port_context(
                ForumTopicCreateTransport::Rest,
                tenant_id,
                &auth,
                Some(&foreign_user_request),
                "en",
            ),
            Err(ForumError::Validation(message)) if message.contains("request actor")
        ));
    }
}
