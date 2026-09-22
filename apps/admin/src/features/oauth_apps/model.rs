use crate::entities::oauth_app::model::{AppType, OAuthApp};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize)]
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
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateOAuthAppResult {
    pub app: OAuthApp,
    pub client_secret: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateOAuthAppInput {
    pub name: String,
    pub description: Option<String>,
    pub icon_url: Option<String>,
    pub redirect_uris: Vec<String>,
    pub scopes: Vec<String>,
    pub grant_types: Vec<String>,
}
