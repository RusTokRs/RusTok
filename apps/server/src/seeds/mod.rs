//! # RusToK Database Seeds
//!
//! Seed data for development and testing.
//! The Loco hook is an adapter in `app`; seed execution itself accepts the neutral
//! server runtime. A later CLI provider must depend on an owner-owned seed
//! service rather than on this server package.

use anyhow::Result;
use sea_orm::{ActiveModelTrait, ActiveValue::Set};
use std::path::Path;

use crate::auth::hash_password;
use crate::models::{tenants, users};
use crate::services::rbac_service::RbacService;
use crate::services::server_runtime_context::ServerRuntimeContext;

const DEFAULT_DEV_SEED_PASSWORD: &str = "dev-password-123";

fn superadmin_email() -> Option<String> {
    for key in ["SUPERADMIN_EMAIL", "SEED_ADMIN_EMAIL"] {
        if let Ok(v) = std::env::var(key) {
            let v = v.trim().to_string();
            if !v.is_empty() {
                return Some(v);
            }
        }
    }
    None
}

fn superadmin_password() -> String {
    for key in [
        "SUPERADMIN_PASSWORD",
        "SEED_ADMIN_PASSWORD",
        "RUSTOK_DEV_SEED_PASSWORD",
    ] {
        if let Ok(v) = std::env::var(key) {
            let v = v.trim().to_string();
            if !v.is_empty() {
                return v;
            }
        }
    }
    DEFAULT_DEV_SEED_PASSWORD.to_string()
}

fn superadmin_tenant_slug() -> String {
    for key in ["SUPERADMIN_TENANT_SLUG", "SEED_TENANT_SLUG"] {
        if let Ok(v) = std::env::var(key) {
            let v = v.trim().to_string();
            if !v.is_empty() {
                return v;
            }
        }
    }
    "demo".to_string()
}

fn superadmin_tenant_name() -> String {
    for key in ["SUPERADMIN_TENANT_NAME", "SEED_TENANT_NAME"] {
        if let Ok(v) = std::env::var(key) {
            let v = v.trim().to_string();
            if !v.is_empty() {
                return v;
            }
        }
    }
    "Demo Workspace".to_string()
}

/// Seed the database with initial data
pub async fn seed(runtime: &ServerRuntimeContext, path: &Path) -> Result<()> {
    let seed_name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("default");

    tracing::info!(seed = %seed_name, "Running database seed...");

    match seed_name {
        "default" | "dev" => seed_development(runtime).await?,
        "test" => seed_test().await?,
        "minimal" => seed_minimal(runtime).await?,
        _ => {
            tracing::warn!(seed = %seed_name, "Unknown seed file, using default");
            seed_development(runtime).await?;
        }
    }

    tracing::info!("Database seed complete");
    Ok(())
}

/// Development seed data
async fn seed_development(runtime: &ServerRuntimeContext) -> Result<()> {
    tracing::info!("Seeding development data...");

    let tenant_slug = superadmin_tenant_slug();
    let tenant_name = superadmin_tenant_name();

    let demo_tenant = tenants::Entity::find_or_create(
        runtime.db(),
        &tenant_name,
        &tenant_slug,
        Some("demo.localhost"),
    )
    .await?;

    let admin_email = superadmin_email().unwrap_or_else(|| "admin@demo.local".to_string());

    seed_user(
        runtime,
        demo_tenant.id,
        &admin_email,
        "Super Admin",
        rustok_core::UserRole::SuperAdmin,
    )
    .await?;

    seed_user(
        runtime,
        demo_tenant.id,
        "customer@demo.local",
        "Demo Customer",
        rustok_core::UserRole::Customer,
    )
    .await?;

    let registry = crate::modules::build_registry();
    for module in ["content", "commerce", "pages", "blog", "forum", "index"] {
        crate::services::module_lifecycle::ModuleLifecycleService::toggle_module_with_actor(
            runtime.db(),
            &registry,
            demo_tenant.id,
            module,
            true,
            Some("seed".to_string()),
        )
        .await?;
    }

    tracing::info!(tenant_id = %demo_tenant.id, "Development seed data ensured");

    Ok(())
}

async fn seed_user(
    runtime: &ServerRuntimeContext,
    tenant_id: uuid::Uuid,
    email: &str,
    name: &str,
    role: rustok_core::UserRole,
) -> Result<()> {
    if users::Entity::find_by_email(runtime.db(), tenant_id, email)
        .await?
        .is_some()
    {
        return Ok(());
    }

    let seed_password = superadmin_password();
    let password_hash = hash_password(&seed_password)?;
    let mut user = users::ActiveModel::new(tenant_id, email, &password_hash);
    user.name = Set(Some(name.to_string()));
    let user = user.insert(runtime.db()).await?;

    RbacService::assign_role_permissions(runtime.db(), &user.id, &tenant_id, role).await?;

    Ok(())
}

/// Test seed data
async fn seed_test() -> Result<()> {
    tracing::info!("Seeding test data...");

    // Minimal data for tests

    Ok(())
}

/// Minimal seed data — creates only the default superadmin from env vars
async fn seed_minimal(runtime: &ServerRuntimeContext) -> Result<()> {
    tracing::info!("Seeding minimal data...");

    let Some(email) = superadmin_email() else {
        tracing::warn!("SUPERADMIN_EMAIL not set — minimal seed skipped");
        return Ok(());
    };

    let tenant_slug = superadmin_tenant_slug();
    let tenant_name = superadmin_tenant_name();

    let tenant =
        tenants::Entity::find_or_create(runtime.db(), &tenant_name, &tenant_slug, None).await?;

    seed_user(
        runtime,
        tenant.id,
        &email,
        "Super Admin",
        rustok_core::UserRole::SuperAdmin,
    )
    .await?;

    tracing::info!(tenant_id = %tenant.id, "Minimal seed complete");

    Ok(())
}
