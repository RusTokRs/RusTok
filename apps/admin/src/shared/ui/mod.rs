pub use leptos_ui::*;

pub mod page_header;
pub use page_header::page_header;

use crate::shared::i18n::{use_locale, Locale};
use leptos::prelude::*;

#[component]
pub fn ui_language_toggle() -> impl IntoView {
    let locale = use_locale();

    let set_locale = move |value: Locale| {
        locale.set_locale.set(value);
    };

    let current_locale = Signal::derive(move || {
        match locale.locale.get() {
            Locale::Ru => "ru",
            Locale::En => "en",
        }
        .to_string()
    });

    view! {
        <leptos_ui::ui_language_toggle
            current_locale=current_locale
            on_set_locale=move |lang| {
                match lang {
                    "ru" => set_locale(Locale::Ru),
                    "en" => set_locale(Locale::En),
                    _ => {}
                }
            }
        />
    }
}
