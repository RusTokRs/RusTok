use std::collections::{BTreeMap, HashSet};

use crate::app::modules::GeneratedModuleNavigationEntry;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct NavChild {
    pub href: String,
    pub label: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ModuleNavGroup {
    pub key: &'static str,
    pub items: Vec<ModuleNavItem>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ModuleNavItem {
    pub label: String,
    pub order: usize,
    pub children: Vec<NavChild>,
}

pub(super) fn build_module_nav_groups(
    entries: &[GeneratedModuleNavigationEntry],
    enabled_modules: &HashSet<String>,
    overview_label: &str,
    settings_label: &str,
) -> Vec<ModuleNavGroup> {
    let mut grouped = BTreeMap::<&'static str, Vec<ModuleNavItem>>::new();

    for entry in entries
        .iter()
        .filter(|entry| enabled_modules.contains(entry.module_slug))
    {
        let mut children = vec![NavChild {
            href: format!("/modules/{}", entry.route_segment),
            label: overview_label.to_string(),
        }];

        children.extend(entry.child_pages.iter().map(|child| NavChild {
            href: format!("/modules/{}/{}", entry.route_segment, child.subpath),
            label: child.nav_label.to_string(),
        }));

        if entry.has_settings {
            children.push(NavChild {
                href: format!("/modules?module_slug={}", entry.module_slug),
                label: format!("{} {}", entry.nav_label, settings_label),
            });
        }

        grouped
            .entry(entry.nav_group)
            .or_default()
            .push(ModuleNavItem {
                label: entry.nav_label.to_string(),
                order: entry.nav_order,
                children,
            });
    }

    let mut groups = grouped
        .into_iter()
        .map(|(key, mut items)| {
            items.sort_by(|left, right| {
                left.order
                    .cmp(&right.order)
                    .then_with(|| left.label.cmp(&right.label))
            });
            ModuleNavGroup { key, items }
        })
        .collect::<Vec<_>>();

    groups.sort_by(|left, right| {
        module_group_order(left.key)
            .cmp(&module_group_order(right.key))
            .then_with(|| left.key.cmp(right.key))
    });
    groups
}

pub(super) fn href_is_active(path: &str, module_query: Option<&str>, href: &str) -> bool {
    if let Some(module_slug) = href.strip_prefix("/modules?module_slug=") {
        return path == "/modules" && module_query == Some(module_slug);
    }

    if href == "/dashboard" {
        return path == "/dashboard" || path == "/";
    }

    if href == "/modules" {
        return path == "/modules" && module_query.is_none();
    }

    path == href || path.starts_with(&format!("{}/", href.trim_end_matches('/')))
}

pub(super) fn module_group_order(group: &str) -> usize {
    match group {
        "Content" => 10,
        "Commerce" => 20,
        "Runtime" => 30,
        "Governance" => 40,
        "Automation" => 50,
        _ => 90,
    }
}

pub(super) fn module_group_icon(group: &str) -> &'static str {
    match group {
        "Content" => "content",
        "Commerce" => "commerce",
        "Runtime" => "runtime",
        "Governance" => "lock",
        "Automation" => "activity",
        _ => "box",
    }
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
    }
}
