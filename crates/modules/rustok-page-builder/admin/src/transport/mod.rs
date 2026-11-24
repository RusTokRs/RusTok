use crate::{ConsumerPropertyEditorRuntime, PageBuilderAdminProviderStatus};
use rustok_page_builder::dto::{PageBuilderCapabilityRequest, PageBuilderCapabilityResponse};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

#[cfg(not(target_arch = "wasm32"))]
pub type PageBuilderAdminFacadeFuture = Pin<
    Box<
        dyn Future<Output = Result<PageBuilderCapabilityResponse, PageBuilderAdminFacadeError>>
            + Send
            + 'static,
    >,
>;

#[cfg(target_arch = "wasm32")]
pub type PageBuilderAdminFacadeFuture = Pin<
    Box<
        dyn Future<Output = Result<PageBuilderCapabilityResponse, PageBuilderAdminFacadeError>>
            + 'static,
    >,
>;

/// FFA boundary owned by the Page Builder admin package.
///
/// Implementations may use a native Leptos server function or GraphQL, but the editor and its
/// controller only see canonical capability and consumer-property envelopes and never branch on
/// transport. The facade itself is `Send + Sync` so it can live in Leptos owner context. Native SSR
/// futures are `Send` for Axum handlers; wasm-client futures remain local for `spawn_local`.
pub trait PageBuilderAdminFacade: Send + Sync {
    fn execute(&self, request: PageBuilderCapabilityRequest) -> PageBuilderAdminFacadeFuture;

    /// Optional consumer-owned property surface for the selected document.
    ///
    /// Page Builder renders the registered schema and invokes the supplied port. The consumer keeps
    /// ownership of persistence, optimistic revision policy and transport selection.
    fn consumer_properties(&self) -> Option<Arc<ConsumerPropertyEditorRuntime>> {
        None
    }

    /// Optional provider control snapshot for this concrete consumer composition.
    ///
    /// Rollout flags must match the flags used by the facade's capability pipeline. Observed health
    /// remains optional; returning no snapshot is intentionally different from claiming the provider
    /// is healthy. The admin runtime may only use this status to narrow host tenant/RBAC capabilities.
    fn provider_status(&self) -> Option<PageBuilderAdminProviderStatus> {
        None
    }
}

impl<T> PageBuilderAdminFacade for Arc<T>
where
    T: PageBuilderAdminFacade + ?Sized,
{
    fn execute(&self, request: PageBuilderCapabilityRequest) -> PageBuilderAdminFacadeFuture {
        self.as_ref().execute(request)
    }

    fn consumer_properties(&self) -> Option<Arc<ConsumerPropertyEditorRuntime>> {
        self.as_ref().consumer_properties()
    }

    fn provider_status(&self) -> Option<PageBuilderAdminProviderStatus> {
        self.as_ref().provider_status()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageBuilderAdminFacadeError {
    pub message: String,
    pub stable_code: Option<String>,
}

impl PageBuilderAdminFacadeError {
    /// Rehydrates stable save codes from server-function suffixes and GraphQL business errors.
    pub fn new(message: impl Into<String>) -> Self {
        let message = message.into();
        let stable_code = if message.contains("PAGE_DOCUMENT_REVISION_CONFLICT") {
            Some("REVISION_CONFLICT")
        } else if message.contains("PAGE_DOCUMENT_SAVE_IDEMPOTENCY_CONFLICT") {
            Some("PAGE_DOCUMENT_SAVE_IDEMPOTENCY_CONFLICT")
        } else if message.contains("PAGE_DOCUMENT_SAVE_RECEIPT_EXPIRED") {
            Some("PAGE_DOCUMENT_SAVE_RECEIPT_EXPIRED")
        } else if message.contains("PAGE_DOCUMENT_SAVE_OUTCOME_UNKNOWN") {
            Some("PAGE_DOCUMENT_SAVE_OUTCOME_UNKNOWN")
        } else if message.ends_with(" (REVISION_CONFLICT)") {
            Some("REVISION_CONFLICT")
        } else {
            None
        };
        let message = stable_code
            .and_then(|code| message.strip_suffix(&format!(" ({code})")))
            .unwrap_or(&message)
            .to_string();
        Self {
            message,
            stable_code: stable_code.map(str::to_string),
        }
    }

    pub fn with_stable_code(message: impl Into<String>, stable_code: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            stable_code: Some(stable_code.into()),
        }
    }
}

impl std::fmt::Display for PageBuilderAdminFacadeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.stable_code.as_deref() {
            Some(code) => write!(formatter, "{} ({code})", self.message),
            None => formatter.write_str(&self.message),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_function_error_round_trip_preserves_stable_save_codes() {
        for code in ["REVISION_CONFLICT", "PAGE_DOCUMENT_SAVE_OUTCOME_UNKNOWN"] {
            let server_error = PageBuilderAdminFacadeError::with_stable_code(
                "Page Builder save failed",
                code,
            )
            .to_string();
            let client_error = PageBuilderAdminFacadeError::new(format!(
                "Server function error: {server_error}"
            ));
            assert_eq!(client_error.stable_code.as_deref(), Some(code));
            assert_eq!(
                client_error.message,
                "Server function error: Page Builder save failed"
            );
        }
    }

    #[test]
    fn graphql_revision_conflict_code_is_normalized_for_both_hosts() {
        let error = PageBuilderAdminFacadeError::new(
            "GraphQL error: Page document changed concurrently (PAGE_DOCUMENT_REVISION_CONFLICT): stale revision",
        );
        assert_eq!(error.stable_code.as_deref(), Some("REVISION_CONFLICT"));
    }

    #[test]
    fn unknown_save_outcome_code_survives_graphql_and_server_function_wrappers() {
        let error = PageBuilderAdminFacadeError::new(
            "GraphQL error: PAGE_DOCUMENT_SAVE_OUTCOME_UNKNOWN: preserve the draft",
        );
        assert_eq!(
            error.stable_code.as_deref(),
            Some("PAGE_DOCUMENT_SAVE_OUTCOME_UNKNOWN")
        );
    }

    #[test]
    fn idempotency_conflict_and_expired_receipt_codes_survive_wrappers() {
        for code in [
            "PAGE_DOCUMENT_SAVE_IDEMPOTENCY_CONFLICT",
            "PAGE_DOCUMENT_SAVE_RECEIPT_EXPIRED",
        ] {
            let error = PageBuilderAdminFacadeError::new(format!(
                "Server function error: save failed ({code})"
            ));
            assert_eq!(error.stable_code.as_deref(), Some(code));
            assert_eq!(error.message, "Server function error: save failed");
        }
    }

    #[test]
    fn unknown_parenthesized_messages_are_not_misclassified() {
        let error = PageBuilderAdminFacadeError::new("provider returned (CUSTOM_CODE)");
        assert_eq!(error.stable_code, None);
        assert_eq!(error.message, "provider returned (CUSTOM_CODE)");
    }
}

impl std::error::Error for PageBuilderAdminFacadeError {}
