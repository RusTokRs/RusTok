//! Default SuperAdmin Initializer
//!
//! Creates the default SuperAdmin user on first startup when configured
//! via environment. An already-existing account is never modified: if an
//! operator demoted or disabled it (for example during incident
//! response), a restart must not silently undo that decision.
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
use crate::services::auth_lifecycle::{AuthLifecycleError, AuthLifecycleService};
use crate::services::rbac_service::RbacService;
use crate::services::server_runtime_context::ServerRuntimeContext;

fn env_value(
    key: &str,
    value: std::result::Result<String, std::env::VarError>,
) -> Result<Option<String>> {
    match value {
        Ok(value) if !value.trim().is_empty() => Ok(Some(value)),
        Ok(_) | Err(std::env::VarError::NotPresent) => Ok(None),
        Err(std::env::VarError::NotUnicode(_)) => Err(crate::error::Error::Message(format!(
            "FATAL: env var {key} must be valid UTF-8."
        ))),
    }
}

fn env_first(primary: &str, fallback: &str) -> Result<Option<String>> {
    if let Some(value) = env_value(primary, std::env::var(primary))? {
        return Ok(Some(value));
    }
    env_value(fallback, std::env::var(fallback))
}

pub async fn ensure_default_superadmin(ctx: &ServerRuntimeContext) -> Result<()> {
    let Some(email) = env_first("SUPERADMIN_EMAIL", "SEED_ADMIN_EMAIL")? else {
        tracing::debug!("SUPERADMIN_EMAIL not set — skipping default superadmin setup");
        return Ok(());
    };

    let Some(password) = env_first("SUPERADMIN_PASSWORD", "SEED_ADMIN_PASSWORD")? else {
        tracing::warn!("SUPERADMIN_EMAIL is set but SUPERADMIN_PASSWORD is missing — skipping");
        return Ok(());
    };

    let tenant_slug = env_first("SUPERADMIN_TENANT_SLUG", "SEED_TENANT_SLUG")?
        .unwrap_or_else(|| "default".to_string());

    let tenant_name = env_first("SUPERADMIN_TENANT_NAME", "SEED_TENANT_NAME")?
        .unwrap_or_else(|| "Default".to_string());

    ensure_configured_superadmin(ctx, &email, &password, &tenant_name, &tenant_slug).await
}

async fn ensure_configured_superadmin(
    ctx: &ServerRuntimeContext,
    email: &str,
    password: &str,
    tenant_name: &str,
    tenant_slug: &str,
) -> Result<()> {
    // The lookup must use exactly the identity that account creation persists.
    // Check for an existing account before validating new credentials, then
    // validate before tenant creation so invalid bootstrap config has no
    // provisioning side effects.
    let email = crate::auth::normalize_email(email).map_err(AuthLifecycleError::from)?;
    let existing_tenant = tenants::find_by_slug(ctx.db(), tenant_slug).await?;

    if let Some(tenant) = existing_tenant.as_ref()
        && let Some(user) = users::Entity::find_by_email(ctx.db(), tenant.id, &email).await?
    {
        observe_existing_superadmin(ctx, tenant_slug, &user).await?;
        return Ok(());
    }

    if let Err(error) = crate::auth::validate_password_policy(
        password,
        crate::auth::DEFAULT_MIN_PASSWORD_LENGTH_CHARS,
    ) {
        // Another replica may have provisioned the exact identity after the
        // first lookup. Preserve bootstrap idempotence without creating a
        // tenant solely to discover that the configured password is invalid.
        if let Some(tenant) = tenants::find_by_slug(ctx.db(), tenant_slug).await?
            && let Some(user) = users::Entity::find_by_email(ctx.db(), tenant.id, &email).await?
        {
            observe_existing_superadmin(ctx, tenant_slug, &user).await?;
            return Ok(());
        }
        return Err(AuthLifecycleError::from(error).into());
    }

    let tenant = match existing_tenant {
        Some(tenant) => tenant,
        None => tenants::find_or_create(ctx.db(), tenant_name, tenant_slug, None).await?,
    };

    let result = AuthLifecycleService::create_user_runtime(
        ctx,
        tenant.id,
        &email,
        password,
        Some("Super Admin".to_string()),
        rustok_core::UserRole::SuperAdmin,
        Some(rustok_core::UserStatus::Active),
        crate::auth::DEFAULT_MIN_PASSWORD_LENGTH_CHARS,
    )
    .await;

    finish_superadmin_creation(ctx, tenant.id, tenant_slug, &email, result).await
}

async fn finish_superadmin_creation(
    ctx: &ServerRuntimeContext,
    tenant_id: uuid::Uuid,
    tenant_slug: &str,
    email: &str,
    result: std::result::Result<users::Model, AuthLifecycleError>,
) -> Result<()> {
    match result {
        Ok(user) => {
            tracing::info!(
                tenant = %tenant_slug,
                user_id = %user.id,
                "Default superadmin created"
            );
            Ok(())
        }
        Err(AuthLifecycleError::EmailAlreadyExists) => {
            // The account-creation transaction has ended. Re-read the exact
            // tenant/email identity on the database connection, not the
            // failed transaction. Another replica may have won the race.
            // An existing account is never repaired or elevated by bootstrap.
            let user = users::Entity::find_by_email(ctx.db(), tenant_id, email)
                .await?
                .ok_or(AuthLifecycleError::EmailAlreadyExists)?;
            observe_existing_superadmin(ctx, tenant_slug, &user).await
        }
        Err(error) => Err(error.into()),
    }
}

async fn observe_existing_superadmin(
    ctx: &ServerRuntimeContext,
    tenant_slug: &str,
    user: &users::Model,
) -> Result<()> {
    let role = RbacService::get_user_role(ctx.db(), &user.tenant_id, &user.id).await?;
    if role == rustok_core::UserRole::SuperAdmin {
        tracing::debug!(
            tenant = %tenant_slug,
            user_id = %user.id,
            "Default superadmin already exists"
        );
    } else {
        tracing::warn!(
            tenant = %tenant_slug,
            user_id = %user.id,
            current_role = %role,
            "Configured superadmin account exists without the SuperAdmin role; leaving it unchanged (recovery is an explicit admin action)"
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::env_value;

    #[test]
    fn missing_and_empty_optional_environment_values_are_absent() {
        assert_eq!(
            env_value("SUPERADMIN_EMAIL", Ok("  ".to_string())).expect("empty is absent"),
            None
        );
        assert_eq!(
            env_value(
                "SUPERADMIN_EMAIL",
                Err(std::env::VarError::NotPresent)
            )
            .expect("missing is absent"),
            None
        );
        assert_eq!(
            env_value("SUPERADMIN_EMAIL", Ok("admin@example.test".to_string()))
                .expect("valid value"),
            Some("admin@example.test".to_string())
        );
    }

    #[test]
    fn non_unicode_optional_environment_values_fail_closed() {
        let error = env_value(
            "SUPERADMIN_EMAIL",
            Err(std::env::VarError::NotUnicode(std::ffi::OsString::from(
                "invalid",
            ))),
        )
        .expect_err("non-Unicode configuration must not silently fall back");
        assert!(error.to_string().contains("SUPERADMIN_EMAIL"));
        assert!(error.to_string().contains("valid UTF-8"));
    }
}

#[cfg(test)]
mod existing_account_tests {
    use super::*;
    use crate::common::settings::RustokSettings;
    use rustok_core::{UserRole, UserStatus};
    use rustok_migrations::SqliteTestMigrator;
    use rustok_outbox::entity as sys_events;
    use rustok_test_utils::db::setup_test_db_with_migrations;
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

    async fn test_context() -> ServerRuntimeContext {
        let db = setup_test_db_with_migrations::<SqliteTestMigrator>().await;
        ServerRuntimeContext::new(db, RustokSettings::default())
    }

    #[tokio::test]
    async fn normalized_existing_account_is_never_modified_by_bootstrap() {
        let ctx = test_context().await;
        let tenant = tenants::find_or_create(ctx.db(), "Seed tenant", "seed-tenant", None)
            .await
            .expect("tenant");
        let existing = AuthLifecycleService::create_user_runtime(
            &ctx,
            tenant.id,
            "seed@example.test",
            "OriginalPassword123!",
            Some("Existing account".to_string()),
            UserRole::Customer,
            Some(UserStatus::Inactive),
            crate::auth::DEFAULT_MIN_PASSWORD_LENGTH_CHARS,
        )
        .await
        .expect("existing user");

        ensure_configured_superadmin(
            &ctx,
            "  SEED@EXAMPLE.TEST  ",
            "short",
            "Seed tenant",
            "seed-tenant",
        )
        .await
        .expect("normalized existing account is an idempotent success");

        let actual = users::Entity::find_by_email(ctx.db(), tenant.id, "seed@example.test")
            .await
            .expect("read user")
            .expect("existing user");
        assert_eq!(actual.id, existing.id);
        assert_eq!(actual.status, UserStatus::Inactive);
        assert_eq!(actual.password_hash, existing.password_hash);
        assert_eq!(actual.name, existing.name);
        assert_eq!(
            RbacService::get_user_role(ctx.db(), &tenant.id, &actual.id)
                .await
                .expect("role"),
            UserRole::Customer
        );
    }

    #[tokio::test]
    async fn duplicate_creation_recovers_only_an_existing_exact_identity() {
        let ctx = test_context().await;
        let tenant = tenants::find_or_create(ctx.db(), "Race tenant", "race-tenant", None)
            .await
            .expect("tenant");
        let other = tenants::find_or_create(ctx.db(), "Other tenant", "other-tenant", None)
            .await
            .expect("other tenant");
        // Model the winning transaction as already committed when the losing
        // creation returns its typed duplicate-email error.
        let winner = AuthLifecycleService::create_user_runtime(
            &ctx,
            tenant.id,
            "race@example.test",
            "OriginalPassword123!",
            None,
            UserRole::Customer,
            Some(UserStatus::Inactive),
            crate::auth::DEFAULT_MIN_PASSWORD_LENGTH_CHARS,
        )
        .await
        .expect("winning account");
        finish_superadmin_creation(
            &ctx,
            tenant.id,
            "race-tenant",
            &winner.email,
            Err(AuthLifecycleError::EmailAlreadyExists),
        )
        .await
        .expect("committed same-tenant identity resolves the conflict");
        let actual = users::Entity::find_by_email(ctx.db(), tenant.id, &winner.email)
            .await
            .expect("read")
            .expect("winner");
        assert_eq!(actual.id, winner.id);
        assert_eq!(actual.status, UserStatus::Inactive);
        assert_eq!(actual.password_hash, winner.password_hash);
        assert_eq!(
            RbacService::get_user_role(ctx.db(), &tenant.id, &winner.id)
                .await
                .expect("role"),
            UserRole::Customer
        );

        assert!(
            finish_superadmin_creation(
                &ctx,
                other.id,
                "other-tenant",
                &winner.email,
                Err(AuthLifecycleError::EmailAlreadyExists),
            )
            .await
            .is_err()
        );
        assert!(
            finish_superadmin_creation(
                &ctx,
                tenant.id,
                "race-tenant",
                &winner.email,
                Err(AuthLifecycleError::Internal(
                    crate::error::Error::InternalServerError
                )),
            )
            .await
            .is_err()
        );
    }

    #[tokio::test]
    async fn default_superadmin_bootstrap_publishes_registration_event() {
        let ctx = test_context().await;
        let tenant =
            tenants::find_or_create(ctx.db(), "Bootstrap tenant", "bootstrap-tenant", None)
                .await
                .expect("tenant");

        ensure_configured_superadmin(
            &ctx,
            "bootstrap-admin@example.test",
            "StrongBootstrapPassword123!",
            "Bootstrap tenant",
            "bootstrap-tenant",
        )
        .await
        .expect("bootstrap superadmin");

        let events = sys_events::Entity::find()
            .filter(sys_events::Column::TenantId.eq(tenant.id))
            .filter(sys_events::Column::EventType.eq("user.account_registered"))
            .all(ctx.db())
            .await
            .expect("registration outbox events");
        assert_eq!(
            events.len(),
            1,
            "bootstrap account creation must publish exactly one registration event"
        );
    }

    #[tokio::test]
    async fn invalid_password_does_not_provision_a_tenant() {
        let ctx = test_context().await;
        assert!(
            ensure_configured_superadmin(
                &ctx,
                "bootstrap-admin@example.test",
                "short",
                "Invalid password tenant",
                "invalid-password-tenant",
            )
            .await
            .is_err()
        );
        assert!(
            tenants::find_by_slug(ctx.db(), "invalid-password-tenant")
                .await
                .expect("tenant lookup")
                .is_none()
        );
    }

    #[tokio::test]
    async fn invalid_email_does_not_provision_a_tenant() {
        let ctx = test_context().await;
        assert!(
            ensure_configured_superadmin(
                &ctx,
                "not-an-email",
                "OriginalPassword123!",
                "Invalid tenant",
                "invalid-seed-tenant",
            )
            .await
            .is_err()
        );
        assert!(
            tenants::find_by_slug(ctx.db(), "invalid-seed-tenant")
                .await
                .expect("tenant lookup")
                .is_none()
        );
    }
}
