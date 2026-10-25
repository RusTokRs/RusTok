mod client;
pub mod components;
pub mod context;
pub mod hooks;
pub mod storage;
pub mod transport;

use rustok_ui_auth::{AuthError, AuthSession, AuthUser};

pub const ADMIN_TOKEN_KEY: &str = "rustok-admin-token";
pub const ADMIN_TENANT_KEY: &str = "rustok-admin-tenant";
pub const ADMIN_USER_KEY: &str = "rustok-admin-user";
pub const ADMIN_SESSION_KEY: &str = "rustok-admin-session";

/// HttpOnly cookie used by SSR hosts (backend-for-frontend) to hold
/// the refresh token on behalf of the browser. The value is
/// `{tenant_slug}.{refresh_token}` — tenant slugs and refresh tokens
/// never contain a dot, so the split is unambiguous.
pub const ADMIN_REFRESH_COOKIE: &str = "rustok-admin-refresh-v1";

/// 90 days: the server-side absolute refresh-lineage cap.
const ADMIN_REFRESH_COOKIE_MAX_AGE_SECS: u64 = 90 * 24 * 60 * 60;

pub fn refresh_cookie_value(tenant: &str, refresh_token: &str) -> String {
    format!("{tenant}.{refresh_token}")
}

pub fn parse_refresh_cookie_value(value: &str) -> Option<(&str, &str)> {
    let (tenant, token) = value.trim().split_once('.')?;
    (!tenant.is_empty() && !token.is_empty()).then_some((tenant, token))
}

/// Extract `(tenant, refresh_token)` from a raw `Cookie` header value.
pub fn extract_refresh_cookie(cookie_header: &str) -> Option<(String, String)> {
    cookie_header.split(';').find_map(|part| {
        let (name, value) = part.trim().split_once('=')?;
        if name.trim() != ADMIN_REFRESH_COOKIE {
            return None;
        }
        let (tenant, token) = parse_refresh_cookie_value(value)?;
        Some((tenant.to_string(), token.to_string()))
    })
}

/// `Set-Cookie` header value persisting the BFF refresh cookie.
pub fn refresh_cookie_header(tenant: &str, refresh_token: &str, secure: bool) -> String {
    let mut cookie = format!(
        "{ADMIN_REFRESH_COOKIE}={}; Path=/; HttpOnly; SameSite=Lax; Max-Age={ADMIN_REFRESH_COOKIE_MAX_AGE_SECS}",
        refresh_cookie_value(tenant, refresh_token)
    );
    if secure {
        cookie.push_str("; Secure");
    }
    cookie
}

/// `Set-Cookie` header value clearing the BFF refresh cookie.
pub fn clear_refresh_cookie_header(secure: bool) -> String {
    let mut cookie =
        format!("{ADMIN_REFRESH_COOKIE}=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0");
    if secure {
        cookie.push_str("; Secure");
    }
    cookie
}

pub use client::AuthorizedBrowserClient;
pub use components::{GuestRoute, ProtectedRoute, RequireAuth};
pub use context::{AuthContext, AuthProvider};
pub use hooks::{
    use_auth, use_auth_error, use_current_user, use_is_authenticated, use_is_loading,
    use_is_token_valid, use_session, use_tenant, use_token,
};
pub use storage::{ServerAuthSnapshot, provide_server_auth_snapshot};

/// Extract the stable session id (`session_id` claim) from an access
/// token WITHOUT verifying it. The value identifies the refresh
/// lineage and therefore survives token rotation, which makes it the
/// right CSRF-derivation secret for same-origin browser adapters:
/// unguessable (122-bit UUID), embedded by the server into pages it
/// renders, and stable for the whole session lifetime. Callers must
/// still authenticate the token itself through the API — this is a
/// parsing helper, not validation.
pub fn session_id_from_access_token(token: &str) -> Option<String> {
    let payload = token.split('.').nth(1)?;
    let decoded = base64url_decode(payload)?;
    let json = String::from_utf8(decoded).ok()?;
    let marker = "\"session_id\":\"";
    let start = json.find(marker)? + marker.len();
    let end = start + json[start..].find('"')?;
    let value = &json[start..end];
    (!value.is_empty()).then(|| value.to_string())
}

fn base64url_decode(input: &str) -> Option<Vec<u8>> {
    fn value_of(byte: u8) -> Option<u8> {
        match byte {
            b'A'..=b'Z' => Some(byte - b'A'),
            b'a'..=b'z' => Some(byte - b'a' + 26),
            b'0'..=b'9' => Some(byte - b'0' + 52),
            b'-' => Some(62),
            b'_' => Some(63),
            _ => None,
        }
    }

    let mut output = Vec::with_capacity(input.len() * 3 / 4);
    let mut buffer: u32 = 0;
    let mut bits: u32 = 0;
    for byte in input.bytes() {
        if byte == b'=' {
            break;
        }
        buffer = (buffer << 6) | u32::from(value_of(byte)?);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            output.push((buffer >> bits) as u8);
        }
    }
    Some(output)
}

