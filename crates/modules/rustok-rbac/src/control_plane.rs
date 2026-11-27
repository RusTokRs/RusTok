use std::collections::{HashMap, hash_map::Entry};

use rustok_api::{AuthPrincipalKind, StoredLocale};
use rustok_core::UserRole;
use sea_orm::{ConnectionTrait, DbBackend, Statement, Value};
use thiserror::Error;
use uuid::Uuid;

use crate::services::module_availability::PermissionAvailability;

/// Host-neutral authenticated facts required by the RBAC control-plane policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RbacControlPlanePrincipal {
    pub tenant_id: Uuid,
    pub principal_kind: AuthPrincipalKind,
}

/// Fail-closed admission errors for tenant-scoped RBAC control-plane operations.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum RbacControlPlaneAdmissionError {
    #[error("RBAC control plane requires a direct, session-bound user principal")]
    DirectSessionRequired,
    #[error("authenticated principal belongs to another tenant")]
    TenantMismatch,
}

/// Require a direct user whose authenticated tenant matches the routed tenant
/// before any RBAC role or permission control-plane admission.
///
/// Authentication validates grant, client and session invariants once and stores
/// the resulting typed principal kind in the shared authorization context.
/// Delegated users and services remain valid for data-plane operations admitted
/// by their effective permissions, but they cannot enter RBAC control-plane state.
pub fn require_direct_control_plane_user(
    principal: RbacControlPlanePrincipal,
    tenant_id: Uuid,
) -> Result<(), RbacControlPlaneAdmissionError> {
    if !principal.principal_kind.is_direct_user() {
        return Err(RbacControlPlaneAdmissionError::DirectSessionRequired);
    }

    if principal.tenant_id.is_nil() || tenant_id.is_nil() || principal.tenant_id != tenant_id {
        return Err(RbacControlPlaneAdmissionError::TenantMismatch);
    }

    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TenantRoleOverviewRecord {
    pub id: Option<Uuid>,
    pub slug: String,
    pub display_name: String,
    pub description: Option<String>,
    pub is_system: bool,
    pub permissions: Vec<String>,
    pub inactive_permissions: Vec<String>,
}

fn prepare_control_plane_statement(backend: DbBackend, sql: &str, values: Vec<Value>) -> Statement {
    let rewritten = match backend {
        DbBackend::Postgres => {
            let mut out = String::with_capacity(sql.len() + 16);
            let mut index = 1usize;
            for ch in sql.chars() {
                if ch == '?' {
                    out.push('$');
                    out.push_str(&index.to_string());
                    index += 1;
                } else {
                    out.push(ch);
                }
            }
            out
        }
        _ => sql.to_string(),
    };
    Statement::from_sql_and_values(backend, rewritten, values)
}

fn builtin_seed_display_name(role: &UserRole) -> &'static str {
    match role {
        UserRole::SuperAdmin => "Super Admin",
        UserRole::Admin => "Admin",
        UserRole::Manager => "Manager",
        UserRole::Customer => "Customer",
    }
}

/// Read persisted tenant roles, permission grants, and localized built-in display
/// names from the authoritative relational store.
pub async fn load_tenant_roles_overview_on<C: ConnectionTrait>(
    db: &C,
    tenant_id: Uuid,
    availability: Option<&PermissionAvailability>,
    locale: Option<&str>,
) -> Result<Vec<TenantRoleOverviewRecord>, sea_orm::DbErr> {
    if tenant_id.is_nil() {
        return Err(sea_orm::DbErr::Custom(
            "tenant_id must not be nil when loading RBAC role overview".to_string(),
        ));
    }

    let requested_locale = match locale.and_then(|tag| StoredLocale::new(tag).ok()) {
        Some(locale) => locale,
        None => StoredLocale::new("en")
            .map_err(|error| sea_orm::DbErr::Custom(error.to_string()))?,
    };
    let backend = db.get_database_backend();
    // One statement observes role identity, grants and display names together.
    // Keep roles with zero grants; exclude foreign-tenant permission links.
    let rows = db
        .query_all_raw(prepare_control_plane_statement(
            backend,
            "SELECT r.id, r.slug, r.name, r.description, r.is_system, \
                    p.id AS permission_id, p.resource, p.action, \
                    exact_copy.name AS exact_name, source_copy.name AS source_name \
             FROM roles r \
             LEFT JOIN (role_permissions rp JOIN permissions p ON p.id = rp.permission_id) \
               ON rp.role_id = r.id AND p.tenant_id = r.tenant_id \
             LEFT JOIN rbac_localized_presentations exact_copy \
               ON exact_copy.tenant_id = r.tenant_id AND exact_copy.resource_kind = 'role' \
               AND exact_copy.resource_key = r.slug AND exact_copy.locale = ? \
             LEFT JOIN rbac_localized_presentations source_copy \
               ON source_copy.tenant_id = r.tenant_id AND source_copy.resource_kind = 'role' \
               AND source_copy.resource_key = r.slug AND source_copy.locale = 'en' \
             WHERE r.tenant_id = ?",
            vec![requested_locale.as_str().into(), tenant_id.into()],
        ))
        .await?;

    let mut by_id: HashMap<Uuid, TenantRoleOverviewRecord> = HashMap::new();
    for row in rows {
        let id: Uuid = row.try_get("", "id")?;
        let record = match by_id.entry(id) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => {
                let slug: String = row.try_get("", "slug")?;
                let persisted_name: String = row.try_get("", "name")?;
                let is_system: bool = row.try_get("", "is_system")?;
                let display_name = if is_system {
                    if let Ok(builtin) = slug.parse::<UserRole>() {
                        let exact_name: Option<String> = row.try_get("", "exact_name")?;
                        let source_name: Option<String> = row.try_get("", "source_name")?;
                        let seed_name = builtin_seed_display_name(&builtin);
                        if let Some(name) = exact_name {
                            name
                        } else if !persisted_name.trim().is_empty() && persisted_name != seed_name {
                            persisted_name
                        } else {
                            source_name.unwrap_or_else(|| seed_name.to_string())
                        }
                    } else {
                        persisted_name
                    }
                } else {
                    persisted_name
                };
                entry.insert(TenantRoleOverviewRecord {
                    id: Some(id),
                    slug,
                    display_name,
                    description: row.try_get("", "description")?,
                    is_system,
                    permissions: Vec::new(),
                    inactive_permissions: Vec::new(),
                })
            }
        };
        if row.try_get::<Option<Uuid>>("", "permission_id")?.is_some() {
            let resource: String = row.try_get("", "resource")?;
            let action: String = row.try_get("", "action")?;
            record.permissions.push(format!("{resource}:{action}"));
        }
    }

    let mut roles: Vec<_> = by_id.into_values().collect();
    for role in &mut roles {
        role.permissions.sort();
        role.permissions.dedup();
        if let Some(snapshot) = availability {
            role.inactive_permissions = role
                .permissions
                .iter()
                .filter(|permission| !snapshot.status(permission).is_active())
                .cloned()
                .collect();
        }
    }
    roles.sort_by(|left, right| {
        right.is_system.cmp(&left.is_system)
            .then_with(|| left.slug.cmp(&right.slug))
            .then_with(|| left.id.cmp(&right.id))
    });

    Ok(roles)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn principal(tenant_id: Uuid, principal_kind: AuthPrincipalKind) -> RbacControlPlanePrincipal {
        RbacControlPlanePrincipal {
            tenant_id,
            principal_kind,
        }
    }

    #[test]
    fn direct_user_is_allowed_for_matching_tenant() {
        let tenant_id = Uuid::new_v4();

        assert_eq!(
            require_direct_control_plane_user(
                principal(tenant_id, AuthPrincipalKind::DirectUser),
                tenant_id,
            ),
            Ok(())
        );
    }

    #[test]
    fn delegated_and_service_principals_are_denied_even_with_management_permission() {
        for principal_kind in [AuthPrincipalKind::DelegatedUser, AuthPrincipalKind::Service] {
            let tenant_id = Uuid::new_v4();

            assert_eq!(
                require_direct_control_plane_user(principal(tenant_id, principal_kind), tenant_id,),
                Err(RbacControlPlaneAdmissionError::DirectSessionRequired)
            );
        }
    }

    #[test]
    fn cross_tenant_context_is_denied() {
        let principal = principal(Uuid::new_v4(), AuthPrincipalKind::DirectUser);

        assert_eq!(
            require_direct_control_plane_user(principal, Uuid::new_v4()),
            Err(RbacControlPlaneAdmissionError::TenantMismatch)
        );
    }

    #[test]
    fn nil_tenant_id_is_denied() {
        let nil_tenant = Uuid::nil();
        let valid_tenant = Uuid::new_v4();

        assert_eq!(
            require_direct_control_plane_user(
                principal(nil_tenant, AuthPrincipalKind::DirectUser),
                nil_tenant,
            ),
            Err(RbacControlPlaneAdmissionError::TenantMismatch)
        );

        assert_eq!(
            require_direct_control_plane_user(
                principal(valid_tenant, AuthPrincipalKind::DirectUser),
                nil_tenant,
            ),
            Err(RbacControlPlaneAdmissionError::TenantMismatch)
        );

        assert_eq!(
            require_direct_control_plane_user(
                principal(nil_tenant, AuthPrincipalKind::DirectUser),
                valid_tenant,
            ),
            Err(RbacControlPlaneAdmissionError::TenantMismatch)
        );
    }
}
