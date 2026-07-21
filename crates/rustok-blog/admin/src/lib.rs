mod core;
mod i18n;
mod model;
mod moderation;
mod transport;
mod ui;

use leptos::prelude::*;
use moderation::BlogModerationPanel;
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
