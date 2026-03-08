use async_trait::async_trait;
use loco_rs::app::AppContext;
use loco_rs::task::{Task, Vars};
use std::collections::BTreeMap;
use tracing::info;

use crate::services::oauth_app::{CreateOAuthAppInput, OAuthAppService};

pub struct CreateOAuthAppTask;

#[async_trait]
impl Task for CreateOAuthAppTask {
    fn task(&self) -> loco_rs::task::TaskInfo {
        loco_rs::task::TaskInfo {
            name: "create_oauth_app".to_string(),
            detail: "Create a new OAuth application (e.g., for local development)".to_string(),
        }
    }

    async fn run(&self, app_context: &AppContext, vars: &Vars) -> loco_rs::Result<()> {
        let name = vars
            .cli_arg("name")
            .unwrap_or_else(|_| "Development App".to_string());
        let slug = vars
            .cli_arg("slug")
            .unwrap_or_else(|_| "dev-app".to_string());

        info!("Creating OAuth app: {} (slug: {})", name, slug);

        // Fetch a default tenant (we'll just use the first available one for DX purposes)
        let db = &app_context.db;
        use crate::models::tenants::Entity as Tenants;
        use sea_orm::EntityTrait;

        let tenant = Tenants::find().one(db).await?.ok_or_else(|| {
            loco_rs::Error::Message("No tenant found. Run seeds first.".to_string())
        })?;

        let input = CreateOAuthAppInput {
            name,
            slug,
            description: Some("Created via CLI task".to_string()),
            app_type: "third_party".to_string(),
            redirect_uris: vec![
                "http://localhost:3000/api/auth/callback".to_string(),
                "http://localhost:1420".to_string(),
            ],
            scopes: vec![
                "openid".to_string(),
                "profile".to_string(),
                "email".to_string(),
                "offline_access".to_string(),
            ],
            grant_types: vec![
                "authorization_code".to_string(),
                "refresh_token".to_string(),
            ],
        };

        let result = OAuthAppService::create_app(db, tenant.id, input)
            .await
            .map_err(|e| loco_rs::Error::Message(format!("Failed to create app: {}", e)))?;

        println!("==================================================");
        println!("✅ OAuth Application created successfully!");
        println!("Name:           {}", result.app.name);
        println!("Type:           {}", result.app.app_type);
        println!("Client ID:      {}", result.app.client_id);
        println!("Client Secret:  {}", result.client_secret);
        println!("==================================================");
        println!("IMPORTANT: Store the Client Secret securely. It will not be shown again.");

        Ok(())
    }
}
