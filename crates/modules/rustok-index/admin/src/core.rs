use crate::i18n::t;
use crate::model::IndexAdminBootstrap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexInfoCardViewModel {
    pub label: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexAdminOverviewViewModel {
    pub info_cards: Vec<IndexInfoCardViewModel>,
    pub module_description: String,
}

pub fn build_index_admin_overview_view_model(
    locale: Option<&str>,
    bootstrap: IndexAdminBootstrap,
) -> IndexAdminOverviewViewModel {
    IndexAdminOverviewViewModel {
        info_cards: vec![
            IndexInfoCardViewModel {
                label: t(locale, "index.info.tenant", "Tenant"),
                value: bootstrap.tenant.slug,
            },
            IndexInfoCardViewModel {
                label: t(locale, "index.info.locale", "Locale"),
                value: bootstrap.tenant.default_locale,
            },
            IndexInfoCardViewModel {
                label: t(locale, "index.info.rewriteStatus", "Rewrite status"),
                value: bootstrap.module.rewrite_status,
            },
            IndexInfoCardViewModel {
                label: t(locale, "index.info.currentMilestone", "Current milestone"),
                value: bootstrap.module.current_milestone,
            },
        ],
        module_description: bootstrap.module.description,
    }
}

pub fn format_index_admin_bootstrap_error(
    locale: Option<&str>,
    error: impl std::fmt::Display,
) -> String {
    format!(
        "{}: {error}",
        t(
            locale,
            "index.error.loadBootstrap",
            "Failed to load index bootstrap"
        )
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{IndexAdminBootstrap, IndexModuleSnapshot, IndexTenantSnapshot};

    #[test]
    fn overview_view_model_formats_rewrite_state_without_framework_runtime() {
        let view_model = build_index_admin_overview_view_model(
            Some("en"),
            IndexAdminBootstrap {
                tenant: IndexTenantSnapshot {
                    id: "tenant-1".to_string(),
                    slug: "acme".to_string(),
                    name: "Acme".to_string(),
                    default_locale: "en".to_string(),
                },
                module: IndexModuleSnapshot {
                    slug: "index".to_string(),
                    name: "Index".to_string(),
                    description: "Cross-module relational index and query engine.".to_string(),
                    rewrite_status: "in_progress".to_string(),
                    current_milestone: "M0/M1".to_string(),
                },
            },
        );

        assert_eq!(view_model.info_cards.len(), 4);
        assert_eq!(view_model.info_cards[0].value, "acme");
        assert_eq!(view_model.info_cards[2].value, "in_progress");
        assert_eq!(view_model.info_cards[3].value, "M0/M1");
        assert_eq!(
            view_model.module_description,
            "Cross-module relational index and query engine."
        );
    }
}
