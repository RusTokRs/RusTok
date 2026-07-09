use std::fmt;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Document type stored in the index.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DocumentType {
    Node,
    Product,
    Category,
}

impl fmt::Display for DocumentType {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::Node => "node",
            Self::Product => "product",
            Self::Category => "category",
        };
        formatter.write_str(value)
    }
}

/// Canonical document stored for indexing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexDocument {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub doc_type: DocumentType,
    pub locale: String,

    // Search fields.
    pub title: String,
    pub slug: String,
    pub content: Option<String>,
    pub keywords: Vec<String>,

    // Sorting and filtering fields.
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub published_at: Option<DateTime<Utc>>,
    pub status: String,
    pub price: Option<i64>,

    // Full source payload.
    pub payload: serde_json::Value,
}
