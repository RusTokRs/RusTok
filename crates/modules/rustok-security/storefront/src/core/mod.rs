/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! Pure Rust view-models and input validation for Security storefront UI.

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SecurityStorefrontViewModel {
    pub title: String,
}

impl SecurityStorefrontViewModel {
    pub fn new(title: String) -> Self {
        Self { title }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_storefront_view_model() {
        let vm = SecurityStorefrontViewModel::new("Sample Title".to_string());
        assert_eq!(vm.title, "Sample Title");
    }
}
