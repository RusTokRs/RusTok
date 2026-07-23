use crate::ConsumerPropertyEditorRuntime;
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageBuilderAdminFacadeError {
    pub message: String,
    pub stable_code: Option<String>,
}

impl PageBuilderAdminFacadeError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            stable_code: None,
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

impl std::error::Error for PageBuilderAdminFacadeError {}
