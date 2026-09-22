/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use rustok_ui_core::badges::{UiBadgeTone, status_badge_class, status_badge_tone};
use rustok_ui_core::money::format_ui_price;
use rustok_ui_core::pagination::UiPaginationState;
use serde::{Deserialize, Serialize};

/// Framework-neutral view-model consumed by the Dioxus adapter.
/// This matches the pattern in `module/src/core.rs`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DioxusProductCardViewModel {
    pub id: String,
    pub title: String,
    pub status: String,
    pub price_cents: i64,
    pub currency: String,
}

impl DioxusProductCardViewModel {
    pub fn badge_tone(&self) -> UiBadgeTone {
        status_badge_tone(&self.status)
    }

    pub fn badge_class(&self) -> &'static str {
        status_badge_class(&self.status)
    }

    pub fn formatted_price(&self) -> String {
        format_ui_price(self.price_cents, &self.currency)
    }
}

/// Demonstration adapter converting a list of view-models into a paginated collection
/// using shared `UiPaginationState`.
pub fn paginate_dioxus_items<T: Clone>(items: &[T], pagination: &UiPaginationState) -> Vec<T> {
    let offset = pagination.offset() as usize;
    let limit = pagination.per_page as usize;
    items.iter().skip(offset).take(limit).cloned().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dioxus_product_card_view_model() {
        let card = DioxusProductCardViewModel {
            id: "prod-1".to_string(),
            title: "Premium Widget".to_string(),
            status: "ACTIVE".to_string(),
            price_cents: 4999,
            currency: "USD".to_string(),
        };

        assert_eq!(card.badge_tone(), UiBadgeTone::Success);
        assert!(card.badge_class().contains("emerald"));
        assert_eq!(card.formatted_price(), "49.99 USD");
    }

    #[test]
    fn test_paginate_dioxus_items() {
        let items = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        let pagination = UiPaginationState::new(2, 3, Some(10));
        let page = paginate_dioxus_items(&items, &pagination);
        assert_eq!(page, vec![4, 5, 6]);
    }
}
