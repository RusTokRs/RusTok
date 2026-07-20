use leptos::prelude::*;
pub use leptos_ui::*;

pub mod page_header;
pub use page_header::PageHeader;

use crate::{Locale, t_string, use_i18n};

#[cfg(target_arch = "wasm32")]
const THEME_STORAGE_KEY: &str = "rustok-admin-theme";

#[derive(Clone, Copy, Eq, PartialEq)]
enum ThemeMode {
    Light,
    Dark,
}

impl ThemeMode {
    #[cfg(target_arch = "wasm32")]
    fn as_str(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }

    fn is_dark(self) -> bool {
        self == Self::Dark
    }

    fn toggle(self) -> Self {
        match self {
            Self::Light => Self::Dark,
            Self::Dark => Self::Light,
        }
    }
}

#[component]
pub fn Button(
    #[prop(into)] on_click: Callback<()>,
    #[prop(optional)] children: Option<Children>,
    #[prop(optional, into)] class: String,
    #[prop(default = Signal::derive(|| false))] disabled: Signal<bool>,
) -> impl IntoView {
    let base_class = "inline-flex h-9 shrink-0 items-center justify-center gap-2 whitespace-nowrap rounded-md bg-primary px-4 py-2 text-sm font-medium text-primary-foreground shadow-xs outline-none transition-all hover:bg-primary/90 focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50 disabled:pointer-events-none disabled:opacity-50";
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
            on:click=move |_| on_click.run(())
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
                class="flex h-9 w-full min-w-0 rounded-md border border-input bg-background px-3 py-1 text-sm shadow-xs outline-none transition-[color,box-shadow] placeholder:text-muted-foreground focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50 disabled:cursor-not-allowed disabled:opacity-50"
            />
        </div>
    }
}

#[component]
pub fn LanguageToggle() -> impl IntoView {
    let i18n = use_i18n();

    view! {
        <select
            aria-label=move || t_string!(i18n, app.nav.language).to_string()
            prop:value=move || match i18n.get_locale() {
                Locale::ru => "ru",
                Locale::en => "en",
            }
            on:change=move |ev| match event_target_value(&ev).as_str() {
                "ru" => i18n.set_locale(Locale::ru),
                "en" => i18n.set_locale(Locale::en),
                _ => {}
            }
            class="h-9 min-w-32 rounded-md border border-input bg-background px-3 py-1 text-sm font-medium text-foreground shadow-xs outline-none transition-[color,box-shadow] focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50"
        >
            <option value="ru">{move || t_string!(i18n, app.nav.languageRu)}</option>
            <option value="en">{move || t_string!(i18n, app.nav.languageEn)}</option>
        </select>
    }
}

#[component]
pub fn ThemeModeToggle() -> impl IntoView {
    let i18n = use_i18n();
    let (theme, set_theme) = signal(initial_theme_mode());

    Effect::new(move |_| {
        apply_theme_mode(theme.get());
    });

    let toggle_theme = move |_| {
        set_theme.update(|mode| *mode = mode.toggle());
    };

    let icon_base = "absolute h-4 w-4 transition-all duration-300 ease-out";
    let sun_class = move || {
        if theme.get().is_dark() {
            format!("{icon_base} rotate-90 scale-0 opacity-0")
        } else {
            format!("{icon_base} rotate-0 scale-100 opacity-100")
        }
    };
    let moon_class = move || {
        if theme.get().is_dark() {
            format!("{icon_base} rotate-0 scale-100 opacity-100")
        } else {
            format!("{icon_base} -rotate-90 scale-0 opacity-0")
        }
    };

    view! {
        <button
            type="button"
            aria-label=move || t_string!(i18n, app.theme.toggle).to_string()
            title=move || {
                if theme.get().is_dark() {
                    t_string!(i18n, app.theme.dark).to_string()
                } else {
                    t_string!(i18n, app.theme.light).to_string()
                }
            }
            class="relative inline-flex h-8 w-8 shrink-0 items-center justify-center overflow-hidden rounded-md border border-input bg-background text-foreground shadow-xs outline-none transition-[background-color,color,box-shadow] hover:bg-accent hover:text-accent-foreground focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50"
            on:click=toggle_theme
        >
            <svg
                aria-hidden="true"
                class=sun_class
                fill="none"
                viewBox="0 0 24 24"
                stroke="currentColor"
                stroke-width="2"
                stroke-linecap="round"
                stroke-linejoin="round"
            >
                <circle cx="12" cy="12" r="4" />
                <path d="M12 2v2" />
                <path d="M12 20v2" />
                <path d="m4.93 4.93 1.41 1.41" />
                <path d="m17.66 17.66 1.41 1.41" />
                <path d="M2 12h2" />
                <path d="M20 12h2" />
                <path d="m6.34 17.66-1.41 1.41" />
                <path d="m19.07 4.93-1.41 1.41" />
            </svg>
            <svg
                aria-hidden="true"
                class=moon_class
                fill="none"
                viewBox="0 0 24 24"
                stroke="currentColor"
                stroke-width="2"
                stroke-linecap="round"
                stroke-linejoin="round"
            >
                <path d="M12 3a6.9 6.9 0 0 0 9 9 9 9 0 1 1-9-9Z" />
            </svg>
            <span class="sr-only">{move || t_string!(i18n, app.theme.toggle)}</span>
        </button>
    }
}

fn initial_theme_mode() -> ThemeMode {
    read_stored_theme_mode().unwrap_or(ThemeMode::Light)
}

#[cfg(target_arch = "wasm32")]
fn read_stored_theme_mode() -> Option<ThemeMode> {
    let storage = web_sys::window()?.local_storage().ok().flatten()?;
    match storage.get_item(THEME_STORAGE_KEY).ok().flatten()?.as_str() {
        "dark" => Some(ThemeMode::Dark),
        "light" => Some(ThemeMode::Light),
        _ => None,
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn read_stored_theme_mode() -> Option<ThemeMode> {
    None
}

#[cfg(target_arch = "wasm32")]
fn apply_theme_mode(mode: ThemeMode) {
    use wasm_bindgen::JsCast;

    if let Some(window) = web_sys::window() {
        if let Some(storage) = window.local_storage().ok().flatten() {
            let _ = storage.set_item(THEME_STORAGE_KEY, mode.as_str());
        }

        if let Some(document) = window.document() {
            if let Some(root) = document.document_element() {
                let classes = root.class_list();
                if mode.is_dark() {
                    let _ = classes.add_1("dark");
                } else {
                    let _ = classes.remove_1("dark");
                }

                if let Some(html) = root.dyn_ref::<web_sys::HtmlElement>() {
                    let _ = html.style().set_property("color-scheme", mode.as_str());
                }
            }
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn apply_theme_mode(_mode: ThemeMode) {}
