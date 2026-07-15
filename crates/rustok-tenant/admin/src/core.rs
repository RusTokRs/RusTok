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
