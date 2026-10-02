use crate::model::{TenantAdminBootstrap, TenantAdminModule};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TenantAdminShellCopy {
    pub badge: String,
    pub title: String,
    pub subtitle: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TenantAdminInfoCards {
    pub tenant_label: String,
    pub name_label: String,
    pub domain_label: String,
    pub status_label: String,
    pub domain_value: String,
    pub status_value: String,
}

pub struct TenantAdminInfoCardCopy {
    pub tenant_label: String,
    pub name_label: String,
    pub domain_label: String,
    pub status_label: String,
    pub not_available: String,
    pub active: String,
    pub inactive: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TenantAdminModulesCopy {
    pub title: String,
    pub subtitle: String,
    pub updated_prefix: String,
    pub enabled_label: String,
    pub disabled_label: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TenantAdminErrorCopy {
    pub load_bootstrap: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TenantAdminModuleViewModel {
    pub slug: String,
    pub name: String,
    pub description: String,
    pub kind: String,
    pub source: String,
    pub enabled_label: String,
}

pub fn shell_copy(
    badge: impl Into<String>,
    title: impl Into<String>,
    subtitle: impl Into<String>,
) -> TenantAdminShellCopy {
    TenantAdminShellCopy {
        badge: badge.into(),
        title: title.into(),
        subtitle: subtitle.into(),
    }
}

pub fn info_cards(
    bootstrap: &TenantAdminBootstrap,
    copy: TenantAdminInfoCardCopy,
) -> TenantAdminInfoCards {
    TenantAdminInfoCards {
        tenant_label: copy.tenant_label,
        name_label: copy.name_label,
        domain_label: copy.domain_label,
        status_label: copy.status_label,
        domain_value: bootstrap
            .tenant
            .domain
            .clone()
            .unwrap_or(copy.not_available),
        status_value: if bootstrap.tenant.is_active {
            copy.active
        } else {
            copy.inactive
        },
    }
}

pub fn modules_copy(
    title: impl Into<String>,
    subtitle: impl Into<String>,
    updated_prefix: impl Into<String>,
    enabled_label: impl Into<String>,
    disabled_label: impl Into<String>,
) -> TenantAdminModulesCopy {
    TenantAdminModulesCopy {
        title: title.into(),
        subtitle: subtitle.into(),
        updated_prefix: updated_prefix.into(),
        enabled_label: enabled_label.into(),
        disabled_label: disabled_label.into(),
    }
}

pub fn error_copy(load_bootstrap: impl Into<String>) -> TenantAdminErrorCopy {
    TenantAdminErrorCopy {
        load_bootstrap: load_bootstrap.into(),
    }
}

pub fn module_view_model(
    module: TenantAdminModule,
    copy: &TenantAdminModulesCopy,
) -> TenantAdminModuleViewModel {
    TenantAdminModuleViewModel {
        slug: module.slug,
        name: module.name,
        description: module.description,
        kind: module.kind,
        source: module.source,
        enabled_label: if module.enabled {
            copy.enabled_label.clone()
        } else {
            copy.disabled_label.clone()
        },
    }
}

pub fn load_bootstrap_error_message(
    copy: &TenantAdminErrorCopy,
    error: impl std::fmt::Display,
) -> String {
    format!("{}: {error}", copy.load_bootstrap)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{TenantAdminBootstrap, TenantAdminModule, TenantAdminTenant};

    #[test]
    fn shell_copy_constructs_properly() {
        let copy = shell_copy("tenant", "Tenant Runtime", "Overview");
        assert_eq!(copy.badge, "tenant");
        assert_eq!(copy.title, "Tenant Runtime");
        assert_eq!(copy.subtitle, "Overview");
    }

    #[test]
    fn info_cards_handles_active_and_fallback_values() {
        let card_copy = TenantAdminInfoCardCopy {
            tenant_label: "Tenant".to_string(),
            name_label: "Name".to_string(),
            domain_label: "Domain".to_string(),
            status_label: "Status".to_string(),
            not_available: "n/a".to_string(),
            active: "active".to_string(),
            inactive: "inactive".to_string(),
        };

        let bootstrap_active = TenantAdminBootstrap {
            tenant: TenantAdminTenant {
                id: "t1".to_string(),
                slug: "acme".to_string(),
                name: "Acme Corp".to_string(),
                domain: Some("acme.com".to_string()),
                is_active: true,
                created_at: "2026-01-01T00:00:00Z".to_string(),
                updated_at: "2026-01-02T00:00:00Z".to_string(),
            },
            modules: vec![],
        };

        let cards = info_cards(&bootstrap_active, card_copy);
        assert_eq!(cards.tenant_label, "Tenant");
        assert_eq!(cards.domain_value, "acme.com");
        assert_eq!(cards.status_value, "active");

        let card_copy2 = TenantAdminInfoCardCopy {
            tenant_label: "Tenant".to_string(),
            name_label: "Name".to_string(),
            domain_label: "Domain".to_string(),
            status_label: "Status".to_string(),
            not_available: "н/д".to_string(),
            active: "активен".to_string(),
            inactive: "неактивен".to_string(),
        };

        let bootstrap_inactive = TenantAdminBootstrap {
            tenant: TenantAdminTenant {
                id: "t2".to_string(),
                slug: "beta".to_string(),
                name: "Beta".to_string(),
                domain: None,
                is_active: false,
                created_at: "2026-01-01T00:00:00Z".to_string(),
                updated_at: "2026-01-02T00:00:00Z".to_string(),
            },
            modules: vec![],
        };

        let cards2 = info_cards(&bootstrap_inactive, card_copy2);
        assert_eq!(cards2.domain_value, "н/д");
        assert_eq!(cards2.status_value, "неактивен");
    }

    #[test]
    fn module_view_model_reflects_enablement() {
        let copy = modules_copy("Modules", "Subtitle", "Updated", "enabled", "disabled");

        let mod_enabled = TenantAdminModule {
            slug: "catalog".to_string(),
            name: "Catalog".to_string(),
            description: "Catalog module".to_string(),
            kind: "core".to_string(),
            enabled: true,
            source: "builtin".to_string(),
        };
        let vm1 = module_view_model(mod_enabled, &copy);
        assert_eq!(vm1.enabled_label, "enabled");
        assert_eq!(vm1.slug, "catalog");

        let mod_disabled = TenantAdminModule {
            slug: "reviews".to_string(),
            name: "Reviews".to_string(),
            description: "Reviews module".to_string(),
            kind: "optional".to_string(),
            enabled: false,
            source: "plugin".to_string(),
        };
        let vm2 = module_view_model(mod_disabled, &copy);
        assert_eq!(vm2.enabled_label, "disabled");
    }

    #[test]
    fn error_copy_formats_message() {
        let err_copy = error_copy("Failed to load");
        assert_eq!(
            load_bootstrap_error_message(&err_copy, "network timeout"),
            "Failed to load: network timeout"
        );
    }
}
