#![recursion_limit = "512"]

mod core;
mod i18n;
mod model;
mod moderation;
mod page_builder;
mod transport;
mod ui;

use leptos::prelude::*;
use moderation::BlogModerationPanel;
pub use page_builder::{
    BlogContributionAdapter, BlogWidgetOwnerSchemaRef, BlogWidgetPropertyEditorModel,
    BlogWidgetRenderModel, blog_contribution_manifest, blog_fly_registry_set,
    blog_full_admin_contribution_policy, blog_widget_contribution,
    blog_widget_preview_contribution, build_blog_admin_contribution_registry,
    register_blog_fly_widgets,
};
use ui::BlogAdmin as BlogEditor;

#[component]
pub fn BlogAdmin() -> impl IntoView {
    view! {
        <div class="pb-8">
            <BlogEditor />
            <BlogModerationPanel />
        </div>
    }
}
