use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize)]
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

#[derive(Clone, Debug, Serialize)]
pub struct CreateUserInput {
    pub email: String,
    pub password: String,
    pub name: Option<String>,
    pub role: Option<String>,
    pub status: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct UpdateUserInput {
    pub name: Option<String>,
    pub role: String,
    pub status: String,
}
