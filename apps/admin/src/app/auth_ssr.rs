use axum::http::{HeaderMap, request::Parts};
use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use leptos::prelude::*;
use leptos_auth::{AuthSession, AuthUser, ServerAuthSnapshot};
use rustok_web::CspNonce;

pub const ADMIN_SESSION_COOKIE: &str = "rustok-admin-session-v1";
pub const ADMIN_USER_COOKIE: &str = "rustok-admin-user-v1";

/// Transitional compatibility bootstrap for the existing browser LocalStorage authentication.
///
/// Classic SSR ultimately should use a server-issued HttpOnly session cookie. Until that migration
/// is complete this tiny script mirrors the already-existing serialized session and user into
/// same-origin cookies, reloads once, and lets the Rust server build a request-scoped auth snapshot.
/// It does not implement authentication or token refresh.
pub const AUTH_COOKIE_BOOTSTRAP_JS: &str = r#"
(() => {
  const sessionKey = "rustok-admin-session";
  const userKey = "rustok-admin-user";
  const sessionCookie = "rustok-admin-session-v1";
  const userCookie = "rustok-admin-user-v1";
  const textEncoder = new TextEncoder();
  const base64url = (value) => {
    const bytes = textEncoder.encode(value);
    let binary = "";
    for (const byte of bytes) binary += String.fromCharCode(byte);
    return btoa(binary).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/g, "");
  };
  const cookies = Object.fromEntries(document.cookie.split(";").map((part) => {
    const index = part.indexOf("=");
    return index < 0 ? [part.trim(), ""] : [part.slice(0, index).trim(), part.slice(index + 1)];
  }));
  const secure = location.protocol === "https:" ? "; Secure" : "";
  const setCookie = (name, value) => {
    document.cookie = `${name}=${value}; Path=/; SameSite=Lax${secure}`;
  };
  const clearCookie = (name) => {
    document.cookie = `${name}=; Path=/; Max-Age=0; SameSite=Lax${secure}`;
  };
  let changed = false;
  try {
    const session = localStorage.getItem(sessionKey);
    const user = localStorage.getItem(userKey);
    if (session && user) {
      const encodedSession = base64url(session);
      const encodedUser = base64url(user);
      if (cookies[sessionCookie] !== encodedSession) {
        setCookie(sessionCookie, encodedSession);
        changed = true;
      }
      if (cookies[userCookie] !== encodedUser) {
        setCookie(userCookie, encodedUser);
        changed = true;
      }
    } else if (cookies[sessionCookie] || cookies[userCookie]) {
      clearCookie(sessionCookie);
      clearCookie(userCookie);
      changed = true;
    }
  } catch (_) {
    return;
  }
  if (changed) location.reload();
})();
"#;

#[component]
pub fn AuthCookieBootstrap() -> impl IntoView {
    let nonce = use_context::<CspNonce>()
        .or_else(crate::app::security::request_csp_nonce)
        .map(|nonce| nonce.as_str().to_string());
    view! {
        <script
            nonce=nonce
            data-rustok-auth-bootstrap="local-storage-cookie-v1"
            inner_html=AUTH_COOKIE_BOOTSTRAP_JS
        ></script>
    }
}

pub fn request_auth_snapshot() -> ServerAuthSnapshot {
    use_context::<Parts>()
        .map(|parts| auth_snapshot_from_headers(&parts.headers))
        .unwrap_or_default()
}

pub fn auth_snapshot_from_headers(headers: &HeaderMap) -> ServerAuthSnapshot {
    let cookies = headers
        .get(axum::http::header::COOKIE)
        .and_then(|value| value.to_str().ok())
        .map(parse_cookie_header)
        .unwrap_or_default();
    let session = cookies
        .get(ADMIN_SESSION_COOKIE)
        .and_then(|value| decode_cookie_json::<AuthSession>(value));
    let user = cookies
        .get(ADMIN_USER_COOKIE)
        .and_then(|value| decode_cookie_json::<AuthUser>(value));
    ServerAuthSnapshot { session, user }
}

fn parse_cookie_header(header: &str) -> std::collections::HashMap<String, String> {
    header
        .split(';')
        .filter_map(|part| {
            let (name, value) = part.trim().split_once('=')?;
            Some((name.trim().to_string(), value.trim().to_string()))
        })
        .collect()
}

fn decode_cookie_json<T>(value: &str) -> Option<T>
where
    T: serde::de::DeserializeOwned,
{
    let bytes = URL_SAFE_NO_PAD.decode(value.trim()).ok()?;
    serde_json::from_slice(&bytes).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encoded<T: serde::Serialize>(value: &T) -> String {
        URL_SAFE_NO_PAD.encode(serde_json::to_vec(value).unwrap())
    }

    #[test]
    fn auth_bootstrap_renders_the_request_nonce() {
        let nonce = CspNonce::generate();
        let owner = Owner::new();
        let html = owner.with(|| {
            provide_context(nonce.clone());
            view! { <AuthCookieBootstrap/> }.to_html()
        });

        assert!(html.contains(format!(r#"nonce="{}""#, nonce.as_str()).as_str()));
        assert!(html.contains("data-rustok-auth-bootstrap=\"local-storage-cookie-v1\""));
    }

    #[test]
    fn request_cookie_snapshot_round_trips_auth_models() {
        let session = AuthSession {
            token: "token".to_string(),
            refresh_token: "refresh".to_string(),
            expires_at: 42,
            tenant: "demo".to_string(),
        };
        let user = AuthUser {
            id: "user".to_string(),
            email: "user@example.test".to_string(),
            name: Some("User".to_string()),
            role: "admin".to_string(),
        };
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::COOKIE,
            format!(
                "{ADMIN_SESSION_COOKIE}={}; {ADMIN_USER_COOKIE}={}",
                encoded(&session),
                encoded(&user),
            )
            .parse()
            .unwrap(),
        );
        let snapshot = auth_snapshot_from_headers(&headers);
        assert_eq!(snapshot.session, Some(session));
        assert_eq!(snapshot.user, Some(user));
    }

    #[test]
    fn malformed_cookie_is_ignored_without_panicking() {
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::COOKIE,
            format!("{ADMIN_SESSION_COOKIE}=invalid").parse().unwrap(),
        );
        assert_eq!(
            auth_snapshot_from_headers(&headers),
            ServerAuthSnapshot::default()
        );
    }
}
