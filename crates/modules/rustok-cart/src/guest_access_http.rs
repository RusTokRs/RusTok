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

    let presented_token_present = presented_token.is_some();
    let (mut response, issued_token) =
        crate::with_guest_cart_request_scope(presented_token, async move {
            let response = next.run(request).await;
            let issued_token = crate::issued_guest_cart_token();
            (response, issued_token)
        })
        .await;

    if presented_token_present || issued_token.is_some() {
        response
            .headers_mut()
            .insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    }

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
    }

    response
}

fn extract_presented_token(headers: &HeaderMap) -> Result<Option<String>, &'static str> {
    let header_token = extract_header_token(headers)?;
    let cookie_token = extract_cookie_token(headers)?;

    match (header_token, cookie_token) {
        (Some(header), Some(cookie)) if header != cookie => {
            Err("Conflicting guest cart access tokens")
        }
        (Some(header), _) => Ok(Some(header)),
        (_, Some(cookie)) => Ok(Some(cookie)),
        (None, None) => Ok(None),
    }
}

fn extract_header_token(headers: &HeaderMap) -> Result<Option<String>, &'static str> {
    let values = headers.get_all(crate::GUEST_CART_TOKEN_HEADER);
    let mut token = None;

    for value in values.iter() {
        let text = value
            .to_str()
            .map_err(|_| "Invalid guest cart access token")?;
        let normalized = crate::normalize_presented_guest_cart_token(text)
            .ok_or("Invalid guest cart access token")?;

        if token.is_some() {
            return Err("Duplicate guest cart access tokens");
        }
        token = Some(normalized);
    }

    Ok(token)
}

fn extract_cookie_token(headers: &HeaderMap) -> Result<Option<String>, &'static str> {
    let mut token = None;

    for raw_header in headers.get_all(COOKIE).iter() {
        let raw = raw_header
            .to_str()
            .map_err(|_| "Invalid guest cart cookie")?;

        for entry in raw.split(';') {
            let entry = entry.trim();
            let Some((name, value)) = entry.split_once('=') else {
                continue;
            };
            if name != crate::GUEST_CART_TOKEN_COOKIE {
                continue;
            }

            let normalized = crate::normalize_presented_guest_cart_token(value)
                .ok_or("Invalid guest cart access token")?;
            if token.is_some() {
                return Err("Duplicate guest cart access tokens");
            }
            token = Some(normalized);
        }
    }

    Ok(token)
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
fn duplicate_header_capabilities_fail_closed() {
    let mut headers = HeaderMap::new();
    headers.append(
        crate::GUEST_CART_TOKEN_HEADER,
        token('a').parse().expect("header token"),
    );
    headers.append(
        crate::GUEST_CART_TOKEN_HEADER,
        token('a').parse().expect("duplicate header token"),
    );

    assert_eq!(
        extract_presented_token(&headers),
        Err("Duplicate guest cart access tokens")
    );
}


    #[test]
    fn duplicate_cookie_capabilities_fail_closed() {
    let token = token('a');
    let mut headers = HeaderMap::new();
    headers.insert(
        axum::http::header::COOKIE,
        format!(
            "{}={}; {}={}",
            crate::GUEST_CART_TOKEN_COOKIE,
            token,
            crate::GUEST_CART_TOKEN_COOKIE,
            token
        )
        .parse()
        .expect("cookie"),
    );

    assert_eq!(
        extract_presented_token(&headers),
        Err("Duplicate guest cart access tokens")
    );
}


    #[test]
    fn invalid_header_does_not_fall_back_to_cookie_capability() {
    let token = token('a');
    let mut headers = HeaderMap::new();
    headers.insert(
        crate::GUEST_CART_TOKEN_HEADER,
        "invalid-token".parse().expect("invalid header"),
    );
    headers.insert(
        axum::http::header::COOKIE,
        format!("{}={token}", crate::GUEST_CART_TOKEN_COOKIE)
            .parse()
            .expect("cookie"),
    );

    assert_eq!(
        extract_presented_token(&headers),
        Err("Invalid guest cart access token")
    );
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
