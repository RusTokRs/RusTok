//! Host-side composition of Product selection with the current Cart route reference.

/// Selection edits own Product query fields, not Cart's `cart_id` or other host selectors.
/// Preserve the latter from the current URL even if Cart is disabled or the Product snapshot
/// predates it. Route values are not credentials. Ambiguous routes must never be rewritten.
pub(crate) fn selection_target(path: &str, href: &str, current_search: &str) -> Option<String> {
    let (target_path, query) = href.split_once('?').unwrap_or((href, ""));
    if target_path != path || href.contains('#') {
        return None;
    }
    let current = rustok_ui_core::decode_single_value_ui_query(
        current_search.strip_prefix('?').unwrap_or(current_search),
    )
    .ok()?;
    let mut target = rustok_ui_core::decode_single_value_ui_query(query).ok()?;
    // The catalog may initially select its first product without a `handle` query.
    // Preserve that resolved identity in the deep link, but never combine the old
    // Product snapshot's selection with a different handle already in the URL.
    let selected_handle = target.get("handle").cloned();
    if current
        .get("handle")
        .zip(selected_handle.as_ref())
        .is_some_and(|(current, selected)| current != selected)
    {
        return None;
    }
    let is_selection_key = |key: &str| {
        key.starts_with("axis.")
            || matches!(
                key,
                "variant_id"
                    | "quantity"
                    | "currency"
                    | "region_id"
                    | "price_list_id"
                    | "channel_id"
                    | "channel_slug"
            )
    };
    target.retain(|key, _| is_selection_key(key));
    for (key, value) in current {
        if !is_selection_key(&key) {
            target.insert(key, value);
        }
    }
    if !target.contains_key("handle") {
        if let Some(handle) = selected_handle {
            target.insert("handle".into(), handle);
        }
    }
    if target.is_empty() {
        return Some(path.to_string());
    }
    let pairs = target
        .iter()
        .map(|(key, value)| (key.as_str(), value.as_str()))
        .collect::<Vec<_>>();
    Some(format!(
        "{path}?{}",
        rustok_ui_core::encode_ui_query_pairs(&pairs)
    ))
}

#[cfg(test)]
mod tests {
    use super::selection_target;

    #[test]
    fn selection_preserves_the_current_cart_reference_not_a_stale_snapshot() {
        assert_eq!(
            selection_target(
                "/en/products",
                "/en/products?cart_id=old&quantity=2",
                "?cart_id=new&quantity=1"
            ),
            Some("/en/products?cart_id=new&quantity=2".into())
        );
        assert_eq!(
            selection_target("/en/products", "/en/products?cart_id=old", ""),
            Some("/en/products".into())
        );
        assert_eq!(
            selection_target(
                "/en/products",
                "/en/products?handle=current&cart_id=old&quantity=3",
                "?handle=current&cart_id=new&campaign=live"
            ),
            Some("/en/products?campaign=live&cart_id=new&handle=current&quantity=3".into())
        );
        // Catalog auto-selection must become explicit so the link restores the same product.
        assert_eq!(
            selection_target(
                "/en/products",
                "/en/products?handle=first&variant_id=sku&quantity=1",
                "?campaign=live"
            ),
            Some("/en/products?campaign=live&handle=first&quantity=1&variant_id=sku".into())
        );
        // Explicitly empty/invalid input must remain invalid; selection must not repair it.
        assert_eq!(
            selection_target("/en/products", "/en/products?quantity=", "?cart_id="),
            Some("/en/products?cart_id=&quantity=".into())
        );
        // Only the query delimiter is stripped: a literal `?` in a key is host-owned.
        assert_eq!(
            selection_target("/en/products", "/en/products?quantity=2", "??campaign=a"),
            Some("/en/products?%3Fcampaign=a&quantity=2".into())
        );
    }

    #[test]
    fn ambiguous_and_cross_route_selection_writes_are_refused() {
        for (href, current) in [
            ("/en/other?quantity=2", ""),
            ("/en/products?quantity=1&quantity=2", ""),
            ("/en/products?quantity=2", "?cart_id=a&cart_id=b"),
            ("/en/products?handle=old&quantity=2", "?handle=new"),
            ("/en/products?quantity=2#fragment", ""),
        ] {
            assert_eq!(selection_target("/en/products", href, current), None);
        }
    }
}
