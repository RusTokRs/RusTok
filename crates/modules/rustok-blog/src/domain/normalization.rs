/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! Canonical slug normalization for blog routes, categories, and tags.

/// Normalizes a human-facing label or route value into a canonical route key.
///
/// Non-ASCII characters are transliterated. Returns `None` if the input produces an empty slug.
pub fn normalize_term_route_key(value: &str) -> Option<String> {
    let route_key = slug::slugify(value);
    (!route_key.is_empty()).then_some(route_key)
}

#[cfg(test)]
mod tests {
    use super::normalize_term_route_key;

    #[test]
    fn test_slug_normalization() {
        assert_eq!(
            normalize_term_route_key("  Hello World  "),
            Some("hello-world".to_string())
        );
        assert_eq!(normalize_term_route_key("   "), None);
        assert_eq!(
            normalize_term_route_key("Новости дня!"),
            Some("novosti-dnia".to_string())
        );
    }
}
