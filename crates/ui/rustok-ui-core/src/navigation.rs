/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! Framework-agnostic navigation structures and algorithms for host shells.
//!
//! Provides models for navigation items, groups, child pages, breadcrumbs,
//! active link checking, and group icon/order resolutions.

use std::collections::{BTreeMap, HashSet};

/// Framework-agnostic child navigation link.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UiNavChild {
    pub href: String,
    pub label: String,
}

/// Navigation item inside a group, containing subpath links.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UiNavItem {
    pub label: String,
    pub order: usize,
    pub children: Vec<UiNavChild>,
}

/// Logical grouping of navigation items in a sidebar or drawer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UiNavGroup {
    pub key: &'static str,
    pub items: Vec<UiNavItem>,
}

/// Static descriptor of a child page registered by a module.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UiChildPageEntry {
    pub subpath: &'static str,
    pub nav_label: &'static str,
}

/// Descriptor of a module's navigation contribution to the host shell.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UiNavigationEntry {
    pub module_slug: String,
    pub route_segment: String,
    pub nav_label: String,
    pub nav_group: &'static str,
    pub nav_order: usize,
    pub has_settings: bool,
    pub child_pages: Vec<UiChildPageEntry>,
}

/// Single breadcrumb segment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UiBreadcrumb {
    pub label: String,
    pub href: Option<String>,
    pub is_current: bool,
}

/// Builds sorted, filtered navigation groups from module navigation descriptors.
pub fn build_ui_nav_groups(
    entries: &[UiNavigationEntry],
    enabled_modules: &HashSet<String>,
    overview_label: &str,
    settings_label: &str,
) -> Vec<UiNavGroup> {
    let mut grouped = BTreeMap::<&'static str, Vec<UiNavItem>>::new();

    for entry in entries
        .iter()
        .filter(|entry| enabled_modules.contains(&entry.module_slug))
    {
        let mut children = vec![UiNavChild {
            href: format!("/modules/{}", entry.route_segment),
            label: overview_label.to_string(),
        }];

        children.extend(entry.child_pages.iter().map(|child| UiNavChild {
            href: format!("/modules/{}/{}", entry.route_segment, child.subpath),
            label: child.nav_label.to_string(),
        }));

        if entry.has_settings {
            children.push(UiNavChild {
                href: format!("/modules?module_slug={}", entry.module_slug),
                label: format!("{} {}", entry.nav_label, settings_label),
            });
        }

        grouped
            .entry(entry.nav_group)
            .or_default()
            .push(UiNavItem {
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
            UiNavGroup { key, items }
        })
        .collect::<Vec<_>>();

    groups.sort_by(|left, right| {
        ui_module_group_order(left.key)
            .cmp(&ui_module_group_order(right.key))
            .then_with(|| left.key.cmp(right.key))
    });
    groups
}

/// Checks whether a given navigation `href` matches the current `path` and query parameter.
pub fn ui_href_is_active(path: &str, module_query: Option<&str>, href: &str) -> bool {
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

/// Canonical sorting order for module navigation groups.
pub fn ui_module_group_order(group: &str) -> usize {
    match group {
        "Content" => 10,
        "Commerce" => 20,
        "Runtime" => 30,
        "Governance" => 40,
        "Automation" => 50,
        _ => 90,
    }
}

/// Canonical SVG icon name or identifier for module navigation groups.
pub fn ui_module_group_icon(group: &str) -> &'static str {
    match group {
        "Content" => "content",
        "Commerce" => "commerce",
        "Runtime" => "runtime",
        "Governance" => "lock",
        "Automation" => "activity",
        _ => "box",
    }
}

/// Generates a breadcrumb trail for a given current path and active navigation groups.
pub fn build_ui_breadcrumbs(
    path: &str,
    nav_groups: &[UiNavGroup],
    home_label: &str,
) -> Vec<UiBreadcrumb> {
    let mut crumbs = vec![UiBreadcrumb {
        label: home_label.to_string(),
        href: Some("/dashboard".to_string()),
        is_current: path == "/" || path == "/dashboard",
    }];

    if path == "/" || path == "/dashboard" {
        return crumbs;
    }

    // Check if the path directly matches a child link in any group
    for group in nav_groups {
        for item in &group.items {
            for child in &item.children {
                if child.href == path {
                    crumbs.push(UiBreadcrumb {
                        label: item.label.clone(),
                        href: item.children.first().map(|c| c.href.clone()),
                        is_current: false,
                    });
                    crumbs.push(UiBreadcrumb {
                        label: child.label.clone(),
                        href: Some(child.href.clone()),
                        is_current: true,
                    });
                    return crumbs;
                }
            }
        }
    }

    // Segment-based fallback
    let segments: Vec<&str> = path.trim_matches('/').split('/').collect();
    let mut current_href = String::new();
    for (i, seg) in segments.iter().enumerate() {
        current_href.push('/');
        current_href.push_str(seg);
        let is_last = i == segments.len() - 1;
        let capitalized = seg
            .chars()
            .next()
            .map(|first| first.to_uppercase().collect::<String>() + &seg[1..])
            .unwrap_or_default();

        crumbs.push(UiBreadcrumb {
            label: capitalized,
            href: if is_last { None } else { Some(current_href.clone()) },
            is_current: is_last,
        });
    }

    crumbs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ui_href_is_active() {
        assert!(ui_href_is_active("/", None, "/dashboard"));
        assert!(ui_href_is_active("/dashboard", None, "/dashboard"));
        assert!(ui_href_is_active("/modules", None, "/modules"));
        assert!(!ui_href_is_active("/modules", Some("blog"), "/modules"));
        assert!(ui_href_is_active(
            "/modules",
            Some("blog"),
            "/modules?module_slug=blog",
        ));
        assert!(ui_href_is_active(
            "/modules/blog/posts",
            None,
            "/modules/blog"
        ));
        assert!(!ui_href_is_active("/modules/blogger", None, "/modules/blog"));
    }

    #[test]
    fn test_build_ui_nav_groups() {
        let entries = vec![
            UiNavigationEntry {
                module_slug: "pricing".to_string(),
                route_segment: "pricing".to_string(),
                nav_label: "Pricing".to_string(),
                nav_group: "Commerce",
                nav_order: 20,
                has_settings: false,
                child_pages: vec![],
            },
            UiNavigationEntry {
                module_slug: "blog".to_string(),
                route_segment: "blog".to_string(),
                nav_label: "Blog".to_string(),
                nav_group: "Content",
                nav_order: 10,
                has_settings: true,
                child_pages: vec![UiChildPageEntry {
                    subpath: "posts",
                    nav_label: "Posts",
                }],
            },
            UiNavigationEntry {
                module_slug: "workflow".to_string(),
                route_segment: "workflow".to_string(),
                nav_label: "Workflow".to_string(),
                nav_group: "Automation",
                nav_order: 30,
                has_settings: false,
                child_pages: vec![],
            },
        ];

        let enabled = HashSet::from(["pricing".to_string(), "blog".to_string()]);
        let groups = build_ui_nav_groups(&entries, &enabled, "Overview", "Settings");

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
        assert_eq!(ui_module_group_icon(groups[1].key), "commerce");
    }

    #[test]
    fn test_build_ui_breadcrumbs() {
        let groups = vec![UiNavGroup {
            key: "Content",
            items: vec![UiNavItem {
                label: "Blog".to_string(),
                order: 10,
                children: vec![
                    UiNavChild {
                        href: "/modules/blog".to_string(),
                        label: "Overview".to_string(),
                    },
                    UiNavChild {
                        href: "/modules/blog/posts".to_string(),
                        label: "Posts".to_string(),
                    },
                ],
            }],
        }];

        let home_crumbs = build_ui_breadcrumbs("/dashboard", &groups, "Home");
        assert_eq!(home_crumbs.len(), 1);
        assert!(home_crumbs[0].is_current);

        let post_crumbs = build_ui_breadcrumbs("/modules/blog/posts", &groups, "Home");
        assert_eq!(post_crumbs.len(), 3);
        assert_eq!(post_crumbs[1].label, "Blog");
        assert_eq!(post_crumbs[2].label, "Posts");
        assert!(post_crumbs[2].is_current);
    }
}
