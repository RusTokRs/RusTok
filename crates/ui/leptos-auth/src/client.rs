use futures_util::{Sink, Stream};
use leptos::server_fn::{
    Bytes,
    client::{Client, browser::BrowserClient},
    error::FromServerFnError,
    request::browser::BrowserRequest,
    response::browser::BrowserResponse,
};

/// Leptos browser client for authenticated first-party server functions.
///
/// Authentication remains transport metadata: the bearer token and tenant are
/// read from the canonical browser auth storage and are never serialized into
/// a server-function input or exposed in rendered HTML.
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
        {
            let headers = request.headers();
            if let Some(token) = crate::storage::get_token() {
                headers.set("Authorization", format!("Bearer {token}").as_str());
            }
            if let Some(tenant) = crate::storage::get_tenant() {
                headers.set("X-Tenant-ID", tenant.as_str());
            }
        }

        <BrowserClient as Client<Error, InputStreamError, OutputStreamError>>::send(request)
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
