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
