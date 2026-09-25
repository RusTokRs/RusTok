use std::collections::HashSet;
use std::sync::RwLock;

use leptos::prelude::AnyView;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdminSlot {
    DashboardSection,
    NavItem,
}

#[derive(Clone)]
pub struct AdminComponentRegistration {
    pub id: &'static str,
    pub module_slug: Option<&'static str>,
    pub slot: AdminSlot,
    pub order: usize,
    pub render: fn() -> AnyView,
}

#[derive(Clone)]
pub struct AdminChildPageRegistration {
    pub subpath: &'static str,
    pub title: &'static str,
    pub nav_label: &'static str,
}

#[derive(Clone)]
pub struct AdminPageRegistration {
    pub module_slug: &'static str,
    pub route_segment: &'static str,
    pub title: &'static str,
    pub child_pages: &'static [AdminChildPageRegistration],
    pub render: fn() -> AnyView,
}

static REGISTRY: RwLock<Vec<AdminComponentRegistration>> = RwLock::new(Vec::new());
static PAGE_REGISTRY: RwLock<Vec<AdminPageRegistration>> = RwLock::new(Vec::new());

pub fn register_component(component: AdminComponentRegistration) {
    if let Ok(mut registry) = REGISTRY.write() {
        registry.push(component);
    }
}

pub fn register_page(page: AdminPageRegistration) {
    if let Ok(mut registry) = PAGE_REGISTRY.write() {
        registry.push(page);
    }
}

pub fn components_for_slot(
    slot: AdminSlot,
    enabled_modules: Option<&HashSet<String>>,
) -> Vec<AdminComponentRegistration> {
    super::init_modules();
    let registry = REGISTRY.read().unwrap_or_else(std::sync::PoisonError::into_inner);
    let mut components = registry
        .iter()
        .filter(|component| component.slot == slot)
        .filter(|component| match (component.module_slug, enabled_modules) {
            (Some(module_slug), Some(enabled_modules)) => enabled_modules.contains(module_slug),
            (Some(_), None) => false,
            (None, _) => true,
        })
        .cloned()
        .collect::<Vec<_>>();
    drop(registry);

    components.sort_by(|left, right| {
        left.order
            .cmp(&right.order)
            .then_with(|| left.id.cmp(right.id))
    });
    components
}

pub fn page_for_route_segment(
    route_segment: &str,
    enabled_modules: Option<&HashSet<String>>,
) -> Option<AdminPageRegistration> {
    super::init_modules();
    let registry = PAGE_REGISTRY.read().unwrap_or_else(std::sync::PoisonError::into_inner);
    registry
        .iter()
        .find(|page| {
            page.route_segment == route_segment
                && match enabled_modules {
                    Some(enabled_modules) => enabled_modules.contains(page.module_slug),
                    None => true,
                }
        })
        .cloned()
}

impl AdminPageRegistration {
    pub fn child_page_for_subpath(&self, subpath: &str) -> Option<AdminChildPageRegistration> {
        let normalized = subpath.trim_matches('/');
        if normalized.is_empty() {
            return None;
        }

        self.child_pages
            .iter()
            .find(|child| {
                normalized == child.subpath
                    || normalized.starts_with(&format!("{}/", child.subpath))
            })
            .cloned()
    }
}

#[cfg(test)]
mod tests {
    use leptos::prelude::IntoAny;

    use super::{AdminChildPageRegistration, AdminPageRegistration};

    #[test]
    fn child_page_resolution_matches_exact_and_nested_paths() {
        let registration = AdminPageRegistration {
            module_slug: "workflow",
            route_segment: "workflow",
            title: "Workflow",
            child_pages: &[
                AdminChildPageRegistration {
                    subpath: "templates",
                    title: "Workflow Templates",
                    nav_label: "Templates",
                },
                AdminChildPageRegistration {
                    subpath: "history/runs",
                    title: "Execution Runs",
                    nav_label: "Runs",
                },
            ],
            render: || ().into_any(),
        };

        assert_eq!(
            registration
                .child_page_for_subpath("templates")
                .map(|page| page.title),
            Some("Workflow Templates")
        );
        assert_eq!(
            registration
                .child_page_for_subpath("history/runs/2026")
                .map(|page| page.nav_label),
            Some("Runs")
        );
        assert!(registration.child_page_for_subpath("").is_none());
        assert!(registration.child_page_for_subpath("unknown").is_none());
    }
}
