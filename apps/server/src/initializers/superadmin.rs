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

use crate::error::Result;
use crate::models::{tenants, users};
use crate::services::auth_lifecycle::AuthLifecycleService;
use crate::services::rbac_service::RbacService;
use crate::services::server_runtime_context::ServerRuntimeContext;

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

pub async fn ensure_default_superadmin(ctx: &ServerRuntimeContext) -> Result<()> {
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
        tenants::Entity::find_or_create(ctx.db(), &tenant_name, &tenant_slug, None).await?;

    if let Some(user) = users::Entity::find_by_email(ctx.db(), tenant.id, &email).await? {
        RbacService::replace_user_role_committed(
            ctx.db(),
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

    let user = AuthLifecycleService::create_user_runtime(
        ctx,
        tenant.id,
        &email,
        &password,
        Some("Super Admin".to_string()),
        rustok_core::UserRole::SuperAdmin,
        Some(rustok_core::UserStatus::Active),
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
