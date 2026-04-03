use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexAdminBootstrap {
    pub tenant: IndexTenantSnapshot,
    pub module: IndexModuleSnapshot,
    pub counters: Vec<IndexCounterSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexTenantSnapshot {
    pub id: String,
    pub slug: String,
    pub name: String,
    pub default_locale: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexModuleSnapshot {
    pub slug: String,
    pub name: String,
    pub description: String,
    pub supports_postgres_fts: bool,
    pub document_types: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexCounterSnapshot {
    pub key: String,
    pub label: String,
    pub value: u64,
}
