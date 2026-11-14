//! Host projection updates for independently mounted client islands.

#[cfg(any(test, all(target_arch = "wasm32", feature = "product-buyer-island")))]
use std::collections::BTreeSet;
use std::collections::HashSet;

#[cfg(target_arch = "wasm32")]
pub(crate) const MODULES_CHANGED: &str = "rustok:storefront:enabled-modules";

/// A malformed or missing host projection never grants client Cart capability.
#[cfg(any(test, all(target_arch = "wasm32", feature = "product-buyer-island")))]
pub(crate) fn cart_available(content: &str) -> bool {
    if content.len() > 16 * 1024 {
        return false;
    }
    let mut modules = BTreeSet::new();
    for slug in content.split(',') {
        if !rustok_api::is_valid_module_slug(slug) || !modules.insert(slug) {
            return false;
        }
    }
    modules.contains("cart") && modules.contains("commerce")
}

/// Called synchronously by the host context when its effective module snapshot is replaced.
/// The public list contains no credentials; server-side Cart admission is still authoritative.
pub(crate) fn publish(modules: &HashSet<String>) {
    #[cfg(target_arch = "wasm32")]
    {
        use leptos::wasm_bindgen::JsValue;
        let Some(window) = web_sys::window() else {
            return;
        };
        let mut modules = modules.iter().map(String::as_str).collect::<Vec<_>>();
        modules.sort_unstable();
        let value = modules.join(",");
        let stored = window
            .document()
            .and_then(|document| {
                document
                    .query_selector("meta[name='rustok-enabled-modules']")
                    .ok()
                    .flatten()
            })
            .is_some_and(|meta| meta.set_attribute("content", &value).is_ok());
        let init = web_sys::CustomEventInit::new();
        init.set_detail(&JsValue::from_str(if stored { &value } else { "" }));
        if let Ok(event) = web_sys::CustomEvent::new_with_event_init_dict(MODULES_CHANGED, &init) {
            let _ = window.dispatch_event(&event);
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    let _ = modules;
}

#[cfg(test)]
mod tests {
    use super::cart_available;

    #[test]
    fn cart_capability_needs_a_well_formed_host_projection_and_both_owners() {
        assert!(cart_available("cart,commerce,product"));
        assert!(!cart_available("cart,product"));
        assert!(!cart_available("commerce,product"));
        assert!(!cart_available("cart,commerce,cart"));
        assert!(!cart_available("cart,commerce,?"));
        assert!(!cart_available(""));
    }
}
