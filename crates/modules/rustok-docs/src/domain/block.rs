use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 4 Polymorphic Display Modes for Universal Entity References (UER).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityDisplayMode {
    /// Inline pill / @mention in running text with popover preview.
    InlineMention,
    /// Compact preview bookmark card (title, excerpt, thumbnail, badge).
    CompactCard,
    /// Fully functional interactive UI widget with real-time state.
    InteractiveWidget,
    /// Structured database relation cell with rollup capability.
    DatabaseRelation,
}

impl Default for EntityDisplayMode {
    fn default() -> Self {
        Self::CompactCard
    }
}

/// Canonical Uniform Entity Reference (UER) protocol for cross-module embedding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntityRef {
    pub tenant_id: Uuid,
    pub module: String,
    pub entity_type: String,
    pub entity_id: Uuid,
    pub display_mode: EntityDisplayMode,
}

impl EntityRef {
    /// Formats the reference as a standard RusToK URN.
    pub fn to_urn(&self) -> String {
        format!(
            "urn:rustok:{}:{}:{}:{}",
            self.tenant_id, self.module, self.entity_type, self.entity_id
        )
    }
}

/// 5 Polymorphic Views for Embedded Notion-like Databases.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DatabaseViewType {
    Table,
    Kanban,
    List,
    Calendar,
    Gallery,
}

impl Default for DatabaseViewType {
    fn default() -> Self {
        Self::Table
    }
}

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
    /// Universal Entity Reference (Media, Blog, Forum, Wiki/Pages, Commerce, ERP).
    EntityEmbed(EntityRef),
    /// Dynamic Notion-like Database View (Table, Kanban, List, Calendar, Gallery).
    DatabaseView {
        view_type: DatabaseViewType,
        database_id: Option<Uuid>,
    },
    /// Live embedded ERP / Commerce object (legacy alias, preserved for backward-compatibility).
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
