use std::time::Duration;

use rustok_api::{AuthContext, PortContext, RequestContext};
use uuid::Uuid;

use crate::error::{ForumError, ForumResult};

const FORUM_TOPIC_READ_FACTS_DEADLINE: Duration = Duration::from_secs(5);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ForumTopicReadTransport {
    Graphql,
    NativeServer,
}

impl ForumTopicReadTransport {
    const fn label(self) -> &'static str {
        match self {
            Self::Graphql => "graphql",
            Self::NativeServer => "native",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ForumTopicReadOperation {
    SelectedTopic,
    MarkRead,
}

impl ForumTopicReadOperation {
    const fn label(self) -> &'static str {
        match self {
            Self::SelectedTopic => "selected-topic",
            Self::MarkRead => "mark-read",
        }
    }
}

/// Build the exact read-only caller context used by richer topic audience reads.
///
/// Tenant and user identity come only from authenticated transport extensions.
/// Request DTOs never select either identity. The request snapshot, when
/// available, must agree with the authenticated principal before an optional
/// trust, Channel, or Groups facts provider can be called.
pub fn topic_read_audience_port_context(
    transport: ForumTopicReadTransport,
    operation: ForumTopicReadOperation,
    tenant_id: Uuid,
    auth: &AuthContext,
    request: Option<&RequestContext>,
    effective_locale: &str,
) -> ForumResult<PortContext> {
    if tenant_id.is_nil() || auth.tenant_id != tenant_id {
        return Err(ForumError::Validation(
            "Forum topic-read authenticated tenant does not match the requested tenant"
                .to_string(),
        ));
    }

    if let Some(request) = request {
        if request.tenant_id != tenant_id {
            return Err(ForumError::Validation(
                "Forum topic-read request tenant does not match the requested tenant".to_string(),
            ));
        }
        if request.user_id != Some(auth.user_id) {
            return Err(ForumError::Validation(
                "Forum topic-read request actor does not match the authenticated user".to_string(),
            ));
        }
    }

    let locale = effective_locale.trim();
    if locale.is_empty() {
        return Err(ForumError::Validation(
            "Forum topic-read request locale is unavailable".to_string(),
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
    .with_deadline(FORUM_TOPIC_READ_FACTS_DEADLINE);

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
            permissions: vec![Permission::FORUM_TOPICS_READ],
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

        let context = topic_read_audience_port_context(
            ForumTopicReadTransport::NativeServer,
            ForumTopicReadOperation::SelectedTopic,
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
        assert_eq!(context.claims, vec![Permission::FORUM_TOPICS_READ.to_string()]);
        assert!(
            context
                .correlation_id
                .starts_with("forum-native-selected-topic-")
        );
    }

    #[test]
    fn graphql_context_without_http_request_preserves_effective_locale() {
        let tenant_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let auth = auth(tenant_id, user_id);

        let context = topic_read_audience_port_context(
            ForumTopicReadTransport::Graphql,
            ForumTopicReadOperation::MarkRead,
            tenant_id,
            &auth,
            None,
            "en",
        )
        .expect("GraphQL connection context should compose without an HTTP snapshot");

        assert_eq!(context.locale, "en");
        assert!(context.channel.is_none());
        assert!(context.correlation_id.starts_with("forum-graphql-mark-read-"));
    }

    #[test]
    fn mismatched_auth_request_tenant_or_user_fails_before_owner_facts() {
        let tenant_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let auth = auth(tenant_id, user_id);

        assert!(matches!(
            topic_read_audience_port_context(
                ForumTopicReadTransport::Graphql,
                ForumTopicReadOperation::SelectedTopic,
                Uuid::new_v4(),
                &auth,
                None,
                "en",
            ),
            Err(ForumError::Validation(message)) if message.contains("authenticated tenant")
        ));

        let foreign_tenant_request = request(Uuid::new_v4(), user_id);
        assert!(matches!(
            topic_read_audience_port_context(
                ForumTopicReadTransport::NativeServer,
                ForumTopicReadOperation::SelectedTopic,
                tenant_id,
                &auth,
                Some(&foreign_tenant_request),
                "en",
            ),
            Err(ForumError::Validation(message)) if message.contains("request tenant")
        ));

        let foreign_user_request = request(tenant_id, Uuid::new_v4());
        assert!(matches!(
            topic_read_audience_port_context(
                ForumTopicReadTransport::NativeServer,
                ForumTopicReadOperation::MarkRead,
                tenant_id,
                &auth,
                Some(&foreign_user_request),
                "en",
            ),
            Err(ForumError::Validation(message)) if message.contains("request actor")
        ));
    }
}
