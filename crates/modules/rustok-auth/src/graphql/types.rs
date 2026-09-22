use async_graphql::{Enum, InputObject, Object, SimpleObject};
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::{AuthorizedOAuthAppRecord, OAuthAppMutationRecord};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Enum)]
pub enum AppType {
    Embedded,
    FirstParty,
    Mobile,
    Service,
    ThirdParty,
}

impl AppType {
    pub fn from_value(value: &str) -> Self {
        match value {
            "embedded" => Self::Embedded,
            "first_party" => Self::FirstParty,
            "mobile" => Self::Mobile,
            "service" => Self::Service,
            _ => Self::ThirdParty,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Embedded => "embedded",
            Self::FirstParty => "first_party",
            Self::Mobile => "mobile",
            Self::Service => "service",
            Self::ThirdParty => "third_party",
        }
    }
}

pub struct OAuthAppGql(pub OAuthAppMutationRecord);

#[Object]
impl OAuthAppGql {
    async fn id(&self) -> Uuid {
        self.0.id
    }
    async fn name(&self) -> &str {
        &self.0.name
    }
    async fn slug(&self) -> &str {
        &self.0.slug
    }
    async fn description(&self) -> Option<&str> {
        self.0.description.as_deref()
    }
    async fn icon_url(&self) -> Option<&str> {
        self.0.icon_url.as_deref()
    }
    async fn app_type(&self) -> AppType {
        AppType::from_value(&self.0.app_type)
    }
    async fn client_id(&self) -> Uuid {
        self.0.client_id
    }
    async fn redirect_uris(&self) -> Vec<String> {
        self.0.redirect_uris.clone()
    }
    async fn scopes(&self) -> Vec<String> {
        self.0.scopes.clone()
    }
    async fn grant_types(&self) -> Vec<String> {
        self.0.grant_types.clone()
    }
    async fn granted_permissions(&self) -> Vec<String> {
        self.0.granted_permissions.clone()
    }
    async fn manifest_ref(&self) -> Option<&str> {
        self.0.manifest_ref.as_deref()
    }
    async fn auto_created(&self) -> bool {
        self.0.auto_created
    }
    async fn managed_by_manifest(&self) -> bool {
        self.0.managed_by_manifest
    }
    async fn is_active(&self) -> bool {
        self.0.is_active
    }
    async fn can_edit(&self) -> bool {
        self.0.can_edit
    }
    async fn can_rotate_secret(&self) -> bool {
        self.0.can_rotate_secret
    }
    async fn can_revoke(&self) -> bool {
        self.0.can_revoke
    }
    async fn active_token_count(&self) -> u64 {
        self.0.active_token_count.max(0) as u64
    }
    async fn last_used_at(&self) -> Option<DateTime<Utc>> {
        self.0.last_used_at
    }
    async fn created_at(&self) -> DateTime<Utc> {
        self.0.created_at
    }
}

#[derive(Debug, InputObject)]
pub struct CreateOAuthAppInput {
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    pub app_type: AppType,
    pub icon_url: Option<String>,
    pub redirect_uris: Option<Vec<String>>,
    pub scopes: Vec<String>,
    pub grant_types: Vec<String>,
    pub granted_permissions: Vec<String>,
}

#[derive(Debug, InputObject)]
pub struct UpdateOAuthAppInput {
    pub name: String,
    pub description: Option<String>,
    pub icon_url: Option<String>,
    pub redirect_uris: Vec<String>,
    pub scopes: Vec<String>,
    pub grant_types: Vec<String>,
    pub granted_permissions: Vec<String>,
}

#[derive(SimpleObject)]
pub struct CreateOAuthAppResultGql {
    pub app: OAuthAppGql,
    pub client_secret: String,
}

#[derive(SimpleObject)]
pub struct RotateSecretResultGql {
    pub app: OAuthAppGql,
    pub client_secret: String,
}

pub struct AuthorizedAppGql(pub AuthorizedOAuthAppRecord);

#[Object]
impl AuthorizedAppGql {
    async fn app(&self) -> OAuthAppGql {
        OAuthAppGql(self.0.app.clone())
    }
    async fn scopes(&self) -> Vec<String> {
        self.0.scopes.clone()
    }
    async fn granted_at(&self) -> DateTime<Utc> {
        self.0.granted_at
    }
}
