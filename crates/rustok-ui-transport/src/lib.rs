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

#[cfg(test)]
mod tests {
    use super::{UiTransportError, UiTransportPath, execute_selected_transport};
    use std::future::Future;
    use std::pin::Pin;
    use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

    fn block_on<F: Future>(future: F) -> F::Output {
        let waker = noop_waker();
        let mut context = Context::from_waker(&waker);
        let mut future = Box::pin(future);

        loop {
            match Pin::new(&mut future).poll(&mut context) {
                Poll::Ready(output) => return output,
                Poll::Pending => std::thread::yield_now(),
            }
        }
    }

    fn noop_waker() -> Waker {
        unsafe fn clone(_: *const ()) -> RawWaker {
            RawWaker::new(std::ptr::null(), &VTABLE)
        }
        unsafe fn wake(_: *const ()) {}
        unsafe fn wake_by_ref(_: *const ()) {}
        unsafe fn drop(_: *const ()) {}

        static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, wake, wake_by_ref, drop);
        let raw_waker = RawWaker::new(std::ptr::null(), &VTABLE);

        unsafe { Waker::from_raw(raw_waker) }
    }

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

    #[test]
    fn selected_transport_returns_native_success_without_calling_graphql() {
        let result = block_on(execute_selected_transport(
            "product",
            UiTransportPath::NativeServer,
            || async { Ok::<_, &'static str>("native") },
            || async {
                panic!("graphql transport must not run in native mode");
                #[allow(unreachable_code)]
                Err::<&'static str, &'static str>("unreachable")
            },
        ))
        .expect("native success should be returned");

        assert_eq!(result, "native");
    }

    #[test]
    fn selected_transport_returns_native_error_without_calling_graphql() {
        let error = block_on(execute_selected_transport(
            "product",
            UiTransportPath::NativeServer,
            || async { Err::<&'static str, _>("native unavailable") },
            || async {
                panic!("graphql transport must not run in native mode");
                #[allow(unreachable_code)]
                Err::<&'static str, &'static str>("unreachable")
            },
        ))
        .expect_err("selected native transport failed");

        assert_eq!(error.failed_path, UiTransportPath::NativeServer);
        assert!(!error.fallback_attempted);
        assert_eq!(error.native_error.as_deref(), Some("native unavailable"));
        assert_eq!(error.graphql_error, None);
    }

    #[test]
    fn selected_transport_returns_graphql_success_without_calling_native() {
        let result = block_on(execute_selected_transport(
            "cart",
            UiTransportPath::Graphql,
            || async {
                panic!("native transport must not run in graphql mode");
                #[allow(unreachable_code)]
                Err::<&'static str, &'static str>("unreachable")
            },
            || async { Ok::<_, &'static str>("graphql") },
        ))
        .expect("graphql success should be returned");

        assert_eq!(result, "graphql");
    }

    #[test]
    fn selected_transport_returns_graphql_error_without_calling_native() {
        let error = block_on(execute_selected_transport(
            "cart",
            UiTransportPath::Graphql,
            || async {
                panic!("native transport must not run in graphql mode");
                #[allow(unreachable_code)]
                Err::<&'static str, &'static str>("unreachable")
            },
            || async { Err::<&'static str, &'static str>("graphql unauthorized") },
        ))
        .expect_err("selected graphql transport failed");

        assert_eq!(error.failed_path, UiTransportPath::Graphql);
        assert!(!error.fallback_attempted);
        assert_eq!(error.native_error, None);
        assert_eq!(error.graphql_error.as_deref(), Some("graphql unauthorized"));
    }
}
