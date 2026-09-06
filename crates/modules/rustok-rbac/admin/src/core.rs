use crate::i18n::t;
use crate::model::{RbacAdminBootstrap, RbacModulePermissionGroup, RbacRoleInfo};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RbacInfoCardViewModel {
    pub label: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RbacPermissionsSectionViewModel {
    pub title: String,
    pub subtitle: String,
    pub count_label: String,
    pub permissions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RbacAdminOverviewViewModel {
    pub info_cards: Vec<RbacInfoCardViewModel>,
    pub granted_permissions: RbacPermissionsSectionViewModel,
    pub roles: Vec<RbacRoleInfo>,
    pub module_permissions: Vec<RbacModulePermissionGroup>,
}

pub fn build_rbac_admin_overview_view_model(
    locale: Option<&str>,
    bootstrap: RbacAdminBootstrap,
) -> RbacAdminOverviewViewModel {
    let permission_count = bootstrap.granted_permissions.len();
    RbacAdminOverviewViewModel {
        info_cards: vec![
            RbacInfoCardViewModel {
                label: t(locale, "rbac.info.tenant", "Tenant"),
                value: bootstrap.tenant_slug,
            },
            RbacInfoCardViewModel {
                label: t(locale, "rbac.info.role", "Role"),
                value: bootstrap.inferred_role,
            },
            RbacInfoCardViewModel {
                label: t(locale, "rbac.info.userId", "User ID"),
                value: bootstrap.current_user_id,
            },
        ],
        granted_permissions: RbacPermissionsSectionViewModel {
            title: t(locale, "rbac.permissions.title", "Granted Permissions"),
            subtitle: t(
                locale,
                "rbac.permissions.subtitle",
                "Live snapshot derived from the current security context.",
            ),
            count_label: format!(
                "{} {}",
                permission_count,
                t(locale, "rbac.permissions.count", "permissions")
            ),
            permissions: bootstrap.granted_permissions,
        },
        roles: bootstrap.roles,
        module_permissions: bootstrap.module_permissions,
    }
}

pub fn format_rbac_admin_bootstrap_error(
    locale: Option<&str>,
    error: impl std::fmt::Display,
) -> String {
    format!(
        "{}: {error}",
        t(
            locale,
            "rbac.error.loadBootstrap",
            "Failed to load RBAC bootstrap"
        )
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overview_view_model_formats_bootstrap_without_framework_runtime() {
        let view_model = build_rbac_admin_overview_view_model(
            Some("en"),
            RbacAdminBootstrap {
                tenant_slug: "acme".to_string(),
                current_user_id: "user-1".to_string(),
                inferred_role: "Admin".to_string(),
                granted_permissions: vec!["catalog.read".to_string(), "rbac.manage".to_string()],
                module_permissions: vec![RbacModulePermissionGroup {
                    module_slug: "catalog".to_string(),
                    permissions: vec!["catalog.read".to_string()],
                }],
                roles: vec![RbacRoleInfo {
                    slug: "admin".to_string(),
                    display_name: "Admin".to_string(),
                    permissions: vec!["settings:read".to_string()],
                }],
            },
        );

        assert_eq!(view_model.info_cards.len(), 3);
        assert_eq!(view_model.info_cards[0].value, "acme");
        assert_eq!(view_model.info_cards[1].value, "Admin");
        assert_eq!(view_model.granted_permissions.count_label, "2 permissions");
        assert_eq!(view_model.granted_permissions.permissions.len(), 2);
        assert_eq!(view_model.module_permissions[0].module_slug, "catalog");
        assert_eq!(view_model.roles[0].slug, "admin");
    }
}
