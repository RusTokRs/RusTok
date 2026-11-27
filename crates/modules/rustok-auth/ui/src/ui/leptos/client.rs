use futures_util::{Sink, Stream};
use leptos::server_fn::{
    Bytes,
    client::{Client, browser::BrowserClient},
    error::{FromServerFnError, ServerFnErrorErr},
    request::browser::BrowserRequest,
    response::browser::BrowserResponse,
};

/// Leptos browser client for authenticated first-party server functions.
///
/// Authentication remains transport metadata: the bearer token and tenant are
/// read from the canonical browser auth storage and are never serialized into
/// a server-function input or exposed in rendered HTML.
#[derive(Debug, thiserror::Error)]
#[allow(dead_code)]
pub enum AuthorizedBrowserClientError {
    #[error("Authentication metadata could not be applied")]
    AuthMetadataFailed,
    #[error("A verified browser session is required")]
    SessionUnavailable,
    #[error("Tenant metadata could not be applied")]
    TenantMetadataFailed,
}

pub struct AuthorizedBrowserClient;

impl<Error, InputStreamError, OutputStreamError> Client<Error, InputStreamError, OutputStreamError>
    for AuthorizedBrowserClient
where
    Error: FromServerFnError,
    InputStreamError: FromServerFnError,
    OutputStreamError: FromServerFnError,
{
    type Request = BrowserRequest;
    type Response = BrowserResponse;

    fn send(request: Self::Request) -> impl Future<Output = Result<Self::Response, Error>> + Send {
        #[cfg(target_arch = "wasm32")]
        let expected_epoch = crate::transport::browser_epoch::read();
        #[cfg(target_arch = "wasm32")]
        let failure = (|| -> Result<(), AuthorizedBrowserClientError> {
            let epoch = expected_epoch
                .as_ref()
                .map_err(|_| AuthorizedBrowserClientError::SessionUnavailable)?;
            if !crate::transport::browser_epoch::matches(epoch) {
                return Err(AuthorizedBrowserClientError::SessionUnavailable);
            }
            // gloo's convenience setter throws and returns (). The underlying
            // web-sys API preserves the error so malformed auth metadata fails
            // before BrowserClient sends the request.
            let headers = request.headers().into_raw();
            // Never fall back to an ambient SSR cookie if the verified
            // memory projection was invalidated by another tab or transition.
            let session = super::storage::load_session()
                .map_err(|_| AuthorizedBrowserClientError::SessionUnavailable)?;
            headers
                .set(
                    "Authorization",
                    format!("Bearer {}", session.token).as_str(),
                )
                .map_err(|_| AuthorizedBrowserClientError::AuthMetadataFailed)?;
            headers
                .set("X-Tenant-ID", session.tenant.as_str())
                .map_err(|_| AuthorizedBrowserClientError::TenantMetadataFailed)?;
            headers
                .set("X-Tenant-Slug", session.tenant.as_str())
                .map_err(|_| AuthorizedBrowserClientError::TenantMetadataFailed)?;
            if !crate::transport::browser_epoch::matches(epoch) {
                return Err(AuthorizedBrowserClientError::SessionUnavailable);
            }
            Ok(())
        })()
        .err();
        #[cfg(not(target_arch = "wasm32"))]
        let failure: Option<AuthorizedBrowserClientError> = None;
        async move {
            if let Some(message) = failure {
                return Err(Error::from_server_fn_error(ServerFnErrorErr::Request(
                    message.to_string(),
                )));
            }
            #[cfg(target_arch = "wasm32")]
            if !expected_epoch
                .as_ref()
                .is_ok_and(crate::transport::browser_epoch::matches)
            {
                return Err(Error::from_server_fn_error(ServerFnErrorErr::Request(
                    AuthorizedBrowserClientError::SessionUnavailable.to_string(),
                )));
            }
            <BrowserClient as Client<Error, InputStreamError, OutputStreamError>>::send(request)
                .await
        }
    }

    fn open_websocket(
        path: &str,
    ) -> impl Future<
        Output = Result<
            (
                impl Stream<Item = Result<Bytes, Bytes>> + Send + 'static,
                impl Sink<Bytes> + Send + 'static,
            ),
            Error,
        >,
    > + Send {
        <BrowserClient as Client<Error, InputStreamError, OutputStreamError>>::open_websocket(path)
    }

    fn spawn(future: impl Future<Output = ()> + Send + 'static) {
        <BrowserClient as Client<Error, InputStreamError, OutputStreamError>>::spawn(future);
    }
}

#[cfg(test)]
mod tests {
    use super::AuthorizedBrowserClient;
    use leptos::server_fn::{
        client::Client, error::ServerFnError, request::browser::BrowserRequest,
        response::browser::BrowserResponse,
    };

    #[test]
    fn authenticated_client_preserves_the_standard_leptos_wire_types() {
        fn assert_client<C>()
        where
            C: Client<ServerFnError>,
            C::Request: Into<BrowserRequest>,
            C::Response: Into<BrowserResponse>,
        {
        }

        assert_client::<AuthorizedBrowserClient>();
    }
}
