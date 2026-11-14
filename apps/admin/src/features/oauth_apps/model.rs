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

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateOAuthAppResult {
    pub app: OAuthApp,
    pub client_secret: String,
}

impl std::fmt::Debug for CreateOAuthAppResult {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CreateOAuthAppResult")
            .field("app", &self.app)
            .field("client_secret", &"<redacted>")
            .finish()
    }
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

#[cfg(test)]
mod tests {
    use super::CreateOAuthAppResult;
    use crate::entities::oauth_app::model::{AppType, OAuthApp};
    use uuid::Uuid;

    #[test]
    fn oauth_admin_result_debug_redacts_client_secret() {
        let secret = "one-time-client-secret";
        let result = CreateOAuthAppResult {
            app: OAuthApp {
                id: Uuid::nil(),
                name: "Example app".to_string(),
                slug: "example-app".to_string(),
                description: None,
                icon_url: None,
                app_type: AppType::Service,
                client_id: Uuid::nil(),
                redirect_uris: Vec::new(),
                scopes: Vec::new(),
                grant_types: Vec::new(),
                manifest_ref: None,
                auto_created: false,
                managed_by_manifest: false,
                is_active: true,
                can_edit: true,
                can_rotate_secret: true,
                can_revoke: true,
                active_token_count: 0,
                last_used_at: None,
                created_at: chrono::Utc::now(),
            },
            client_secret: secret.to_string(),
        };

        let debug = format!("{result:?}");
        assert!(debug.contains("<redacted>"));
        assert!(!debug.contains(secret));
    }
}
