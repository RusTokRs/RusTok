use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::engine::SearchEngineKind;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SearchSettingsRecord {
    pub id: Uuid,
    pub tenant_id: Option<Uuid>,
    pub active_engine: SearchEngineKind,
    pub fallback_engine: SearchEngineKind,
    pub config: serde_json::Value,
    pub updated_at: DateTime<Utc>,
}

impl SearchSettingsRecord {
    pub fn default_for_tenant(tenant_id: Option<Uuid>) -> Self {
        Self {
            id: Uuid::nil(),
            tenant_id,
            active_engine: SearchEngineKind::Postgres,
            fallback_engine: SearchEngineKind::Postgres,
            config: serde_json::json!({
                "connector_mode": "settings_driven",
                "available_external_engines": [],
                "ranking_profiles": {
                    "default": "balanced",
                    "search_preview": "balanced",
                    "storefront_search": "balanced",
                    "admin_global_search": "exact"
                },
                "filter_presets": {
                    "storefront_search": [
                        {
                            "key": "all",
                            "label": "All results"
                        },
                        {
                            "key": "products",
                            "label": "Products",
                            "entity_types": ["product"],
                            "source_modules": ["commerce"],
                            "ranking_profile": "catalog"
                        },
                        {
                            "key": "content",
                            "label": "Content",
                            "entity_types": ["node"],
                            "ranking_profile": "content"
                        }
                    ],
                    "search_preview": [
                        {
                            "key": "products",
                            "label": "Products",
                            "entity_types": ["product"],
                            "source_modules": ["commerce"],
                            "ranking_profile": "catalog"
                        },
                        {
                            "key": "content",
                            "label": "Content",
                            "entity_types": ["node"],
                            "ranking_profile": "content"
                        }
                    ]
                },
                "notes": "PostgreSQL is the default engine until optional connector crates are installed."
            }),
            updated_at: Utc::now(),
        }
    }
}
