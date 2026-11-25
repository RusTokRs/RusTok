use std::collections::{BTreeMap, HashMap};

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

    let backend = db.get_database_backend();
    let role_rows = db
        .query_all_raw(prepare_control_plane_statement(
            backend,
            "SELECT id, slug, name, description, is_system FROM roles WHERE tenant_id = ? ORDER BY is_system DESC, slug ASC",
            vec![tenant_id.into()],
        ))
        .await?;

    if role_rows.is_empty() {
        return Ok(Vec::new());
    }

    let perm_rows = db
        .query_all_raw(prepare_control_plane_statement(
            backend,
            "SELECT rp.role_id, p.resource, p.action FROM role_permissions rp JOIN roles r ON r.id = rp.role_id JOIN permissions p ON p.id = rp.permission_id WHERE r.tenant_id = ? AND p.tenant_id = ?",
            vec![tenant_id.into(), tenant_id.into()],
        ))
        .await?;

    let mut perms_by_role: HashMap<Uuid, Vec<String>> = HashMap::new();
    for row in perm_rows {
        let role_id: Uuid = row.try_get("", "role_id")?;
        let resource: String = row.try_get("", "resource")?;
        let action: String = row.try_get("", "action")?;
        perms_by_role
            .entry(role_id)
            .or_default()
            .push(format!("{resource}:{action}"));
    }
    for perms in perms_by_role.values_mut() {
        perms.sort();
        perms.dedup();
    }

    let requested_locale = locale
        .and_then(|tag| StoredLocale::new(tag).ok())
        .unwrap_or_else(|| StoredLocale::new("en").expect("static en locale"));
    let presentation_rows = db
        .query_all_raw(prepare_control_plane_statement(
            backend,
            "SELECT resource_key, locale, name FROM rbac_localized_presentations WHERE tenant_id = ? AND resource_kind = 'role' AND locale IN (?, 'en')",
            vec![tenant_id.into(), requested_locale.as_str().into()],
        ))
        .await?;

    let mut exact_names: BTreeMap<String, String> = BTreeMap::new();
    let mut en_names: BTreeMap<String, String> = BTreeMap::new();
    for row in presentation_rows {
        let key: String = row.try_get("", "resource_key")?;
        let row_locale: String = row.try_get("", "locale")?;
        let row_name: String = row.try_get("", "name")?;
        if row_locale == requested_locale.as_str() {
            exact_names.insert(key, row_name);
        } else if row_locale == "en" {
            en_names.insert(key, row_name);
        }
    }

    let mut roles = Vec::with_capacity(role_rows.len());
    for row in role_rows {
        let id: Uuid = row.try_get("", "id")?;
        let slug: String = row.try_get("", "slug")?;
        let persisted_name: String = row.try_get("", "name")?;
        let description: Option<String> = row.try_get("", "description")?;
        let is_system: bool = row.try_get("", "is_system")?;
        let permissions = perms_by_role.remove(&id).unwrap_or_default();

        let inactive_permissions = match availability {
            Some(snapshot) => permissions
                .iter()
                .filter(|permission| {
                    matches!(
                        snapshot.status(permission),
                        crate::ModulePermissionStatus::Disabled(_)
                    )
                })
                .cloned()
                .collect(),
            None => Vec::new(),
        };

        let display_name = if is_system {
            if let Ok(builtin) = slug.parse::<UserRole>() {
                if let Some(localized) = exact_names.get(&slug) {
                    localized.clone()
                } else if !persisted_name.trim().is_empty()
                    && persisted_name != builtin_seed_display_name(&builtin)
                {
                    persisted_name
                } else if let Some(en) = en_names.get(&slug) {
                    en.clone()
                } else {
                    persisted_name
                }
            } else {
                persisted_name
            }
        } else {
            persisted_name
        };

        roles.push(TenantRoleOverviewRecord {
            id: Some(id),
            slug,
            display_name,
            description,
            is_system,
            permissions,
            inactive_permissions,
        });
    }

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
