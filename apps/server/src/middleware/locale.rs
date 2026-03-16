/// Locale extraction middleware for RusToK
///
/// Reads the `Accept-Language` HTTP header and injects a `rustok_core::i18n::Locale`
/// into the Axum request as an `Extension<Locale>`. Controllers and GraphQL resolvers
/// extract it with `Extension(locale): Extension<Locale>`.
///
/// Locale resolution chain (Phase 0):
///   Accept-Language header → "en"
///
/// Phase 1 will extend this to: Accept-Language → tenant.default_locale → "en".

use axum::{extract::Request, middleware::Next, response::Response};
use rustok_core::i18n::{extract_locale_from_header, Locale};

/// Axum middleware that resolves and injects the request locale.
pub async fn resolve_locale(mut request: Request, next: Next) -> Response {
    let accept_language = request
        .headers()
        .get(axum::http::header::ACCEPT_LANGUAGE)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);

    let locale = extract_locale_from_header(accept_language.as_deref());
    request.extensions_mut().insert(locale);

    next.run(request).await
}
