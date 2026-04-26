//! Default SuperAdmin Initializer
//!
//! Automatically ensures a default SuperAdmin user exists on every startup.
//! Runs before the server accepts requests, so it's safe for all environments.
//!
//! ## Configuration (env vars, in priority order)
//!
//! | Variable                | Fallback             | Default           |
//! |-------------------------|----------------------|-------------------|
//! | `SUPERADMIN_EMAIL`      | `SEED_ADMIN_EMAIL`   | *(required)*      |
//! | `SUPERADMIN_PASSWORD`   | `SEED_ADMIN_PASSWORD`| *(required)*      |
//! | `SUPERADMIN_TENANT_SLUG`| `SEED_TENANT_SLUG`   | `"default"`       |
//! | `SUPERADMIN_TENANT_NAME`| `SEED_TENANT_NAME`   | `"Default"`       |
//!
//! If neither primary nor fallback env var is set for email/password,
//! the initializer skips silently (no superadmin will be created).

use async_trait::async_trait;
use loco_rs::{
    app::{AppContext, Initializer},
    Result,
};
use sea_orm::ActiveModelTrait;
use sea_orm::ActiveValue::Set;

use crate::auth::hash_password;
use crate::models::{tenants, users};
use crate::services::rbac_service::RbacService;

pub struct SuperAdminInitializer;

fn env_first(primary: &str, fallback: &str) -> Option<String> {
    std::env::var(primary)
        .ok()
        .filter(|v| !v.trim().is_empty())
        .or_else(|| {
            std::env::var(fallback)
                .ok()
                .filter(|v| !v.trim().is_empty())
        })
}

#[async_trait]
impl Initializer for SuperAdminInitializer {
    fn name(&self) -> String {
        "superadmin".to_string()
    }

    async fn before_run(&self, ctx: &AppContext) -> Result<()> {
        let Some(email) = env_first("SUPERADMIN_EMAIL", "SEED_ADMIN_EMAIL") else {
            tracing::debug!("SUPERADMIN_EMAIL not set — skipping default superadmin setup");
            return Ok(());
        };

        let Some(password) = env_first("SUPERADMIN_PASSWORD", "SEED_ADMIN_PASSWORD") else {
            tracing::warn!("SUPERADMIN_EMAIL is set but SUPERADMIN_PASSWORD is missing — skipping");
            return Ok(());
        };

        let tenant_slug = env_first("SUPERADMIN_TENANT_SLUG", "SEED_TENANT_SLUG")
            .unwrap_or_else(|| "default".to_string());

        let tenant_name = env_first("SUPERADMIN_TENANT_NAME", "SEED_TENANT_NAME")
            .unwrap_or_else(|| "Default".to_string());

        let tenant =
            tenants::Entity::find_or_create(&ctx.db, &tenant_name, &tenant_slug, None).await?;

        if let Some(user) = users::Entity::find_by_email(&ctx.db, tenant.id, &email).await? {
            RbacService::assign_role_permissions(
                &ctx.db,
                &user.id,
                &tenant.id,
                rustok_core::UserRole::SuperAdmin,
            )
            .await?;

            tracing::debug!(
                email = %email,
                tenant = %tenant_slug,
                "Default superadmin already exists - synchronized role permissions"
            );
            return Ok(());
        }

        let password_hash = hash_password(&password)?;
        let mut user = users::ActiveModel::new(tenant.id, &email, &password_hash);
        user.name = Set(Some("Super Admin".to_string()));
        let user = user.insert(&ctx.db).await?;

        RbacService::assign_role_permissions(
            &ctx.db,
            &user.id,
            &tenant.id,
            rustok_core::UserRole::SuperAdmin,
        )
        .await?;

        tracing::info!(
            email = %email,
            tenant = %tenant_slug,
            user_id = %user.id,
            "Default superadmin created"
        );

        Ok(())
    }
}
