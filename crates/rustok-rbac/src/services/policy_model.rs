use rustok_core::{Action, Permission};
use std::collections::HashSet;
use std::fmt::Write;

const DEFAULT_TENANT_POLICY_MODEL: &str = include_str!("../../config/tenant_policy_model.conf");
const RESOLVED_PERMISSIONS_SUBJECT: &str = "__resolved_permissions_subject__";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TenantPolicyEnforcer {
    tenant_domain: String,
    allowed_permissions: HashSet<Permission>,
}

impl TenantPolicyEnforcer {
    pub fn enforce(&self, subject: &str, tenant_domain: &str, permission: &Permission) -> bool {
        if subject != resolved_permissions_subject() || tenant_domain != self.tenant_domain {
            return false;
        }

        self.allowed_permissions.contains(permission)
            || self
                .allowed_permissions
                .contains(&Permission::new(permission.resource, Action::Manage))
    }
}

pub fn default_tenant_policy_model() -> &'static str {
    DEFAULT_TENANT_POLICY_MODEL
}

pub fn resolved_permissions_subject() -> &'static str {
    RESOLVED_PERMISSIONS_SUBJECT
}

pub fn build_tenant_policy_csv(
    tenant_id: &uuid::Uuid,
    resolved_permissions: &[Permission],
) -> String {
    let tenant_domain = tenant_id.to_string();
    let mut policy = String::new();

    for permission in resolved_permissions {
        let subject = permission_subject(permission);
        let object = permission.resource.to_string();
        let action = permission_action_token(permission);

        let _ = writeln!(
            &mut policy,
            "p, {subject}, {tenant_domain}, {object}, {action}"
        );
        let _ = writeln!(
            &mut policy,
            "g, {subject_user}, {subject}, {tenant_domain}",
            subject_user = resolved_permissions_subject(),
        );
    }

    policy
}

pub fn build_tenant_policy_enforcer(
    tenant_id: &uuid::Uuid,
    resolved_permissions: &[Permission],
) -> TenantPolicyEnforcer {
    TenantPolicyEnforcer {
        tenant_domain: tenant_id.to_string(),
        allowed_permissions: resolved_permissions.iter().copied().collect(),
    }
}

fn permission_subject(permission: &Permission) -> String {
    format!("perm::{permission}")
}

fn permission_action_token(permission: &Permission) -> String {
    match permission.action {
        Action::Manage => "*".to_string(),
        _ => permission.action.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_tenant_policy_csv, build_tenant_policy_enforcer, default_tenant_policy_model,
        resolved_permissions_subject,
    };
    use rustok_core::Permission;

    #[test]
    fn model_contains_core_sections() {
        let model = default_tenant_policy_model();
        assert!(model.contains("[request_definition]"));
        assert!(model.contains("[policy_definition]"));
        assert!(model.contains("[role_definition]"));
        assert!(model.contains("[matchers]"));
    }

    #[test]
    fn model_declares_tenant_domain_field() {
        let model = default_tenant_policy_model();
        assert!(model.contains("r = sub, dom, obj, act"));
        assert!(model.contains("p = sub, dom, obj, act"));
        assert!(model.contains("g = _, _, _"));
    }

    #[test]
    fn policy_csv_maps_manage_to_wildcard_action() {
        let tenant_id = uuid::Uuid::new_v4();
        let policy = build_tenant_policy_csv(&tenant_id, &[Permission::USERS_MANAGE]);

        assert!(policy.contains("p, perm::users:manage,"));
        assert!(policy.contains(", users, *"));
        assert!(policy.contains(resolved_permissions_subject()));
    }

    #[test]
    fn enforcer_allows_permissions_loaded_from_generated_policy() {
        let tenant_id = uuid::Uuid::new_v4();
        let tenant_domain = tenant_id.to_string();
        let enforcer = build_tenant_policy_enforcer(
            &tenant_id,
            &[Permission::USERS_MANAGE, Permission::PAGES_READ],
        );

        assert!(enforcer.enforce(
            resolved_permissions_subject(),
            &tenant_domain,
            &Permission::USERS_UPDATE,
        ));
        assert!(enforcer.enforce(
            resolved_permissions_subject(),
            &tenant_domain,
            &Permission::PAGES_READ,
        ));
    }

    #[test]
    fn enforcer_denies_wrong_subject_or_tenant() {
        let tenant_id = uuid::Uuid::new_v4();
        let enforcer = build_tenant_policy_enforcer(&tenant_id, &[Permission::USERS_READ]);

        assert!(!enforcer.enforce("other", &tenant_id.to_string(), &Permission::USERS_READ,));
        assert!(!enforcer.enforce(
            resolved_permissions_subject(),
            &uuid::Uuid::new_v4().to_string(),
            &Permission::USERS_READ,
        ));
    }
}
