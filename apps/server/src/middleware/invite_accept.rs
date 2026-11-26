use axum::{
    body::Body,
    extract::State,
    http::{Method, Request},
    middleware::Next,
    response::{IntoResponse, Response},
};

use crate::controllers::auth::{INVITE_ACCEPT_PATH, accept_invite};
use crate::services::server_runtime_context::ServerAuthRuntime;

/// Keep invite acceptance independent of channel resolution and guest-cart
/// credentials, as in the existing host stack. Dispatch to the registered
/// controller so body admission, error mapping and mutation have one owner.
pub async fn consume_once(
    State(ctx): State<ServerAuthRuntime>,
    request: Request<Body>,
    next: Next,
) -> Response {
    if request.method() != Method::POST || request.uri().path() != INVITE_ACCEPT_PATH {
        return next.run(request).await;
    }

    accept_invite(State(ctx), request).await.into_response()
}
