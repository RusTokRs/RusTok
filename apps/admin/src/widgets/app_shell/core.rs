use std::collections::HashSet;

use crate::app::modules::GeneratedModuleNavigationEntry;
pub(super) use rustok_ui_core::navigation::{
    UiChildPageEntry, UiNavChild as NavChild, UiNavGroup as ModuleNavGroup, UiNavigationEntry,
    build_ui_nav_groups, ui_href_is_active, ui_module_group_icon,
};

pub(super) fn build_module_nav_groups(
    entries: &[GeneratedModuleNavigationEntry],
    enabled_modules: &HashSet<String>,
    overview_label: &str,
    settings_label: &str,
) -> Vec<ModuleNavGroup> {
    let ffa_entries: Vec<UiNavigationEntry> = entries
        .iter()
        .map(|entry| UiNavigationEntry {
            module_slug: entry.module_slug.to_string(),
            route_segment: entry.route_segment.to_string(),
            nav_label: entry.nav_label.to_string(),
            nav_group: entry.nav_group,
            nav_order: entry.nav_order,
            has_settings: entry.has_settings,
            child_pages: entry
                .child_pages
                .iter()
                .map(|child| UiChildPageEntry {
                    subpath: child.subpath,
                    nav_label: child.nav_label,
                })
                .collect(),
        })
        .collect();

    build_ui_nav_groups(
        &ffa_entries,
        enabled_modules,
        overview_label,
        settings_label,
    )
}

pub(super) fn href_is_active(path: &str, module_query: Option<&str>, href: &str) -> bool {
    ui_href_is_active(path, module_query, href)
}

#[cfg(test)]
pub(super) fn module_group_order(group: &str) -> usize {
    rustok_ui_core::navigation::ui_module_group_order(group)
}

pub(super) fn module_group_icon(group: &str) -> &'static str {
    ui_module_group_icon(group)
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use crate::app::modules::{AdminChildPageRegistration, GeneratedModuleNavigationEntry};

    use super::{build_module_nav_groups, href_is_active, module_group_icon};

    #[test]
    fn active_href_policy_handles_dashboard_modules_and_nested_routes() {
        assert!(href_is_active("/", None, "/dashboard"));
        assert!(href_is_active("/dashboard", None, "/dashboard"));
        assert!(href_is_active("/modules", None, "/modules"));
        assert!(!href_is_active("/modules", Some("blog"), "/modules"));
        assert!(href_is_active(
            "/modules",
            Some("blog"),
            "/modules?module_slug=blog",
        ));
        assert!(href_is_active("/modules/blog/posts", None, "/modules/blog",));
        assert!(!href_is_active("/modules/blogger", None, "/modules/blog"));
    }

    #[test]
    fn module_nav_groups_filter_sort_and_append_settings_link() {
        static BLOG_CHILDREN: &[AdminChildPageRegistration] = &[AdminChildPageRegistration {
            subpath: "posts",
            title: "Posts",
            nav_label: "Posts",
        }];
        static ENTRIES: &[GeneratedModuleNavigationEntry] = &[
            GeneratedModuleNavigationEntry {
                module_slug: "pricing",
                route_segment: "pricing",
                nav_label: "Pricing",
                nav_group: "Commerce",
                nav_order: 20,
                has_settings: false,
                child_pages: &[],
            },
            GeneratedModuleNavigationEntry {
                module_slug: "blog",
                route_segment: "blog",
                nav_label: "Blog",
                nav_group: "Content",
                nav_order: 10,
                has_settings: true,
                child_pages: BLOG_CHILDREN,
            },
            GeneratedModuleNavigationEntry {
                module_slug: "workflow",
                route_segment: "workflow",
                nav_label: "Workflow",
                nav_group: "Automation",
                nav_order: 30,
                has_settings: false,
                child_pages: &[],
            },
        ];

        let enabled = HashSet::from(["pricing".to_string(), "blog".to_string()]);
        let groups = build_module_nav_groups(ENTRIES, &enabled, "Overview", "Settings");

        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].key, "Content");
        assert_eq!(groups[0].items[0].label, "Blog");
        assert_eq!(
            groups[0].items[0]
                .children
                .iter()
                .map(|child| child.href.as_str())
                .collect::<Vec<_>>(),
            vec![
                "/modules/blog",
                "/modules/blog/posts",
                "/modules?module_slug=blog",
            ],
        );
        assert_eq!(groups[1].key, "Commerce");
        assert_eq!(module_group_icon(groups[1].key), "commerce");
        assert_eq!(super::module_group_order(groups[0].key), 10);
        let first_item = &groups[0].items[0];
        assert_eq!(first_item.label, "Blog");
    }
}
