/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! Transport facade for WebRTC Calls & Conferencing storefront.

#[derive(Clone, Debug, Default)]
pub struct CallsStorefrontTransport;

impl CallsStorefrontTransport {
    pub fn new() -> Self {
        Self
    }
}
