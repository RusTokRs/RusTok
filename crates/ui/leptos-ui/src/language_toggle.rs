use leptos::prelude::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LanguageToggleOption {
    pub locale: String,
    pub label: String,
}

impl LanguageToggleOption {
    pub fn new(locale: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            locale: locale.into(),
            label: label.into(),
        }
    }
}

#[component]
pub fn LanguageToggle<F>(
    current_locale: Signal<String>,
    options: Vec<LanguageToggleOption>,
    on_set_locale: F,
) -> impl IntoView
where
    F: Fn(&str) + 'static + Copy,
{
    view! {
        <div class="flex gap-2">
            {options
                .into_iter()
                .map(|option| {
                    let locale_for_class = option.locale.clone();
                    let locale_for_click = option.locale.clone();
                    view! {
                        <button
                            type="button"
                            class=move || {
                                let is_active = current_locale.get() == locale_for_class;
                                if is_active {
                                    "inline-flex items-center justify-center rounded-md border border-primary bg-primary px-3 py-1 text-sm font-medium text-primary-foreground transition-colors hover:bg-primary/90"
                                } else {
                                    "inline-flex items-center justify-center rounded-md border border-input px-3 py-1 text-sm font-medium text-foreground transition-colors hover:bg-accent hover:text-accent-foreground"
                                }
                            }
                            on:click=move |_| on_set_locale(locale_for_click.as_str())
                        >
                            {option.label}
                        </button>
                    }
                })
                .collect_view()}
        </div>
    }
}
