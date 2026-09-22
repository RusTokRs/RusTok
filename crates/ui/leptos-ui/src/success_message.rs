use leptos::prelude::*;

#[component]
pub fn SuccessMessage(#[prop(into)] message: String) -> impl IntoView {
    view! {
        <div class="rounded-lg border border-emerald-500/50 bg-emerald-500/10 px-4 py-3 text-sm text-emerald-600 dark:text-emerald-400">
            {message}
        </div>
    }
}
