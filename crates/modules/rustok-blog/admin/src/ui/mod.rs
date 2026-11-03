mod categories;
mod components;
pub mod leptos;
mod moderation;
mod richtext;

use ::leptos::prelude::*;
use rustok_ui_core::UiRouteContext;
use self::categories::BlogCategoriesAdmin;
use self::leptos::BlogAdmin as BlogEditor;
use self::moderation::BlogModerationPanel;
use crate::core::{blog_admin_section_for_subpath, BlogAdminSection};

/// Page composition belongs to the UI adapter, not lib.rs. The host renders
/// one module component for every child page, so the module owns the subpath
/// dispatch: `/categories` swaps the posts workspace for the category
/// management workspace (same shape as the Forum admin root).
#[component]
pub fn BlogAdmin() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let section = blog_admin_section_for_subpath(route_context.subpath.as_deref());
    match section {
        BlogAdminSection::Categories => view! {
            <div class="pb-8">
                <BlogCategoriesAdmin />
            </div>
        }
        .into_any(),
        BlogAdminSection::Posts => view! {
            <div class="pb-8">
                <BlogEditor />
                <BlogModerationPanel />
            </div>
        }
        .into_any(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blog_categories_subpath_selects_category_section() {
        assert_eq!(
            blog_admin_section_for_subpath(Some("categories")),
            BlogAdminSection::Categories
        );
    }

    #[test]
    fn blog_categories_nested_subpath_selects_category_section() {
        assert_eq!(
            blog_admin_section_for_subpath(Some("categories/anything")),
            BlogAdminSection::Categories
        );
    }

    #[test]
    fn blog_posts_subpaths_stay_in_posts_section() {
        assert_eq!(
            blog_admin_section_for_subpath(Some("posts")),
            BlogAdminSection::Posts
        );
        assert_eq!(
            blog_admin_section_for_subpath(Some("new")),
            BlogAdminSection::Posts
        );
        assert_eq!(
            blog_admin_section_for_subpath(Some("category")),
            BlogAdminSection::Posts
        );
        assert_eq!(blog_admin_section_for_subpath(None), BlogAdminSection::Posts);
    }
}
