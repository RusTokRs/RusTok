mod components;
pub mod leptos;
mod richtext;

use ::leptos::prelude::*;
use crate::moderation::BlogModerationPanel;
use self::leptos::BlogAdmin as BlogEditor;

/// Page composition belongs to the UI adapter, not lib.rs.
#[component]
pub fn BlogAdmin() -> impl IntoView {
    view! {
        <div class="pb-8">
            <BlogEditor />
            <BlogModerationPanel />
        </div>
    }
}
