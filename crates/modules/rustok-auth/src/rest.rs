use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Deserialize, ToSchema)]
pub struct LoginParams {
    pub email: String,
    pub password: String,
}

#[derive(Deserialize, ToSchema)]
pub struct RefreshRequest {
    pub refresh_token: String,
}

#[derive(Deserialize, ToSchema)]
pub struct RegisterParams {
    pub email: String,
    pub password: String,
    pub name: Option<String>,
}

#[derive(Deserialize, ToSchema)]
pub struct AcceptInviteParams {
    pub token: String,
    pub password: String,
    pub name: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct InviteAcceptResponse {
    pub status: &'static str,
    pub email: String,
    pub role: rustok_core::UserRole,
}

#[derive(Deserialize, ToSchema)]
pub struct RequestResetParams {
    pub email: String,
}

#[derive(Deserialize, ToSchema)]
pub struct ConfirmResetParams {
    pub token: String,
    pub password: String,
}

#[derive(Deserialize, ToSchema)]
pub struct RequestVerificationParams {
    pub email: String,
}

#[derive(Deserialize, ToSchema)]
pub struct ConfirmVerificationParams {
    pub token: String,
}

#[derive(Deserialize, ToSchema)]
pub struct ChangePasswordParams {
    pub current_password: String,
    pub new_password: String,
}

#[derive(Deserialize, ToSchema)]
pub struct UpdateProfileParams {
    pub name: Option<String>,
}

/// Password-reset request acknowledgement. Deliberately carries no
/// token in any mode: reset links are delivered only through the
/// mailbox, so the response cannot be used to take over an account.
#[derive(Debug, Serialize, ToSchema)]
pub struct ResetRequestResponse {
    pub status: &'static str,
}

/// Email-verification request acknowledgement; same no-token contract
/// as [`ResetRequestResponse`].
#[derive(Debug, Serialize, ToSchema)]
pub struct VerificationRequestResponse {
    pub status: &'static str,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct GenericStatusResponse {
    pub status: &'static str,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SessionItem {
    pub id: Uuid,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub current: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SessionsResponse {
    pub sessions: Vec<SessionItem>,
}

#[derive(Debug, Deserialize, ToSchema, utoipa::IntoParams)]
pub struct SessionListParams {
    pub limit: Option<u64>,
}

#[derive(Serialize, ToSchema)]
pub struct UserResponse {
    pub id: Uuid,
    pub email: String,
    pub name: Option<String>,
    pub role: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct UserItem {
    pub id: Uuid,
    pub email: String,
    pub name: Option<String>,
    pub status: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct UsersResponse {
    pub users: Vec<UserItem>,
    pub total: u64,
    pub page: u64,
    pub page_size: u64,
}

#[derive(Debug, Deserialize, ToSchema, utoipa::IntoParams)]
pub struct UsersListParams {
    pub page: Option<u64>,
    pub page_size: Option<u64>,
    pub search: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct UserInfo {
    pub id: Uuid,
    pub email: String,
    pub name: Option<String>,
    pub role: rustok_core::UserRole,
    pub status: rustok_core::UserStatus,
}

#[derive(Serialize, ToSchema)]
pub struct AuthResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: &'static str,
    pub expires_in: u64,
    pub user: UserInfo,
}

impl std::fmt::Debug for AuthResponse {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AuthResponse")
            .field("access_token", &"<redacted>")
            .field("refresh_token", &"<redacted>")
            .field("token_type", &self.token_type)
            .field("expires_in", &self.expires_in)
            .field("user", &self.user)
            .finish()
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct LogoutResponse {
    pub status: &'static str,
}

/// OAuth2 Token Request (application/json or application/x-www-form-urlencoded).
#[derive(Deserialize, ToSchema)]
pub struct TokenRequest {
    pub grant_type: String,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
    pub scope: Option<String>,
    pub code: Option<String>,
    pub redirect_uri: Option<String>,
    pub code_verifier: Option<String>,
    pub refresh_token: Option<String>,
}

impl std::fmt::Debug for TokenRequest {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("TokenRequest")
            .field("grant_type", &self.grant_type)
            .field("client_id", &self.client_id)
            .field("client_secret", &self.client_secret.as_ref().map(|_| "<redacted>"))
            .field("scope", &self.scope)
            .field("code", &self.code.as_ref().map(|_| "<redacted>"))
            .field("redirect_uri", &self.redirect_uri)
            .field("code_verifier", &self.code_verifier.as_ref().map(|_| "<redacted>"))
            .field("refresh_token", &self.refresh_token.as_ref().map(|_| "<redacted>"))
            .finish()
    }
}

/// OAuth2 Authorization Request.
#[derive(Debug, Deserialize, ToSchema)]
pub struct AuthorizeRequest {
    pub response_type: String,
    pub client_id: String,
    pub redirect_uri: String,
    pub scope: Option<String>,
    pub state: Option<String>,
    pub code_challenge: String,
    pub code_challenge_method: Option<String>,
}

/// Browser OAuth2 Authorization Request.
#[derive(Debug, Deserialize, Clone, ToSchema)]
pub struct BrowserAuthorizeRequest {
    pub response_type: String,
    pub client_id: String,
    pub redirect_uri: String,
    pub scope: Option<String>,
    pub state: Option<String>,
    pub code_challenge: String,
    pub code_challenge_method: Option<String>,
}

/// Server-hosted consent form submission.
#[derive(Debug, Deserialize, ToSchema)]
pub struct ConsentRequest {
    pub action: String,
    pub client_id: String,
    pub redirect_uri: String,
    pub scope: Option<String>,
    pub state: Option<String>,
    pub code_challenge: String,
    pub code_challenge_method: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct BrowserSessionResponse {
    pub status: &'static str,
}

/// OAuth2 Token Response (RFC 6749 section 5.1).
#[derive(Serialize, ToSchema)]
pub struct TokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
    pub scope: String,
}

impl std::fmt::Debug for TokenResponse {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("TokenResponse")
            .field("access_token", &"<redacted>")
            .field("token_type", &self.token_type)
            .field("expires_in", &self.expires_in)
            .field("refresh_token", &self.refresh_token.as_ref().map(|_| "<redacted>"))
            .field("scope", &self.scope)
            .finish()
    }
}

/// OAuth2 Error Response (RFC 6749 section 5.2).
#[derive(Debug, Serialize, ToSchema)]
pub struct TokenErrorResponse {
    pub error: String,
    pub error_description: String,
}

/// OAuth2 Token Revocation Request (RFC 7009).
#[derive(Deserialize, ToSchema)]
pub struct RevokeRequest {
    pub token: String,
    pub token_type_hint: Option<String>,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
}

impl std::fmt::Debug for RevokeRequest {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RevokeRequest")
            .field("token", &"<redacted>")
            .field("token_type_hint", &self.token_type_hint)
            .field("client_id", &self.client_id)
            .field("client_secret", &self.client_secret.as_ref().map(|_| "<redacted>"))
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reset_and_verification_responses_never_carry_tokens() {
        let reset = serde_json::to_value(ResetRequestResponse { status: "ok" })
            .expect("serialize reset response");
        assert_eq!(reset, serde_json::json!({ "status": "ok" }));

        let verification =
            serde_json::to_value(VerificationRequestResponse { status: "ok" })
                .expect("serialize verification response");
        assert_eq!(verification, serde_json::json!({ "status": "ok" }));
    }

    #[test]
    fn auth_response_debug_redacts_tokens() {
        let auth = AuthResponse {
            access_token: "super-secret-access-token".to_string(),
            refresh_token: "super-secret-refresh-token".to_string(),
            token_type: "Bearer",
            expires_in: 3600,
            user: UserInfo {
                id: Uuid::new_v4(),
                email: "admin@example.com".to_string(),
                name: Some("Admin".to_string()),
                role: rustok_core::UserRole::Admin,
                status: rustok_core::UserStatus::Active,
            },
        };
        let auth_dbg = format!("{auth:?}");
        assert!(!auth_dbg.contains("super-secret-access-token"));
        assert!(!auth_dbg.contains("super-secret-refresh-token"));
        assert!(auth_dbg.contains("<redacted>"));
        assert!(auth_dbg.contains("admin@example.com"));
    }

    #[test]
    fn token_request_debug_redacts_secrets() {
        let req = TokenRequest {
            grant_type: "authorization_code".to_string(),
            client_id: Some("app_client_id".to_string()),
            client_secret: Some("confidential_client_secret".to_string()),
            scope: Some("openid".to_string()),
            code: Some("one_time_authorization_code".to_string()),
            redirect_uri: Some("https://app.example.com/callback".to_string()),
            code_verifier: Some("pkce_verifier_secret".to_string()),
            refresh_token: Some("confidential_refresh_token".to_string()),
        };
        let req_dbg = format!("{req:?}");
        assert!(!req_dbg.contains("confidential_client_secret"));
        assert!(!req_dbg.contains("one_time_authorization_code"));
        assert!(!req_dbg.contains("pkce_verifier_secret"));
        assert!(!req_dbg.contains("confidential_refresh_token"));
        assert!(req_dbg.contains("app_client_id"));
        assert!(req_dbg.contains("<redacted>"));
    }

    #[test]
    fn token_response_debug_redacts_tokens() {
        let resp = TokenResponse {
            access_token: "oauth-access-secret".to_string(),
            token_type: "Bearer".to_string(),
            expires_in: 3600,
            refresh_token: Some("oauth-refresh-secret".to_string()),
            scope: "openid".to_string(),
        };
        let resp_dbg = format!("{resp:?}");
        assert!(!resp_dbg.contains("oauth-access-secret"));
        assert!(!resp_dbg.contains("oauth-refresh-secret"));
        assert!(resp_dbg.contains("<redacted>"));
        assert!(resp_dbg.contains("openid"));
    }

    #[test]
    fn revoke_request_debug_redacts_tokens_and_secrets() {
        let revoke = RevokeRequest {
            token: "revocable-token-secret".to_string(),
            token_type_hint: Some("refresh_token".to_string()),
            client_id: Some("client123".to_string()),
            client_secret: Some("secret123".to_string()),
        };
        let revoke_dbg = format!("{revoke:?}");
        assert!(!revoke_dbg.contains("revocable-token-secret"));
        assert!(!revoke_dbg.contains("secret123"));
        assert!(revoke_dbg.contains("<redacted>"));
        assert!(revoke_dbg.contains("client123"));
    }
}
