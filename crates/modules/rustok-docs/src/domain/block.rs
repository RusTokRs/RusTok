use serde::{Deserialize, Serialize};

/// Supported atomic block kinds in a collaborative document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BlockType {
    Paragraph,
    Heading1,
    Heading2,
    Heading3,
    BulletList,
    NumberedList,
    TodoList { checked: bool },
    Quote,
    Callout { icon: String },
    Code { language: String },
    Table,
    Divider,
    /// Live embedded ERP / Commerce object (product, inventory, invoice, deal).
    LiveErp {
        entity_type: String,
        entity_id: String,
    },
}

impl Default for BlockType {
    fn default() -> Self {
        Self::Paragraph
    }
}

/// Normalized projection of an individual document block.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DocBlock {
    pub id: String,
    pub parent_id: Option<String>,
    pub block_type: BlockType,
    pub content: String,
    pub properties: serde_json::Value,
    pub children: Vec<String>,
}
