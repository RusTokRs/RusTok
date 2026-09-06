use crate::dto::{BuilderCapabilityKind, PageBuilderCapabilityResponse, PageBuilderErrorKind};

#[cfg(feature = "server")]
use crate::dto::PageBuilderCapabilityRequest;
#[cfg(feature = "server")]
use crate::service::{
    AuthorizedPageBuilderHandlers, PageBuilderCapabilityService, PageBuilderRequestAuth,
    PageBuilderServiceError, PageBuilderServiceResult,
};
#[cfg(feature = "server")]
use rustok_api::PortContext;
use serde::{Deserialize, Serialize};

/// Canonical transport marker for adapters that expose page-builder capability envelopes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PageBuilderTransportKind {
    Graphql,
    LeptosServerFunction,
    FutureMobileBridge,
}

impl PageBuilderTransportKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Graphql => "graphql",
            Self::LeptosServerFunction => "leptos_server_function",
            Self::FutureMobileBridge => "future_mobile_bridge",
        }
    }
}

/// Transport-neutral success envelope shared by GraphQL, Leptos `#[server]` and future mobile bridges.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PageBuilderTransportSuccess {
    pub transport: PageBuilderTransportKind,
    pub capability: BuilderCapabilityKind,
    pub response: PageBuilderCapabilityResponse,
}

/// Transport-neutral error envelope. Adapters should map this shape to their local error type
/// without inventing transport-local kind/code names.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PageBuilderTransportError {
    pub transport: PageBuilderTransportKind,
    pub capability: BuilderCapabilityKind,
    pub kind: PageBuilderErrorKind,
    pub stable_code: Option<String>,
    pub message: String,
}

impl PageBuilderTransportError {
    #[cfg(feature = "server")]
    pub fn from_service_error(
        transport: PageBuilderTransportKind,
        capability: BuilderCapabilityKind,
        error: PageBuilderServiceError,
    ) -> Self {
        Self {
            transport,
            capability,
            kind: error.kind(),
            stable_code: error.stable_code().map(str::to_string),
            message: error.to_string(),
        }
    }
}

#[cfg(feature = "server")]
pub async fn dispatch_transport_envelope<S>(
    handlers: &AuthorizedPageBuilderHandlers<S>,
    transport: PageBuilderTransportKind,
    context: &PortContext,
    auth: &PageBuilderRequestAuth,
    request: PageBuilderCapabilityRequest,
) -> Result<PageBuilderTransportSuccess, PageBuilderTransportError>
where
    S: PageBuilderCapabilityService,
{
    let capability = request.capability();
    match handlers.handle(context, auth, request).await {
        Ok(response) => Ok(PageBuilderTransportSuccess {
            transport,
            capability,
            response,
        }),
        Err(error) => Err(PageBuilderTransportError::from_service_error(
            transport, capability, error,
        )),
    }
}

#[cfg(feature = "server")]
pub async fn dispatch_graphql_envelope<S>(
    handlers: &AuthorizedPageBuilderHandlers<S>,
    context: &PortContext,
    auth: &PageBuilderRequestAuth,
    request: PageBuilderCapabilityRequest,
) -> Result<PageBuilderTransportSuccess, PageBuilderTransportError>
where
    S: PageBuilderCapabilityService,
{
    dispatch_transport_envelope(
        handlers,
        PageBuilderTransportKind::Graphql,
        context,
        auth,
        request,
    )
    .await
}

#[cfg(feature = "server")]
pub async fn dispatch_leptos_server_function_envelope<S>(
    handlers: &AuthorizedPageBuilderHandlers<S>,
    context: &PortContext,
    auth: &PageBuilderRequestAuth,
    request: PageBuilderCapabilityRequest,
) -> Result<PageBuilderTransportSuccess, PageBuilderTransportError>
where
    S: PageBuilderCapabilityService,
{
    dispatch_transport_envelope(
        handlers,
        PageBuilderTransportKind::LeptosServerFunction,
        context,
        auth,
        request,
    )
    .await
}

#[cfg(feature = "server")]
pub fn transport_error_result<T>(error: PageBuilderTransportError) -> PageBuilderServiceResult<T> {
    Err(PageBuilderServiceError::Runtime(format!(
        "{}:{}:{}",
        error.transport.as_str(),
        error.capability.as_str(),
        error.message
    )))
}
