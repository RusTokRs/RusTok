/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! Storefront transport contracts and action payloads for dynamic page rendering.

use serde::{Deserialize, Serialize};

/// Storefront page fetch request payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FetchStorefrontPagePayload {
    pub tenant_id: String,
    pub path: String,
    pub locale: Option<String>,
}

/// Storefront page transport status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PageBuilderTransportStatus {
    Loading,
    Ready,
    NotFound,
    Error(String),
}
