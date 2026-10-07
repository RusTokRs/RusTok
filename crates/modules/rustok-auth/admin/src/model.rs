use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub id: String,
    pub email: String,
    pub name: Option<String>,
    pub role: UserRole,
    pub status: UserStatus,
    pub created_at: Option<String>,
    pub tenant_name: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum UserRole {
    SuperAdmin,
    Admin,
    Manager,
    Customer,
    #[serde(other)]
    Unknown,
}

impl std::fmt::Display for UserRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UserRole::SuperAdmin => write!(f, "Super Admin"),
            UserRole::Admin => write!(f, "Admin"),
            UserRole::Manager => write!(f, "Manager"),
            UserRole::Customer => write!(f, "Customer"),
            UserRole::Unknown => write!(f, "Unknown"),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum UserStatus {
    Active,
    Inactive,
    Suspended,
    #[serde(other)]
    Unknown,
}

impl std::fmt::Display for UserStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UserStatus::Active => write!(f, "Active"),
            UserStatus::Inactive => write!(f, "Inactive"),
            UserStatus::Suspended => write!(f, "Suspended"),
            UserStatus::Unknown => write!(f, "Unknown"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AppType {
    Embedded,
    FirstParty,
    Mobile,
    Service,
    ThirdParty,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateOAuthAppInput {
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    pub icon_url: Option<String>,
    pub app_type: AppType,
    pub redirect_uris: Option<Vec<String>>,
    pub scopes: Vec<String>,
    pub grant_types: Vec<String>,
    #[serde(default)]
    pub granted_permissions: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateOAuthAppInput {
    pub name: String,
    pub description: Option<String>,
    pub icon_url: Option<String>,
    pub redirect_uris: Vec<String>,
    pub scopes: Vec<String>,
    pub grant_types: Vec<String>,
    #[serde(default)]
    pub granted_permissions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OAuthApp {
    pub id: Uuid,
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    pub icon_url: Option<String>,
    pub app_type: AppType,
    pub client_id: Uuid,
    pub redirect_uris: Vec<String>,
    pub scopes: Vec<String>,
    pub grant_types: Vec<String>,
    pub manifest_ref: Option<String>,
    pub auto_created: bool,
    pub managed_by_manifest: bool,
    pub is_active: bool,
    pub can_edit: bool,
    pub can_rotate_secret: bool,
    pub can_revoke: bool,
    pub active_token_count: i64,
    pub last_used_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphqlUser {
    pub id: String,
    pub email: String,
    pub name: Option<String>,
    pub role: String,
    pub status: String,
    pub created_at: String,
    pub tenant_name: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GraphqlUserResponse {
    pub user: Option<GraphqlUser>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GraphqlUsersResponse {
    pub users: GraphqlUsersConnection,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GraphqlUsersConnection {
    pub edges: Vec<GraphqlUserEdge>,
    #[serde(rename = "pageInfo")]
    pub page_info: GraphqlPageInfo,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GraphqlUserEdge {
    pub cursor: String,
    pub node: GraphqlUser,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GraphqlPageInfo {
    #[serde(rename = "totalCount")]
    pub total_count: i64,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateUserInput {
    pub email: String,
    pub password: String,
    pub name: Option<String>,
    pub role: Option<String>,
    pub status: Option<String>,
}

impl std::fmt::Debug for CreateUserInput {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CreateUserInput")
            .field("email", &self.email)
            .field("password", &"<redacted>")
            .field("name", &self.name)
            .field("role", &self.role)
            .field("status", &self.status)
            .finish()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateUserInput {
    pub name: Option<String>,
    pub role: String,
    pub status: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthSessionItem {
    pub id: String,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub last_used_at: Option<String>,
    pub expires_at: String,
    pub created_at: String,
    pub current: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionsPayload {
    pub sessions: Vec<AuthSessionItem>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionsQueryData {
    pub sessions: SessionsPayload,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RevokeSessionResponse {
    #[serde(rename = "revokeSession")]
    pub revoke_session: RevokeSessionPayload,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RevokeSessionPayload {
    pub success: bool,
    pub revoked: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RevokeAllSessionsResponse {
    #[serde(rename = "revokeAllSessions")]
    pub revoke_all_sessions: RevokeAllSessionsPayload,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RevokeAllSessionsPayload {
    pub success: bool,
    #[serde(rename = "revokedCount")]
    pub revoked_count: i32,
}

