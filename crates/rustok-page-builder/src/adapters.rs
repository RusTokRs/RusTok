use crate::dto::PageBuilderCapabilityRequest;
use crate::transport::{PageBuilderTransportError, PageBuilderTransportSuccess};

#[cfg(feature = "server")]
use crate::service::{
    AuthorizedPageBuilderHandlers, PageBuilderCapabilityService, PageBuilderRequestAuth,
};
#[cfg(feature = "server")]
use crate::transport::{dispatch_graphql_envelope, dispatch_leptos_server_function_envelope};
#[cfg(feature = "server")]
use rustok_api::PortContext;
use serde::{Deserialize, Serialize};

/// Framework-neutral GraphQL endpoint payload for the page-builder capability bridge.
///
/// Host GraphQL schemas should expose this shape (or a one-to-one generated equivalent) and
/// delegate execution to [`handle_page_builder_graphql_endpoint`] instead of calling provider
/// services directly. Keeping the endpoint request tagged by `PageBuilderCapabilityRequest`
/// prevents GraphQL-local aliases for capability names or payload variants.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PageBuilderGraphqlEndpointInput {
    pub request: PageBuilderCapabilityRequest,
}

/// Framework-neutral Leptos `#[server]` endpoint payload for the page-builder capability bridge.
///
/// The actual Leptos server function wrapper is expected to deserialize this payload and call
/// [`handle_page_builder_leptos_server_function_endpoint`], preserving the same canonical
/// request/response envelope used by GraphQL.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PageBuilderLeptosServerFunctionInput {
    pub request: PageBuilderCapabilityRequest,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PageBuilderEndpointSuccess {
    pub envelope: PageBuilderTransportSuccess,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PageBuilderEndpointError {
    pub envelope: PageBuilderTransportError,
}

pub type PageBuilderEndpointResult = Result<PageBuilderEndpointSuccess, PageBuilderEndpointError>;

impl From<PageBuilderTransportSuccess> for PageBuilderEndpointSuccess {
    fn from(envelope: PageBuilderTransportSuccess) -> Self {
        Self { envelope }
    }
}

impl From<PageBuilderTransportError> for PageBuilderEndpointError {
    fn from(envelope: PageBuilderTransportError) -> Self {
        Self { envelope }
    }
}

/// Canonical GraphQL endpoint handler seam.
#[cfg(feature = "server")]
pub async fn handle_page_builder_graphql_endpoint<S>(
    handlers: &AuthorizedPageBuilderHandlers<S>,
    context: &PortContext,
    auth: &PageBuilderRequestAuth,
    input: PageBuilderGraphqlEndpointInput,
) -> PageBuilderEndpointResult
where
    S: PageBuilderCapabilityService,
{
    dispatch_graphql_envelope(handlers, context, auth, input.request)
        .await
        .map(PageBuilderEndpointSuccess::from)
        .map_err(PageBuilderEndpointError::from)
}

/// Canonical Leptos server-function endpoint handler seam.
#[cfg(feature = "server")]
pub async fn handle_page_builder_leptos_server_function_endpoint<S>(
    handlers: &AuthorizedPageBuilderHandlers<S>,
    context: &PortContext,
    auth: &PageBuilderRequestAuth,
    input: PageBuilderLeptosServerFunctionInput,
) -> PageBuilderEndpointResult
where
    S: PageBuilderCapabilityService,
{
    dispatch_leptos_server_function_envelope(handlers, context, auth, input.request)
        .await
        .map(PageBuilderEndpointSuccess::from)
        .map_err(PageBuilderEndpointError::from)
}
