use axum::{
    body::Body,
    http::{
        HeaderMap, HeaderValue, Request, StatusCode,
        header::{CACHE_CONTROL, COOKIE, SET_COOKIE},
    },
    middleware::Next,
    response::{IntoResponse, Response},
};

/// Bind the guest-cart capability to the current HTTP request.
///
/// The cart domain persists only a SHA-256 digest. The plaintext capability is
/// accepted from a dedicated header or HttpOnly cookie, carried through a
/// task-local request scope, and emitted only when a new guest cart is created.
pub async fn resolve(request: Request<Body>, next: Next) -> Response {
    let presented_token = match extract_presented_token(request.headers()) {
        Ok(token) => token,
        Err(message) => return (StatusCode::UNAUTHORIZED, message).into_response(),
    };

    let (mut response, issued_token) =
        crate::with_guest_cart_request_scope(presented_token, async move {
            let response = next.run(request).await;
            let issued_token = crate::issued_guest_cart_token();
            (response, issued_token)
        })
        .await;

    if let Some(token) = issued_token {
        if let Ok(header_value) = HeaderValue::from_str(&token) {
            response
                .headers_mut()
                .insert(crate::GUEST_CART_TOKEN_HEADER, header_value);
        }

        let cookie = format!(
            "{}={}; Path=/; HttpOnly; SameSite=Lax; Max-Age=2592000",
            crate::GUEST_CART_TOKEN_COOKIE,
            token
        );
        if let Ok(cookie_value) = HeaderValue::from_str(&cookie) {
            response.headers_mut().append(SET_COOKIE, cookie_value);
        }
        response
            .headers_mut()
            .insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    }

    response
}

fn extract_presented_token(headers: &HeaderMap) -> Result<Option<String>, &'static str> {
    let header_token = headers
        .get(crate::GUEST_CART_TOKEN_HEADER)
        .and_then(|value| value.to_str().ok())
        .and_then(crate::normalize_presented_guest_cart_token);
    let cookie_token = extract_cookie_token(headers);

    match (header_token, cookie_token) {
        (Some(header), Some(cookie)) if header != cookie => {
            Err("Conflicting guest cart access tokens")
        }
        (Some(header), _) => Ok(Some(header)),
        (_, Some(cookie)) => Ok(Some(cookie)),
        (None, None) => Ok(None),
    }
}

fn extract_cookie_token(headers: &HeaderMap) -> Option<String> {
    let raw = headers.get(COOKIE)?.to_str().ok()?;
    raw.split(';').find_map(|entry| {
        let (name, value) = entry.trim().split_once('=')?;
        if name != crate::GUEST_CART_TOKEN_COOKIE {
            return None;
        }
        crate::normalize_presented_guest_cart_token(value)
    })
}

#[cfg(test)]
mod tests {
    use super::extract_presented_token;
    use axum::http::HeaderMap;

    fn token(seed: char) -> String {
        std::iter::repeat_n(seed, 64).collect()
    }

    #[test]
    fn matching_header_and_cookie_are_accepted() {
        let token = token('a');
        let mut headers = HeaderMap::new();
        headers.insert(
            crate::GUEST_CART_TOKEN_HEADER,
            token.parse().expect("header token"),
        );
        headers.insert(
            axum::http::header::COOKIE,
            format!("{}={token}", crate::GUEST_CART_TOKEN_COOKIE)
                .parse()
                .expect("cookie"),
        );

        assert_eq!(extract_presented_token(&headers), Ok(Some(token)));
    }

    #[test]
    fn conflicting_capabilities_fail_closed() {
        let mut headers = HeaderMap::new();
        headers.insert(
            crate::GUEST_CART_TOKEN_HEADER,
            token('a').parse().expect("header token"),
        );
        headers.insert(
            axum::http::header::COOKIE,
            format!("{}={}", crate::GUEST_CART_TOKEN_COOKIE, token('b'))
                .parse()
                .expect("cookie"),
        );

        assert!(extract_presented_token(&headers).is_err());
    }
}
