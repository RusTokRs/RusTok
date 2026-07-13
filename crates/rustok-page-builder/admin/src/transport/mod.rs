use rustok_page_builder::dto::{
    PageBuilderCapabilityRequest, PageBuilderCapabilityResponse,
};
use std::future::Future;
use std::pin::Pin;

pub type PageBuilderAdminFacadeFuture = Pin<
    Box<
        dyn Future<
                Output = Result<PageBuilderCapabilityResponse, PageBuilderAdminFacadeError>,
            > + 'static,
    >,
>;

/// FFA boundary owned by the Page Builder admin package.
///
/// Implementations may use a native Leptos server function or GraphQL, but the editor and its
/// controller only see the canonical capability envelope and never branch on transport. The facade
/// itself is `Send + Sync` so it can live in Leptos owner context; individual browser futures remain
/// local and are executed through `spawn_local`.
pub trait PageBuilderAdminFacade: Send + Sync {
    fn execute(&self, request: PageBuilderCapabilityRequest) -> PageBuilderAdminFacadeFuture;
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

    pub fn with_stable_code(
        message: impl Into<String>,
        stable_code: impl Into<String>,
    ) -> Self {
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
