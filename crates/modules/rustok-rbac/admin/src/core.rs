use rustok_grid::{ColumnAlign, GridColumnDef};

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
pub struct RbacPermissionRowViewModel {
    pub module_slug: String,
    pub permission: String,
    pub roles: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RbacAdminOverviewViewModel {
    pub info_cards: Vec<RbacInfoCardViewModel>,
    pub granted_permissions: RbacPermissionsSectionViewModel,
    pub roles: Vec<RbacRoleInfo>,
    pub module_permissions: Vec<RbacModulePermissionGroup>,
}

pub fn rbac_role_grid_columns(locale: Option<&str>) -> Vec<GridColumnDef> {
    let is_ru = locale.map(|l| l.starts_with("ru")).unwrap_or(false);
    vec![
        GridColumnDef::new("name", if is_ru { "Роль" } else { "Role" })
            .width(200)
            .align(ColumnAlign::Left),
        GridColumnDef::new("slug", if is_ru { "Слаг" } else { "Slug" })
            .width(160)
            .align(ColumnAlign::Left),
        GridColumnDef::new("type", if is_ru { "Тип" } else { "Type" })
            .width(130)
            .align(ColumnAlign::Center),
        GridColumnDef::new(
            "permissions_count",
            if is_ru { "Разрешения" } else { "Permissions" },
        )
        .width(130)
        .align(ColumnAlign::Center),
        GridColumnDef::new(
            "preview",
            if is_ru { "Список прав" } else { "Permission List" },
        )
        .width(400)
        .align(ColumnAlign::Left)
        .not_sortable(),
    ]
}

pub fn rbac_permission_grid_columns(locale: Option<&str>) -> Vec<GridColumnDef> {
    let is_ru = locale.map(|l| l.starts_with("ru")).unwrap_or(false);
    vec![
        GridColumnDef::new("module", if is_ru { "Модуль" } else { "Module" })
            .width(150)
            .align(ColumnAlign::Left),
        GridColumnDef::new("permission", if is_ru { "Ключ права" } else { "Permission Key" })
            .width(260)
            .align(ColumnAlign::Left),
        GridColumnDef::new("roles", if is_ru { "Назначено ролям" } else { "Granted to Roles" })
            .width(350)
            .align(ColumnAlign::Left)
            .not_sortable(),
    ]
}

pub fn build_rbac_permission_rows(
    groups: &[RbacModulePermissionGroup],
    roles: &[RbacRoleInfo],
) -> Vec<RbacPermissionRowViewModel> {
    let mut rows = Vec::new();
    for group in groups {
        for perm in &group.permissions {
            let mut matching_roles = Vec::new();
            for role in roles {
                if role.permissions.contains(perm) {
                    matching_roles.push(role.display_name.clone());
                }
            }
            rows.push(RbacPermissionRowViewModel {
                module_slug: group.module_slug.clone(),
                permission: perm.clone(),
                roles: matching_roles,
            });
        }
    }
    rows.sort_by(|a, b| a.permission.cmp(&b.permission));
    rows
}

pub fn filter_rbac_roles(roles: &[RbacRoleInfo], search: &str) -> Vec<RbacRoleInfo> {
    let query = search.trim().to_lowercase();
    if query.is_empty() {
        return roles.to_vec();
    }
    roles
        .iter()
        .filter(|role| {
            role.display_name.to_lowercase().contains(&query)
                || role.slug.to_lowercase().contains(&query)
                || role.permissions.iter().any(|p| p.to_lowercase().contains(&query))
        })
        .cloned()
        .collect()
}

pub fn filter_rbac_permission_rows(
    rows: &[RbacPermissionRowViewModel],
    search: &str,
    module_filter: Option<&str>,
) -> Vec<RbacPermissionRowViewModel> {
    let query = search.trim().to_lowercase();
    rows.iter()
        .filter(|row| {
            if let Some(m) = module_filter {
                if m != "all" && !m.is_empty() && row.module_slug != m {
                    return false;
                }
            }
            if query.is_empty() {
                return true;
            }
            row.permission.to_lowercase().contains(&query)
                || row.module_slug.to_lowercase().contains(&query)
                || row.roles.iter().any(|r| r.to_lowercase().contains(&query))
        })
        .cloned()
        .collect()
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
            count_label: crate::i18n::format(
                locale,
                "rbac.permissions.count",
                Some(&rustok_ui_i18n::fluent_args!("count" => permission_count)),
                &format!("{permission_count} permissions"),
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

    fn without_bidi_isolates(value: &str) -> String {
        value.replace(['\u{2068}', '\u{2069}'], "")
    }

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
                    id: None,
                    slug: "admin".to_string(),
                    display_name: "Admin".to_string(),
                    description: None,
                    is_system: true,
                    permissions: vec!["settings:read".to_string()],
                }],
            },
        );

        assert_eq!(view_model.info_cards.len(), 3);
        assert_eq!(view_model.info_cards[0].value, "acme");
        assert_eq!(view_model.info_cards[1].value, "Admin");
        assert_eq!(
            without_bidi_isolates(&view_model.granted_permissions.count_label),
            "2 permissions"
        );
        assert_eq!(view_model.granted_permissions.permissions.len(), 2);
        assert_eq!(view_model.module_permissions[0].module_slug, "catalog");
        assert_eq!(view_model.roles[0].slug, "admin");
    }

    #[test]
    fn overview_view_model_pluralizes_russian_permission_counts() {
        let make_bootstrap = |count: usize| RbacAdminBootstrap {
            tenant_slug: "acme".to_string(),
            current_user_id: "user-1".to_string(),
            inferred_role: "Admin".to_string(),
            granted_permissions: (0..count).map(|i| format!("perm.{i}")).collect(),
            module_permissions: vec![],
            roles: vec![],
        };

        let vm1 = build_rbac_admin_overview_view_model(Some("ru"), make_bootstrap(1));
        assert_eq!(
            without_bidi_isolates(&vm1.granted_permissions.count_label),
            "1 право"
        );

        let vm2 = build_rbac_admin_overview_view_model(Some("ru"), make_bootstrap(2));
        assert_eq!(
            without_bidi_isolates(&vm2.granted_permissions.count_label),
            "2 права"
        );

        let vm5 = build_rbac_admin_overview_view_model(Some("ru"), make_bootstrap(5));
        assert_eq!(
            without_bidi_isolates(&vm5.granted_permissions.count_label),
            "5 прав"
        );
    }

    #[test]
    fn bootstrap_error_formats_localized_messages() {
        let en_err = format_rbac_admin_bootstrap_error(Some("en"), "forbidden");
        assert_eq!(en_err, "Failed to load RBAC bootstrap: forbidden");

        let ru_err = format_rbac_admin_bootstrap_error(Some("ru"), "отказано в доступе");
        assert_eq!(ru_err, "Не удалось загрузить RBAC bootstrap: отказано в доступе");
    }

    #[test]
    fn filter_rbac_roles_by_name_and_permission() {
        let roles = vec![
            RbacRoleInfo {
                id: None,
                slug: "admin".to_string(),
                display_name: "Admin".to_string(),
                description: None,
                is_system: true,
                permissions: vec!["settings:read".to_string(), "users:create".to_string()],
            },
            RbacRoleInfo {
                id: None,
                slug: "customer".to_string(),
                display_name: "Customer".to_string(),
                description: None,
                is_system: true,
                permissions: vec!["catalog:read".to_string()],
            },
        ];

        assert_eq!(filter_rbac_roles(&roles, "admin").len(), 1);
        assert_eq!(filter_rbac_roles(&roles, "settings").len(), 1);
        assert_eq!(filter_rbac_roles(&roles, "nonexistent").len(), 0);
        assert_eq!(filter_rbac_roles(&roles, "").len(), 2);
    }

    #[test]
    fn build_and_filter_permission_rows() {
        let groups = vec![
            RbacModulePermissionGroup {
                module_slug: "users".to_string(),
                permissions: vec!["users:create".to_string(), "users:read".to_string()],
            },
            RbacModulePermissionGroup {
                module_slug: "catalog".to_string(),
                permissions: vec!["catalog:read".to_string()],
            },
        ];
        let roles = vec![
            RbacRoleInfo {
                id: None,
                slug: "admin".to_string(),
                display_name: "Admin".to_string(),
                description: None,
                is_system: true,
                permissions: vec!["users:create".to_string(), "catalog:read".to_string()],
            },
        ];

        let rows = build_rbac_permission_rows(&groups, &roles);
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].permission, "catalog:read");
        assert_eq!(rows[0].roles, vec!["Admin".to_string()]);

        let filtered_mod = filter_rbac_permission_rows(&rows, "", Some("users"));
        assert_eq!(filtered_mod.len(), 2);

        let filtered_search = filter_rbac_permission_rows(&rows, "create", None);
        assert_eq!(filtered_search.len(), 1);
    }
}
