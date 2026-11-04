use std::collections::HashSet;
use std::sync::RwLock;

use leptos::prelude::AnyView;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdminSlot {
    DashboardMetric,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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
    let registry = REGISTRY
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AdminPageLookup {
    Found(AdminPageRegistration),
    Disabled(AdminPageRegistration),
    Missing,
}

pub fn lookup_page_for_route_segment(
    route_segment: &str,
    enabled_modules: Option<&HashSet<String>>,
) -> AdminPageLookup {
    super::init_modules();
    let registry = PAGE_REGISTRY
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let page = registry
        .iter()
        .find(|page| page.route_segment == route_segment)
        .cloned();

    let Some(page) = page else {
        return AdminPageLookup::Missing;
    };

    let is_enabled = match enabled_modules {
        Some(enabled_modules) => enabled_modules.contains(page.module_slug),
        None => false,
    };

    if is_enabled {
        AdminPageLookup::Found(page)
    } else {
        AdminPageLookup::Disabled(page)
    }
}

pub fn page_for_route_segment(
    route_segment: &str,
    enabled_modules: Option<&HashSet<String>>,
) -> Option<AdminPageRegistration> {
    match lookup_page_for_route_segment(route_segment, enabled_modules) {
        AdminPageLookup::Found(page) => Some(page),
        _ => None,
    }
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
    }

    #[test]
    fn page_lookup_distinguishes_found_disabled_and_missing() {
        use std::collections::HashSet;
        use super::{lookup_page_for_route_segment, register_page, AdminPageLookup};

        register_page(AdminPageRegistration {
            module_slug: "blog_test",
            route_segment: "blog_test",
            title: "Blog Test",
            child_pages: &[],
            render: || ().into_any(),
        });

        let enabled = HashSet::from(["blog_test".to_string()]);
        assert!(matches!(
            lookup_page_for_route_segment("blog_test", Some(&enabled)),
            AdminPageLookup::Found(_)
        ));

        let other_enabled = HashSet::from(["forum".to_string()]);
        assert!(matches!(
            lookup_page_for_route_segment("blog_test", Some(&other_enabled)),
            AdminPageLookup::Disabled(_)
        ));

        // When enabled_modules is None (fail closed), module page is Disabled
        assert!(matches!(
            lookup_page_for_route_segment("blog_test", None),
            AdminPageLookup::Disabled(_)
        ));

        assert_eq!(
            lookup_page_for_route_segment("nonexistent_route_404", Some(&enabled)),
            AdminPageLookup::Missing
        );
    }

    #[test]
    fn components_for_slot_filters_by_slot_and_module_availability() {
        use std::collections::HashSet;
        use super::{components_for_slot, register_component, AdminComponentRegistration, AdminSlot};

        register_component(AdminComponentRegistration {
            id: "test_kpi",
            module_slug: Some("commerce"),
            slot: AdminSlot::DashboardMetric,
            order: 10,
            render: || ().into_any(),
        });

        register_component(AdminComponentRegistration {
            id: "test_section",
            module_slug: Some("commerce"),
            slot: AdminSlot::DashboardSection,
            order: 20,
            render: || ().into_any(),
        });

        // Fail-closed when enabled_modules is None
        assert!(components_for_slot(AdminSlot::DashboardMetric, None).is_empty());
        assert!(components_for_slot(AdminSlot::DashboardSection, None).is_empty());

        // When commerce is disabled
        let blog_only = HashSet::from(["blog".to_string()]);
        assert!(components_for_slot(AdminSlot::DashboardMetric, Some(&blog_only)).is_empty());

        // When commerce is enabled
        let commerce_enabled = HashSet::from(["commerce".to_string()]);
        let metrics = components_for_slot(AdminSlot::DashboardMetric, Some(&commerce_enabled));
        assert_eq!(metrics.len(), 1);
        assert_eq!(metrics[0].id, "test_kpi");

        let sections = components_for_slot(AdminSlot::DashboardSection, Some(&commerce_enabled));
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].id, "test_section");
    }
}
