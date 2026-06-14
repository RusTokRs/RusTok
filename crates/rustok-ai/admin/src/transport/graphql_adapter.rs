//! Headless GraphQL adapter contract for AI admin transports.
//!
//! This module intentionally contains no HTTP client and no Leptos imports. Host
//! applications can use the operation documents with their own GraphQL runtime,
//! while the Leptos UI keeps using the native server-function adapter through the
//! transport facade.

use serde::{Deserialize, Serialize};

pub const AI_BOOTSTRAP_OPERATION: &str = "AiBootstrap";
pub const AI_SESSION_OPERATION: &str = "AiSession";
pub const AI_RECENT_STREAM_EVENTS_OPERATION: &str = "AiRecentRunStreamEvents";
pub const AI_SESSION_EVENTS_SUBSCRIPTION_OPERATION: &str = "AiSessionEvents";

pub const AI_BOOTSTRAP_QUERY: &str = r#"
query AiBootstrap {
  aiRuntimeMetrics {
    routerResolutionsTotal
    routerOverridesTotal
    selectedAutoTotal
    selectedDirectTotal
    selectedMcpTotal
    completedRunsTotal
    failedRunsTotal
    waitingApprovalRunsTotal
    localeFallbackTotal
    runLatencyMsTotal
    runLatencySamples
    providerKindTotals { label total }
    executionTargetTotals { label total }
    taskProfileTotals { label total }
    resolvedLocaleTotals { label total }
  }
  aiProviderProfiles {
    id
    slug
    displayName
    providerKind
    baseUrl
    model
    temperature
    maxTokens
    hasSecret
    isActive
    capabilities
    usagePolicy { allowedTaskProfiles deniedTaskProfiles restrictedRoleSlugs }
  }
  aiTaskProfiles { id slug displayName description targetCapability systemPrompt allowedProviderProfileIds preferredProviderProfileIds fallbackStrategy toolProfileId defaultExecutionMode isActive }
  aiToolProfiles { id slug displayName description allowedTools deniedTools sensitiveTools isActive }
  aiChatSessions { id title providerProfileId taskProfileId toolProfileId executionMode requestedLocale resolvedLocale status latestRunStatus pendingApprovals }
  aiRecentRuns(limit: 20) {
    id
    sessionId
    sessionTitle
    providerProfileId
    providerDisplayName
    providerKind
    taskProfileId
    taskProfileSlug
    status
    model
    executionMode
    executionPath
    executionTarget
    requestedLocale
    resolvedLocale
    errorMessage
    startedAt
    completedAt
    updatedAt
    durationMs
  }
  aiRecentRunStreamEvents(limit: 20) {
    sessionId
    runId
    eventKind
    contentDelta
    accumulatedContent
    errorMessage
    createdAt
  }
}
"#;

pub const AI_SESSION_QUERY: &str = r#"
query AiSession($id: UUID!) {
  aiChatSession(id: $id) {
    session { id title providerProfileId taskProfileId toolProfileId executionMode requestedLocale resolvedLocale status latestRunStatus pendingApprovals }
    providerProfile {
      id slug displayName providerKind baseUrl model temperature maxTokens hasSecret isActive capabilities
      usagePolicy { allowedTaskProfiles deniedTaskProfiles restrictedRoleSlugs }
    }
    taskProfile { id slug displayName description targetCapability systemPrompt allowedProviderProfileIds preferredProviderProfileIds fallbackStrategy toolProfileId defaultExecutionMode isActive }
    toolProfile { id slug displayName description allowedTools deniedTools sensitiveTools isActive }
    messages { id role content }
    runs { id taskProfileId status model executionMode executionPath requestedLocale resolvedLocale errorMessage decisionTrace }
    toolTraces { toolName status durationMs }
    approvals { id toolName reason status }
  }
  aiRecentRunStreamEvents(sessionId: $id, limit: 20) {
    sessionId
    runId
    eventKind
    contentDelta
    accumulatedContent
    errorMessage
    createdAt
  }
}
"#;

pub const AI_SESSION_EVENTS_SUBSCRIPTION: &str = r#"
subscription AiSessionEvents($sessionId: UUID!) {
  aiSessionEvents(sessionId: $sessionId) {
    sessionId
    runId
    eventKind
    contentDelta
    accumulatedContent
    errorMessage
    createdAt
  }
}
"#;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct AiGraphqlRequest<V> {
    pub operation_name: &'static str,
    pub query: &'static str,
    pub variables: V,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct EmptyVariables {}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AiSessionVariables {
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AiSessionEventsVariables {
    #[serde(rename = "sessionId")]
    pub session_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AiGraphqlWsConnectionPayload {
    pub token: String,
    #[serde(rename = "tenantSlug")]
    pub tenant_slug: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub locale: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct AiGraphqlWsMessage<P> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(rename = "type")]
    pub message_type: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload: Option<P>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct AiGraphqlWsSubscribePayload<V> {
    pub query: &'static str,
    pub variables: V,
}

pub fn graphql_ws_url_from_location(protocol: Option<&str>, host: Option<&str>) -> String {
    let protocol = protocol.unwrap_or("http:");
    let host = host.unwrap_or("localhost:5150");
    let ws_scheme = if protocol.eq_ignore_ascii_case("https:") {
        "wss"
    } else {
        "ws"
    };
    format!("{ws_scheme}://{host}/api/graphql/ws")
}

pub fn connection_init_message(
    token: impl Into<String>,
    tenant_slug: impl Into<String>,
    locale: Option<String>,
) -> AiGraphqlWsMessage<AiGraphqlWsConnectionPayload> {
    AiGraphqlWsMessage {
        id: None,
        message_type: "connection_init",
        payload: Some(AiGraphqlWsConnectionPayload {
            token: token.into(),
            tenant_slug: tenant_slug.into(),
            locale,
        }),
    }
}

pub fn session_events_subscribe_message(
    message_id: impl Into<String>,
    session_id: impl Into<String>,
) -> AiGraphqlWsMessage<AiGraphqlWsSubscribePayload<AiSessionEventsVariables>> {
    let request = session_events_subscription_request(session_id);
    AiGraphqlWsMessage {
        id: Some(message_id.into()),
        message_type: "subscribe",
        payload: Some(AiGraphqlWsSubscribePayload {
            query: request.query,
            variables: request.variables,
        }),
    }
}

pub fn bootstrap_request() -> AiGraphqlRequest<EmptyVariables> {
    AiGraphqlRequest {
        operation_name: AI_BOOTSTRAP_OPERATION,
        query: AI_BOOTSTRAP_QUERY,
        variables: EmptyVariables::default(),
    }
}

pub fn session_request(session_id: impl Into<String>) -> AiGraphqlRequest<AiSessionVariables> {
    AiGraphqlRequest {
        operation_name: AI_SESSION_OPERATION,
        query: AI_SESSION_QUERY,
        variables: AiSessionVariables {
            id: session_id.into(),
        },
    }
}

pub fn session_events_subscription_request(
    session_id: impl Into<String>,
) -> AiGraphqlRequest<AiSessionEventsVariables> {
    AiGraphqlRequest {
        operation_name: AI_SESSION_EVENTS_SUBSCRIPTION_OPERATION,
        query: AI_SESSION_EVENTS_SUBSCRIPTION,
        variables: AiSessionEventsVariables {
            session_id: session_id.into(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn graphql_ws_url_uses_browser_location_scheme() {
        assert_eq!(
            graphql_ws_url_from_location(Some("https:"), Some("admin.example.test")),
            "wss://admin.example.test/api/graphql/ws"
        );
        assert_eq!(
            graphql_ws_url_from_location(Some("http:"), Some("localhost:5150")),
            "ws://localhost:5150/api/graphql/ws"
        );
    }

    #[test]
    fn websocket_messages_keep_graphql_transport_shape() {
        let init = connection_init_message("token-1", "tenant-a", Some("ru".to_string()));
        assert_eq!(init.message_type, "connection_init");
        assert_eq!(init.payload.as_ref().unwrap().tenant_slug, "tenant-a");

        let subscribe = session_events_subscribe_message("ai-session-events", "session-3");
        assert_eq!(subscribe.id.as_deref(), Some("ai-session-events"));
        assert_eq!(subscribe.message_type, "subscribe");
        let payload = subscribe.payload.as_ref().unwrap();
        assert!(payload.query.contains("subscription AiSessionEvents"));
        assert_eq!(payload.variables.session_id, "session-3");
    }

    #[test]
    fn bootstrap_request_uses_recent_diagnostics_fields() {
        let request = bootstrap_request();
        assert_eq!(request.operation_name, AI_BOOTSTRAP_OPERATION);
        assert!(request.query.contains("aiRecentRuns(limit: 20)"));
        assert!(request.query.contains("aiRecentRunStreamEvents(limit: 20)"));
        assert!(request.query.contains("taskProfileTotals"));
        assert!(request.query.contains("resolvedLocaleTotals"));
    }

    #[test]
    fn session_request_keeps_session_id_variable() {
        let request = session_request("session-1");
        assert_eq!(request.operation_name, AI_SESSION_OPERATION);
        assert_eq!(request.variables.id, "session-1");
        assert!(request.query.contains("query AiSession($id: UUID!)"));
    }

    #[test]
    fn subscription_request_uses_graphql_session_id_name() {
        let request = session_events_subscription_request("session-2");
        assert_eq!(
            request.operation_name,
            AI_SESSION_EVENTS_SUBSCRIPTION_OPERATION
        );
        assert_eq!(request.variables.session_id, "session-2");
        assert!(request.query.contains("subscription AiSessionEvents"));
    }
}
