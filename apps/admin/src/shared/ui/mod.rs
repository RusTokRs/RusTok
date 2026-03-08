pub mod page_header;

pub use page_header::PageHeader;

use leptos::prelude::*;
use leptos::web_sys;

use crate::{use_i18n, Locale};

#[component]
pub fn Button(
    #[prop(into)] on_click: Callback<web_sys::MouseEvent>,
    #[prop(optional)] children: Option<Children>,
    #[prop(optional, into)] class: String,
    #[prop(default = Signal::derive(|| false))] disabled: Signal<bool>,
) -> impl IntoView {
    let base_class = "inline-flex items-center justify-center whitespace-nowrap rounded-md text-sm font-medium ring-offset-background transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:pointer-events-none disabled:opacity-50 bg-primary text-primary-foreground shadow hover:bg-primary/90 h-9 px-4 py-2";
    let merged_class = move || {
        if class.is_empty() {
            base_class.to_string()
        } else {
            format!("{base_class} {class}")
        }
    };

    view! {
        <button
            class=merged_class
            on:click=move |ev| on_click.run(ev)
            disabled=move || disabled.get()
        >
            {children.map(|c| c())}
        </button>
    }
}

#[component]
pub fn Input(
    #[prop(into)] value: Signal<String>,
    #[prop(into)] set_value: WriteSignal<String>,
    #[prop(into)] placeholder: TextProp,
    #[prop(default = "text")] type_: &'static str,
    #[prop(default = String::new().into(), into)] label: TextProp,
) -> impl IntoView {
    view! {
        <div class="flex flex-col gap-2">
            {move || {
                let label_value = label.get();
                (!label_value.is_empty()).then(|| {
                    view! {
                        <label class="text-sm font-medium leading-none peer-disabled:cursor-not-allowed peer-disabled:opacity-70">
                            {label_value}
                        </label>
                    }
                })
            }}
            <input
                type=type_
                placeholder=placeholder
                prop:value=value
                on:input=move |ev| set_value.set(event_target_value(&ev))
                class="flex h-9 w-full rounded-md border border-input bg-background px-3 py-1 text-sm shadow-sm transition-colors placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-50"
            />
        </div>
    }
}

#[component]
pub fn LanguageToggle() -> impl IntoView {
    let i18n = use_i18n();

    view! {
        <div class="flex gap-2">
            <button
                type="button"
                class=move || {
                    let is_active = i18n.get_locale() == Locale::ru;
                    if is_active {
                        "inline-flex items-center justify-center rounded-md border border-primary bg-primary px-3 py-1 text-sm font-medium text-primary-foreground transition-colors hover:bg-primary/90"
                    } else {
                        "inline-flex items-center justify-center rounded-md border border-input px-3 py-1 text-sm font-medium text-foreground transition-colors hover:bg-accent hover:text-accent-foreground"
                    }
                }
                on:click=move |_| i18n.set_locale(Locale::ru)
            >
                "RU"
            </button>
            <button
                type="button"
                class=move || {
                    let is_active = i18n.get_locale() == Locale::en;
                    if is_active {
                        "inline-flex items-center justify-center rounded-md border border-primary bg-primary px-3 py-1 text-sm font-medium text-primary-foreground transition-colors hover:bg-primary/90"
                    } else {
                        "inline-flex items-center justify-center rounded-md border border-input px-3 py-1 text-sm font-medium text-foreground transition-colors hover:bg-accent hover:text-accent-foreground"
                    }
                }
                on:click=move |_| i18n.set_locale(Locale::en)
            >
                "EN"
            </button>
        </div>
    }
}
