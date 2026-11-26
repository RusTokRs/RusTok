mod categories;
mod components;
pub mod leptos;
mod moderation;
mod richtext;

use self::categories::BlogCategoriesAdmin;
use self::leptos::BlogAdmin as BlogEditor;
use self::moderation::BlogModerationPanel;
use crate::core::{BlogAdminSection, blog_admin_section_for_subpath};
use ::leptos::prelude::*;
use rustok_ui_core::UiRouteContext;

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
