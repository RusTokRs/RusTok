use gloo_storage::{LocalStorage, Storage};
use leptos::prelude::*;
use serde_json::{Map, Value};
use std::sync::OnceLock;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Locale {
    En,
    Ru,
}

impl Locale {
    pub fn from_code(code: &str) -> Self {
        match code.to_lowercase().as_str() {
            "en" => Locale::En,
            "ru" => Locale::Ru,
            _ => Locale::En,
        }
    }

    pub fn code(&self) -> &'static str {
        match self {
            Locale::En => "en",
            Locale::Ru => "ru",
        }
    }
}

static EN_MESSAGES: OnceLock<Map<String, Value>> = OnceLock::new();
static RU_MESSAGES: OnceLock<Map<String, Value>> = OnceLock::new();

pub fn translate_locale(locale: Locale, key: &str) -> String {
    let messages = match locale {
        Locale::En => {
            EN_MESSAGES.get_or_init(|| load_messages(include_str!("../../../locales/en.json")))
        }
        Locale::Ru => {
            RU_MESSAGES.get_or_init(|| load_messages(include_str!("../../../locales/ru.json")))
        }
    };

    messages
        .get(key)
        .and_then(|value| value.as_str())
        .unwrap_or(key)
        .to_string()
}

fn load_messages(source: &str) -> Map<String, Value> {
    serde_json::from_str::<Map<String, Value>>(source).unwrap_or_default()
}

#[derive(Clone)]
pub struct LocaleContext {
    pub locale: ReadSignal<Locale>,
    pub set_locale: WriteSignal<Locale>,
}

pub fn provide_locale_context() -> LocaleContext {
    let initial_locale = load_locale_from_storage().unwrap_or(Locale::En);
    let (locale, set_locale) = signal(initial_locale);

    Effect::new(move |_| {
        let _ = LocalStorage::set("rustok-admin-locale", locale.get().code());
    });

    let context = LocaleContext { locale, set_locale };
    provide_context(context.clone());
    context
}

pub fn use_locale() -> LocaleContext {
    use_context::<LocaleContext>().expect("LocaleContext not found")
}

pub fn translate(key: &str) -> String {
    let locale = use_locale().locale.get_untracked();
    translate_locale(locale, key)
}

fn load_locale_from_storage() -> Option<Locale> {
    let value: String = LocalStorage::get("rustok-admin-locale").ok()?;
    Some(Locale::from_code(&value))
}
