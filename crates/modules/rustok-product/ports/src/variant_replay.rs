/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use async_trait::async_trait;
use rustok_api::{PortContext, PortError};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const PRODUCT_VARIANT_REPLAY_READ_CLAIM: &str = "product:variant_replay:read";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProductVariantReplayScan {
    pub after_variant_id: Option<Uuid>,
    pub limit: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProductVariantReplayPage {
    pub entries: Vec<ProductVariantReplayEntry>,
    pub next_after_variant_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProductVariantReplayEntry {
    pub tenant_id: Uuid,
    pub variant_id: Uuid,
    pub source_version: u64,
    pub state: ProductVariantReplayState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProductVariantReplayState {
    Live(ProductVariantReplayFields),
    Deleted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProductVariantReplayFields {
    pub product_id: Uuid,
    pub sku: Option<String>,
    pub barcode: Option<String>,
    pub shipping_profile_slug: Option<String>,
    pub ean: Option<String>,
    pub upc: Option<String>,
    pub inventory_policy: String,
    pub inventory_management: String,
    pub inventory_quantity: i32,
    pub weight_unit: Option<String>,
    pub combination_identity: Option<String>,
    pub position: i32,
}

/// Reads Product-owned revisions and retained deletion identities for Index replay.
#[async_trait]
pub trait ProductVariantReplayPort: Send + Sync {
    async fn scan_variant_replay(
        &self,
        context: PortContext,
        request: ProductVariantReplayScan,
    ) -> Result<ProductVariantReplayPage, PortError>;

    async fn load_variant_replay(
        &self,
        context: PortContext,
        variant_ids: Vec<Uuid>,
    ) -> Result<Vec<ProductVariantReplayEntry>, PortError>;
}
