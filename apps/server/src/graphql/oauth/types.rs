//! GraphQL types for OAuth App management

use async_graphql::{Context, Enum, InputObject, Object, SimpleObject};
use chrono::{DateTime, Utc};
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::models::oauth_apps;
use crate::models::oauth_tokens;

/// OAuth App Type enum for GraphQL
#[derive(Debug, Clone, Copy, PartialEq, Eq, Enum)]
pub enum AppType {
    /// Embedded in the binary (Leptos). No OAuth2 needed.
    Embedded,
    /// First-party app: our admin/storefront in a separate process.
    FirstParty,
    /// Mobile application.
    Mobile,
    /// Machine-to-machine: bots, integrations, CI/CD.
    Service,
    /// Third-party developers: restricted access.
    ThirdParty,
}

impl AppType {
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Self {
        match s {
            "embedded" => Self::Embedded,
            "first_party" => Self::FirstParty,
            "mobile" => Self::Mobile,
            "service" => Self::Service,
            "third_party" => Self::ThirdParty,
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

/// GraphQL representation of an OAuth app
pub struct OAuthAppGql(pub oauth_apps::Model);

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
        AppType::from_str(&self.0.app_type)
    }

    async fn client_id(&self) -> Uuid {
        self.0.client_id
    }

    async fn redirect_uris(&self) -> Vec<String> {
        self.0.redirect_uris_list()
    }

    async fn scopes(&self) -> Vec<String> {
        self.0.scopes_list()
    }

    async fn grant_types(&self) -> Vec<String> {
        self.0.grant_types_list()
    }

    async fn granted_permissions(&self) -> Vec<String> {
        self.0.granted_permissions_list()
    }

    async fn manifest_ref(&self) -> Option<&str> {
        self.0.manifest_ref.as_deref()
    }

    async fn auto_created(&self) -> bool {
        self.0.auto_created
    }

    async fn managed_by_manifest(&self) -> bool {
        self.0.managed_by_manifest()
    }

    async fn is_active(&self) -> bool {
        self.0.is_active()
    }

    async fn can_edit(&self) -> bool {
        self.0.can_edit()
    }

    async fn can_rotate_secret(&self) -> bool {
        self.0.can_rotate_secret()
    }

    async fn can_revoke(&self) -> bool {
        self.0.can_revoke()
    }

    async fn active_token_count(&self, ctx: &Context<'_>) -> async_graphql::Result<u64> {
        let db = ctx.data::<DatabaseConnection>()?;
        oauth_tokens::Entity::count_active_by_app(db, self.0.id)
            .await
            .map_err(|e| async_graphql::Error::new(format!("Failed to count active tokens: {e}")))
    }

    async fn last_used_at(&self) -> Option<DateTime<Utc>> {
        self.0.last_used_at.map(|dt| dt.into())
    }

    async fn created_at(&self) -> DateTime<Utc> {
        self.0.created_at.into()
    }
}

/// Input for creating a new OAuth app
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

/// Input for updating a manual OAuth app
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

/// Result of creating an OAuth app — includes client_secret shown ONCE
#[derive(SimpleObject)]
pub struct CreateOAuthAppResultGql {
    pub app: OAuthAppGql,
    /// client_secret is shown ONCE at creation time. Store it safely!
    pub client_secret: String,
}

/// Result of rotating an OAuth app's secret
#[derive(SimpleObject)]
pub struct RotateSecretResultGql {
    pub app: OAuthAppGql,
    /// New client_secret, shown ONCE. Store it safely!
    pub client_secret: String,
}

/// Represents an application that the user has granted access to
pub struct AuthorizedAppGql {
    pub app: oauth_apps::Model,
    pub scopes: Vec<String>,
    pub granted_at: DateTime<Utc>,
}

#[Object]
impl AuthorizedAppGql {
    async fn app(&self) -> OAuthAppGql {
        OAuthAppGql(self.app.clone())
    }

    async fn scopes(&self) -> Vec<String> {
        self.scopes.clone()
    }

    async fn granted_at(&self) -> DateTime<Utc> {
        self.granted_at
    }
}
