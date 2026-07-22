use std::collections::HashSet;
use std::sync::{OnceLock, RwLock};

use leptos::prelude::AnyView;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StorefrontSlot {
    HeaderNavigation,
    HomeAfterHero,
    HomeAfterCatalog,
    HomeBeforeFooter,
    FooterNavigation,
    CheckoutPaymentHandoff,
    CheckoutResultHandoff,
    CheckoutShippingHandoff,
}

#[derive(Clone)]
pub struct StorefrontComponentRegistration {
    pub id: &'static str,
    pub module_slug: Option<&'static str>,
    pub slot: StorefrontSlot,
    pub order: usize,
    pub render: fn() -> AnyView,
}

#[derive(Clone)]
pub struct StorefrontPageRegistration {
    pub module_slug: &'static str,
    pub route_segment: &'static str,
    pub title: &'static str,
    pub render: fn() -> AnyView,
}

static REGISTRY: OnceLock<RwLock<Vec<StorefrontComponentRegistration>>> = OnceLock::new();
static PAGE_REGISTRY: OnceLock<RwLock<Vec<StorefrontPageRegistration>>> = OnceLock::new();

fn registry() -> &'static RwLock<Vec<StorefrontComponentRegistration>> {
    REGISTRY.get_or_init(|| RwLock::new(Vec::new()))
}

fn page_registry() -> &'static RwLock<Vec<StorefrontPageRegistration>> {
    PAGE_REGISTRY.get_or_init(|| RwLock::new(Vec::new()))
}

pub fn register_component(component: StorefrontComponentRegistration) {
    let mut components = registry().write().expect("storefront module registry lock");
    components.push(component);
}

pub fn register_page(page: StorefrontPageRegistration) {
    let mut pages = page_registry()
        .write()
        .expect("storefront page registry lock");
    pages.push(page);
}

pub fn components_for_slot(
    slot: StorefrontSlot,
    enabled_modules: Option<&HashSet<String>>,
) -> Vec<StorefrontComponentRegistration> {
    let components = registry()
        .read()
        .expect("storefront module registry lock")
        .iter()
        .filter(|component| component.slot == slot)
        .filter(|component| match (component.module_slug, enabled_modules) {
            (Some(module_slug), Some(enabled)) => enabled.contains(module_slug),
            _ => true,
        })
        .cloned()
        .collect::<Vec<_>>();

    let mut sorted = components;
    sorted.sort_by(|left, right| {
        left.order
            .cmp(&right.order)
            .then_with(|| left.id.cmp(right.id))
    });
    sorted
}

pub enum StorefrontPageLookup {
    Found(StorefrontPageRegistration),
    Disabled,
    Missing,
}

pub fn page_for_route_segment(
    route_segment: &str,
    enabled_modules: Option<&HashSet<String>>,
) -> StorefrontPageLookup {
    let page = page_registry()
        .read()
        .expect("storefront page registry lock")
        .iter()
        .find(|page| page.route_segment == route_segment)
        .cloned();

    let Some(page) = page else {
        return StorefrontPageLookup::Missing;
    };

    if enabled_modules
        .map(|enabled| enabled.contains(page.module_slug))
        .unwrap_or(true)
    {
        StorefrontPageLookup::Found(page)
    } else {
        StorefrontPageLookup::Disabled
    }
}

#[cfg(test)]
mod tests {
    use super::{
        StorefrontPageLookup, StorefrontPageRegistration, page_for_route_segment, register_page,
    };
    use leptos::prelude::*;
    use std::collections::HashSet;

    fn render_stub() -> leptos::prelude::AnyView {
        leptos::view! { <div>"stub"</div> }.into_any()
    }

    #[test]
    fn page_lookup_distinguishes_found_disabled_and_missing() {
        register_page(StorefrontPageRegistration {
            module_slug: "blog",
            route_segment: "blog",
            title: "Blog",
            render: render_stub,
        });

        let enabled = HashSet::from(["blog".to_string()]);
        assert!(matches!(
            page_for_route_segment("blog", Some(&enabled)),
            StorefrontPageLookup::Found(_)
        ));

        let disabled = HashSet::from(["forum".to_string()]);
        assert!(matches!(
            page_for_route_segment("blog", Some(&disabled)),
            StorefrontPageLookup::Disabled
        ));

        assert!(matches!(
            page_for_route_segment("missing", Some(&enabled)),
            StorefrontPageLookup::Missing
        ));
    }
}
