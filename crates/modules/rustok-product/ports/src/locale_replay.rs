use std::num::NonZeroU64;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rustok_api::{PortContext, PortError};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const PRODUCT_LOCALE_REPLAY_READ_CLAIM: &str = "product:locale_replay:read";
pub const PRODUCT_LOCALE_REPLAY_FRESHNESS_PENDING: &str =
    "product_index_relation_freshness_pending";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProductLocaleReplayKey {
    pub product_id: Uuid,
    pub locale: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProductLocaleReplayScan {
    pub locale: Option<String>,
    pub after: Option<ProductLocaleReplayKey>,
    /// Maximum number of entries. The transport can return a shorter page with a continuation.
    pub limit: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductLocaleReplayPage {
    pub entries: Vec<ProductLocaleReplayEntry>,
    pub next: Option<ProductLocaleReplayKey>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductLocaleReplayEntry {
    pub tenant_id: Uuid,
    pub product_id: Uuid,
    /// Product graph projection epoch used to order Index mutations.
    pub source_version: u64,
    /// Exact Product revision (live or retained deletion) represented by this entry.
    pub product_source_version: NonZeroU64,
    pub locale: String,
    pub state: ProductLocaleReplayState,
}

impl ProductLocaleReplayEntry {
    pub fn key(&self) -> ProductLocaleReplayKey {
        ProductLocaleReplayKey {
            product_id: self.product_id,
            locale: self.locale.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProductLocaleReplayState {
    Live(ProductLocaleReplayFields),
    Deleted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductLocaleReplayFields {
    pub status: String,
    pub title: String,
    pub handle: String,
    pub description: Option<String>,
    pub seller_id: Option<String>,
    pub vendor: Option<String>,
    pub product_type: Option<String>,
    pub primary_category_id: Option<Uuid>,
    pub tag_ids: Vec<Uuid>,
    pub created_at: DateTime<Utc>,
    pub published_at: Option<DateTime<Utc>>,
    pub attribute_terms: Vec<String>,
    pub variant_ids: Vec<Uuid>,
    pub sales_channel_ids: Vec<Uuid>,
}

/// Reads localized Product projections and retained deletions for Index replay.
#[async_trait]
pub trait ProductLocaleReplayPort: Send + Sync {
    async fn scan_locale_replay(
        &self,
        context: PortContext,
        request: ProductLocaleReplayScan,
    ) -> Result<ProductLocaleReplayPage, PortError>;

    async fn load_locale_replay(
        &self,
        context: PortContext,
        keys: Vec<ProductLocaleReplayKey>,
    ) -> Result<Vec<ProductLocaleReplayEntry>, PortError>;
}
