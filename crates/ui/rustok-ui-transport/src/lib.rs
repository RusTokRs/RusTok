/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use std::fmt::{Display, Formatter};
use std::future::Future;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiTransportPath {
    NativeServer,
    Graphql,
}


#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiTransportRetrySafety {
    /// The operation is safe to execute again if the first transport's response is lost.
    SafeToRetry,
    /// The operation may have side effects and must not be retried through another transport.
    AtMostOnce,
}

impl UiTransportPath {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NativeServer => "native_server",
            Self::Graphql => "graphql",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiTransportError {
    pub surface: String,
    pub failed_path: UiTransportPath,
    pub fallback_attempted: bool,
    pub native_error: Option<String>,
    pub graphql_error: Option<String>,
}

impl UiTransportError {
    pub fn native(surface: impl Into<String>, error: impl Display) -> Self {
        Self {
            surface: surface.into(),
            failed_path: UiTransportPath::NativeServer,
            fallback_attempted: false,
            native_error: Some(error.to_string()),
            graphql_error: None,
        }
    }

    pub fn graphql(surface: impl Into<String>, error: impl Display) -> Self {
        Self {
            surface: surface.into(),
            failed_path: UiTransportPath::Graphql,
            fallback_attempted: false,
            native_error: None,
            graphql_error: Some(error.to_string()),
        }
    }

    pub fn fallback_failed(
        surface: impl Into<String>,
        native_error: impl Display,
        graphql_error: impl Display,
    ) -> Self {
        Self {
            surface: surface.into(),
            failed_path: UiTransportPath::Graphql,
            fallback_attempted: true,
            native_error: Some(native_error.to_string()),
            graphql_error: Some(graphql_error.to_string()),
        }
    }
}

impl Display for UiTransportError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match (&self.native_error, &self.graphql_error) {
            (Some(native), Some(graphql)) => write!(
                f,
                "{} transport fallback failed: native_server={native}; graphql={graphql}",
                self.surface
            ),
            (Some(native), None) => write!(
                f,
                "{} transport failed on {}: {native}",
                self.surface,
                self.failed_path.as_str()
            ),
            (None, Some(graphql)) => write!(
                f,
                "{} transport failed on {}: {graphql}",
                self.surface,
                self.failed_path.as_str()
            ),
            (None, None) => write!(
                f,
                "{} transport failed on {}",
                self.surface,
                self.failed_path.as_str()
            ),
        }
    }
}

impl std::error::Error for UiTransportError {}

pub type UiTransportResult<T> = Result<T, UiTransportError>;

pub async fn execute_selected_transport<T, N, NFut, NE, G, GFut, GE>(
    surface: impl Into<String>,
    path: UiTransportPath,
    native: N,
    graphql: G,
) -> UiTransportResult<T>
where
    N: FnOnce() -> NFut,
    NFut: Future<Output = Result<T, NE>>,
    NE: Display,
    G: FnOnce() -> GFut,
    GFut: Future<Output = Result<T, GE>>,
    GE: Display,
{
    let surface = surface.into();
    match path {
        UiTransportPath::NativeServer => native()
            .await
            .map_err(|error| UiTransportError::native(surface, error)),
        UiTransportPath::Graphql => graphql()
            .await
            .map_err(|error| UiTransportError::graphql(surface, error)),
    }
}

pub async fn execute_with_fallback<T, N, NFut, NE, G, GFut, GE>(
    surface: impl Into<String>,
    primary_path: UiTransportPath,
    native: N,
    graphql: G,
) -> UiTransportResult<T>
where
    N: FnOnce() -> NFut,
    NFut: Future<Output = Result<T, NE>>,
    NE: Display,
    G: FnOnce() -> GFut,
    GFut: Future<Output = Result<T, GE>>,
    GE: Display,
{
    let surface = surface.into();
    match primary_path {
        UiTransportPath::NativeServer => match native().await {
            Ok(value) => Ok(value),
            Err(native_error) => match graphql().await {
                Ok(value) => Ok(value),
                Err(graphql_error) => Err(UiTransportError::fallback_failed(
                    surface,
                    native_error,
                    graphql_error,
                )),
            },
        },
        UiTransportPath::Graphql => match graphql().await {
            Ok(value) => Ok(value),
            Err(graphql_error) => match native().await {
                Ok(value) => Ok(value),
                Err(native_error) => Err(UiTransportError {
                    surface,
                    failed_path: UiTransportPath::NativeServer,
                    fallback_attempted: true,
                    native_error: Some(native_error.to_string()),
                    graphql_error: Some(graphql_error.to_string()),
                }),
            },
        },
    }
}

pub async fn execute_transport_policy<T, N, NFut, NE, G, GFut, GE>(
    surface: impl Into<String>,
    path: UiTransportPath,
    retry_safety: UiTransportRetrySafety,
    fallback_allowed: bool,
    native: N,
    graphql: G,
) -> UiTransportResult<T>
where
    N: FnOnce() -> NFut,
    NFut: Future<Output = Result<T, NE>>,
    NE: Display,
    G: FnOnce() -> GFut,
    GFut: Future<Output = Result<T, GE>>,
    GE: Display,
{
    if fallback_allowed && retry_safety == UiTransportRetrySafety::SafeToRetry {
        execute_with_fallback(surface, path, native, graphql).await
    } else {
        execute_selected_transport(surface, path, native, graphql).await
    }
}

#[cfg(test)]
mod tests {
    use super::{
        UiTransportError, UiTransportPath, UiTransportRetrySafety, execute_selected_transport,
        execute_transport_policy,
        execute_with_fallback,
    };

    #[test]
    fn transport_path_serializes_as_stable_snake_case() {
        let serialized = serde_json::to_string(&UiTransportError::fallback_failed(
            "cart",
            "server function unavailable",
            "network unavailable",
        ))
        .expect("transport error should serialize");

        assert!(serialized.contains(r#""failed_path":"graphql""#));
        assert!(serialized.contains(r#""fallback_attempted":true"#));
        assert!(serialized.contains(r#""surface":"cart""#));
    }

    #[test]
    fn failed_fallback_keeps_both_path_errors() {
        let error = UiTransportError::fallback_failed(
            "product",
            "server function unavailable",
            "network unavailable",
        );

        assert_eq!(error.failed_path, UiTransportPath::Graphql);
        assert!(error.fallback_attempted);
        assert_eq!(
            error.native_error,
            Some("server function unavailable".to_string())
        );
        assert_eq!(error.graphql_error, Some("network unavailable".to_string()));
        assert!(
            error
                .to_string()
                .contains("native_server=server function unavailable")
        );
        assert!(error.to_string().contains("graphql=network unavailable"));
    }

    #[tokio::test]
    async fn selected_transport_returns_native_success_without_calling_graphql() {
        let result = execute_selected_transport(
            "product",
            UiTransportPath::NativeServer,
            || async { Ok::<_, &'static str>("native") },
            || async {
                panic!("graphql transport must not run in native mode");
                #[allow(unreachable_code)]
                Err::<&'static str, &'static str>("unreachable")
            },
        )
        .await
        .expect("native success should be returned");

        assert_eq!(result, "native");
    }

    #[tokio::test]
    async fn selected_transport_returns_native_error_without_calling_graphql() {
        let error = execute_selected_transport(
            "product",
            UiTransportPath::NativeServer,
            || async { Err::<&'static str, _>("native unavailable") },
            || async {
                panic!("graphql transport must not run in native mode");
                #[allow(unreachable_code)]
                Err::<&'static str, &'static str>("unreachable")
            },
        )
        .await
        .expect_err("selected native transport failed");

        assert_eq!(error.failed_path, UiTransportPath::NativeServer);
        assert!(!error.fallback_attempted);
        assert_eq!(error.native_error.as_deref(), Some("native unavailable"));
        assert_eq!(error.graphql_error, None);
    }

    #[tokio::test]
    async fn selected_transport_returns_graphql_success_without_calling_native() {
        let result = execute_selected_transport(
            "cart",
            UiTransportPath::Graphql,
            || async {
                panic!("native transport must not run in graphql mode");
                #[allow(unreachable_code)]
                Err::<&'static str, &'static str>("unreachable")
            },
            || async { Ok::<_, &'static str>("graphql") },
        )
        .await
        .expect("graphql success should be returned");

        assert_eq!(result, "graphql");
    }

    #[tokio::test]
    async fn selected_transport_returns_graphql_error_without_calling_native() {
        let error = execute_selected_transport(
            "cart",
            UiTransportPath::Graphql,
            || async {
                panic!("native transport must not run in graphql mode");
                #[allow(unreachable_code)]
                Err::<&'static str, &'static str>("unreachable")
            },
            || async { Err::<&'static str, &'static str>("graphql unauthorized") },
        )
        .await
        .expect_err("selected graphql transport failed");

        assert_eq!(error.failed_path, UiTransportPath::Graphql);
        assert!(!error.fallback_attempted);
        assert_eq!(error.native_error, None);
        assert_eq!(error.graphql_error.as_deref(), Some("graphql unauthorized"));
    }

    #[tokio::test]
    async fn fallback_succeeds_on_primary_native() {
        let result = execute_with_fallback(
            "pricing",
            UiTransportPath::NativeServer,
            || async { Ok::<_, &'static str>("native success") },
            || async {
                panic!("graphql must not be called when native succeeds");
                #[allow(unreachable_code)]
                Ok::<_, &'static str>("graphql")
            },
        )
        .await
        .expect("primary native should succeed");

        assert_eq!(result, "native success");
    }

    #[tokio::test]
    async fn fallback_recovers_via_secondary_graphql_when_primary_native_fails() {
        let result = execute_with_fallback(
            "pricing",
            UiTransportPath::NativeServer,
            || async { Err::<&'static str, _>("native degraded") },
            || async { Ok::<_, &'static str>("graphql recovered") },
        )
        .await
        .expect("secondary graphql should recover");

        assert_eq!(result, "graphql recovered");
    }

    #[tokio::test]
    async fn fallback_succeeds_on_primary_graphql() {
        let result = execute_with_fallback(
            "pricing",
            UiTransportPath::Graphql,
            || async {
                panic!("native must not be called when graphql succeeds");
                #[allow(unreachable_code)]
                Ok::<_, &'static str>("native")
            },
            || async { Ok::<_, &'static str>("graphql success") },
        )
        .await
        .expect("primary graphql should succeed");

        assert_eq!(result, "graphql success");
    }

    #[tokio::test]
    async fn fallback_recovers_via_secondary_native_when_primary_graphql_fails() {
        let result = execute_with_fallback(
            "pricing",
            UiTransportPath::Graphql,
            || async { Ok::<_, &'static str>("native recovered") },
            || async { Err::<&'static str, _>("graphql network timeout") },
        )
        .await
        .expect("secondary native should recover");

        assert_eq!(result, "native recovered");
    }

    #[tokio::test]
    async fn fallback_fails_closed_when_both_paths_fail() {
        let error = execute_with_fallback(
            "pricing",
            UiTransportPath::NativeServer,
            || async { Err::<(), _>("native server error 500") },
            || async { Err::<(), _>("graphql network reset") },
        )
        .await
        .expect_err("both transports failing must return fallback_failed error");

        assert_eq!(error.surface, "pricing");
        assert_eq!(error.failed_path, UiTransportPath::Graphql);
        assert!(error.fallback_attempted);
        assert_eq!(
            error.native_error.as_deref(),
            Some("native server error 500")
        );
        assert_eq!(
            error.graphql_error.as_deref(),
            Some("graphql network reset")
        );
    }

    #[tokio::test]
    async fn transport_policy_respects_fallback_disallowed() {
        let error = execute_transport_policy(
            "orders",
            UiTransportPath::NativeServer,
            UiTransportRetrySafety::AtMostOnce,
            false,
            || async { Err::<(), _>("native failed") },
            || async {
                panic!("graphql fallback must not be attempted when disallowed");
                #[allow(unreachable_code)]
                Ok::<(), &'static str>(())
            },
        )
        .await
        .expect_err("should fail with native error without fallback");

        assert!(!error.fallback_attempted);
        assert_eq!(error.failed_path, UiTransportPath::NativeServer);
    }

    #[tokio::test]
    async fn transport_policy_does_not_fallback_at_most_once_even_when_enabled() {
        let error = execute_transport_policy(
            "orders",
            UiTransportPath::NativeServer,
            UiTransportRetrySafety::AtMostOnce,
            true,
            || async { Err::<(), _>("native failed after possible side effect") },
            || async {
                panic!("unsafe mutation fallback must not be attempted");
                #[allow(unreachable_code)]
                Ok::<(), &'static str>(())
            },
        )
        .await
        .expect_err("at-most-once operations must not fallback");

        assert!(!error.fallback_attempted);
        assert_eq!(error.failed_path, UiTransportPath::NativeServer);
    }

    #[tokio::test]
    async fn transport_policy_respects_fallback_allowed() {
        let result = execute_transport_policy(
            "orders",
            UiTransportPath::NativeServer,
            UiTransportRetrySafety::SafeToRetry,
            true,
            || async { Err::<&'static str, _>("native failed") },
            || async { Ok::<_, &'static str>("graphql fallback success") },
        )
        .await
        .expect("should succeed via fallback when allowed");

        assert_eq!(result, "graphql fallback success");
    }
}
