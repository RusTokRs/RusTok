use async_graphql::{Context, FieldError, Object, Result};
use rustok_api::{
    AuthContext, AuthPrincipalContext, Permission, StoredLocale, TenantContext,
    graphql::GraphQLError, has_effective_permission,
};
use rustok_core::{Rbac, UserRole, i18n::Locale};
use sea_orm::DatabaseConnection;

use crate::{
    RbacPresentationResourceKind, RbacPresentationStore, SeaOrmRbacPresentationStore,
};

use super::control_plane::require_direct_control_plane_user;
use super::types::RoleInfo;

#[derive(Default)]
pub struct RbacQuery;

const ALL_ROLES: &[UserRole] = &[
    UserRole::SuperAdmin,
    UserRole::Admin,
    UserRole::Manager,
    UserRole::Customer,
];

fn legacy_seed_display_name(role: &UserRole) -> &'static str {
    match role {
        UserRole::SuperAdmin => "Super Admin",
        UserRole::Admin => "Admin",
        UserRole::Manager => "Manager",
        UserRole::Customer => "Customer",
    }
}

async fn role_display_name(
    store: &SeaOrmRbacPresentationStore,
    tenant_id: uuid::Uuid,
    role: &UserRole,
    requested_locale: &StoredLocale,
) -> Result<String> {
    let resource_key = role.to_string();
    let exact = store
        .find_exact(
            tenant_id,
            RbacPresentationResourceKind::Role,
            &resource_key,
            requested_locale,
        )
        .await
        .map_err(|error| {
            tracing::error!(
                error = %error,
                %tenant_id,
                role = %resource_key,
                locale = %requested_locale.as_str(),
                "RBAC owner presentation read failed"
            );
            FieldError::new("RBAC role presentation is temporarily unavailable")
        })?;
    if let Some(presentation) = exact {
        return Ok(presentation.name);
    }

    let source_locale = StoredLocale::new("en")
        .expect("RBAC built-in presentation source locale must remain concrete");
    if requested_locale != &source_locale {
        let source = store
            .find_exact(
                tenant_id,
                RbacPresentationResourceKind::Role,
                &resource_key,
                &source_locale,
            )
            .await
            .map_err(|error| {
                tracing::error!(
                    error = %error,
                    %tenant_id,
                    role = %resource_key,
                    "RBAC owner source presentation read failed"
                );
                FieldError::new("RBAC role presentation is temporarily unavailable")
            })?;
        if let Some(presentation) = source {
            return Ok(presentation.name);
        }
    }

    // Transitional TR-3 bridge only. This fallback preserves current tenants until
    // built-in bootstrap presentation is seeded through the owner plane. It must be
    // removed together with the legacy native-admin path when the seed cutover lands.
    Ok(legacy_seed_display_name(role).to_string())
}

#[Object]
impl RbacQuery {
    /// List all platform roles with their permission sets.
    /// Requires a direct, session-bound user principal with `settings:read`.
    async fn roles(&self, ctx: &Context<'_>) -> Result<Vec<RoleInfo>> {
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
        let principal_context = ctx
            .data::<AuthPrincipalContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
        let tenant = ctx.data::<TenantContext>()?;

        require_direct_control_plane_user(auth, *principal_context, tenant.id)?;

        if !has_effective_permission(&auth.permissions, &Permission::SETTINGS_READ) {
            return Err(<FieldError as GraphQLError>::permission_denied(
                "settings:read required to list roles",
            ));
        }

        let db = ctx.data::<DatabaseConnection>()?;
        let locale = ctx.data_opt::<Locale>().copied().unwrap_or_default();
        let requested_locale = StoredLocale::new(locale.as_str())
            .expect("canonical GraphQL locale must be a valid stored locale");
        let store = SeaOrmRbacPresentationStore::new(db.clone());
        let mut roles = Vec::with_capacity(ALL_ROLES.len());

        for role in ALL_ROLES {
            let mut perms: Vec<String> = Rbac::permissions_for_role(role)
                .iter()
                .map(|permission| permission.to_string())
                .collect();
            perms.sort();
            roles.push(RoleInfo {
                slug: role.to_string(),
                display_name: role_display_name(&store, tenant.id, role, &requested_locale).await?,
                permissions: perms,
            });
        }

        Ok(roles)
    }
}
