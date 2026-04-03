use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutboxAdminBootstrap {
    pub tenant_slug: Option<String>,
    pub health: String,
    pub counters: Vec<OutboxCounterSnapshot>,
    pub relay_notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutboxCounterSnapshot {
    pub key: String,
    pub label: String,
    pub value: u64,
}
