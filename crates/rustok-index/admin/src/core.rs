use crate::i18n::t;
use crate::model::IndexAdminBootstrap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexInfoCardViewModel {
    pub label: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexCounterCardViewModel {
    pub label: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexAdminOverviewViewModel {
    pub tenant_cards: Vec<IndexInfoCardViewModel>,
    pub counter_cards: Vec<IndexCounterCardViewModel>,
    pub module_description: String,
}

pub fn build_index_admin_overview_view_model(
    locale: Option<&str>,
    bootstrap: IndexAdminBootstrap,
) -> IndexAdminOverviewViewModel {
    let backend = if bootstrap.module.supports_postgres_fts {
        t(locale, "index.value.postgres", "postgres")
    } else {
        t(locale, "index.value.generic", "generic")
    };

    IndexAdminOverviewViewModel {
        tenant_cards: vec![
            IndexInfoCardViewModel {
                label: t(locale, "index.info.tenant", "Tenant"),
                value: bootstrap.tenant.slug,
            },
            IndexInfoCardViewModel {
                label: t(locale, "index.info.locale", "Locale"),
                value: bootstrap.tenant.default_locale,
            },
            IndexInfoCardViewModel {
                label: t(locale, "index.info.backend", "FTS backend"),
                value: backend,
            },
            IndexInfoCardViewModel {
                label: t(locale, "index.info.documentTypes", "Document types"),
                value: bootstrap.module.document_types.join(", "),
            },
        ],
        counter_cards: bootstrap
            .counters
            .into_iter()
            .map(|counter| IndexCounterCardViewModel {
                label: counter.label,
                value: counter.value.to_string(),
            })
            .collect(),
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
    use crate::model::{
        IndexAdminBootstrap, IndexCounterSnapshot, IndexModuleSnapshot, IndexTenantSnapshot,
    };

    #[test]
    fn overview_view_model_formats_bootstrap_without_framework_runtime() {
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
                    description: "Read-model substrate".to_string(),
                    supports_postgres_fts: true,
                    document_types: vec!["node".to_string(), "product".to_string()],
                },
                counters: vec![IndexCounterSnapshot {
                    key: "content".to_string(),
                    label: "Content index rows".to_string(),
                    value: 42,
                }],
            },
        );

        assert_eq!(view_model.tenant_cards.len(), 4);
        assert_eq!(view_model.tenant_cards[0].value, "acme");
        assert_eq!(view_model.tenant_cards[2].value, "postgres");
        assert_eq!(view_model.tenant_cards[3].value, "node, product");
        assert_eq!(view_model.counter_cards[0].value, "42");
        assert_eq!(view_model.module_description, "Read-model substrate");
    }
}
