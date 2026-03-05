use leptos::prelude::*;
// Note: This component might need integration with a specific i18n solution if it grows,
// but for now we keep it simple or allow passing the locale logic.
// However, since it's shared, we might need a generic way.

#[component]
pub fn LanguageToggle<F>(current_locale: Signal<String>, on_set_locale: F) -> impl IntoView
where
    F: Fn(&str) + 'static + Copy,
{
    view! {
        <div class="flex gap-2">
            <button
                type="button"
                class=move || {
                    let is_active = current_locale.get() == "ru";
                    if is_active {
                        "inline-flex items-center justify-center rounded-md border border-primary bg-primary px-3 py-1 text-sm font-medium text-primary-foreground transition-colors hover:bg-primary/90"
                    } else {
                        "inline-flex items-center justify-center rounded-md border border-input px-3 py-1 text-sm font-medium text-foreground transition-colors hover:bg-accent hover:text-accent-foreground"
                    }
                }
                on:click=move |_| on_set_locale("ru")
            >
                "RU"
            </button>
            <button
                type="button"
                class=move || {
                    let is_active = current_locale.get() == "en";
                    if is_active {
                        "inline-flex items-center justify-center rounded-md border border-primary bg-primary px-3 py-1 text-sm font-medium text-primary-foreground transition-colors hover:bg-primary/90"
                    } else {
                        "inline-flex items-center justify-center rounded-md border border-input px-3 py-1 text-sm font-medium text-foreground transition-colors hover:bg-accent hover:text-accent-foreground"
                    }
                }
                on:click=move |_| on_set_locale("en")
            >
                "EN"
            </button>
        </div>
    }
}
