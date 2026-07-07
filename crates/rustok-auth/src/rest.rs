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

#[derive(Debug, Serialize, ToSchema)]
pub struct ResetRequestResponse {
    pub status: &'static str,
    pub reset_token: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct VerificationRequestResponse {
    pub status: &'static str,
    pub verification_token: Option<String>,
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

#[derive(Debug, Serialize, ToSchema)]
pub struct AuthResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: &'static str,
    pub expires_in: u64,
    pub user: UserInfo,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct LogoutResponse {
    pub status: &'static str,
}

/// OAuth2 Token Request (application/json or application/x-www-form-urlencoded).
#[derive(Debug, Deserialize, ToSchema)]
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
#[derive(Debug, Serialize, ToSchema)]
pub struct TokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
    pub scope: String,
}

/// OAuth2 Error Response (RFC 6749 section 5.2).
#[derive(Debug, Serialize, ToSchema)]
pub struct TokenErrorResponse {
    pub error: String,
    pub error_description: String,
}

/// OAuth2 Token Revocation Request (RFC 7009).
#[derive(Debug, Deserialize, ToSchema)]
pub struct RevokeRequest {
    pub token: String,
    pub token_type_hint: Option<String>,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
}
