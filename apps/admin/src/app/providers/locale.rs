use leptos::prelude::*;

/// The admin host's supported effective locales.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Locale {
    #[default]
    En,
    Ru,
}

impl Locale {
    pub fn from_effective_locale(locale: Option<&str>) -> Self {
        match rustok_ui_i18n::normalize_admin_locale(locale) {
            "ru" => Self::Ru,
            _ => Self::En,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::En => "en",
            Self::Ru => "ru",
        }
    }
}

/// Leptos lifecycle adapter for the host-provided effective locale.
///
/// Translation semantics remain in `rustok-ui-i18n`; this context owns only
/// reactive propagation and persistence of an explicit operator selection.
#[derive(Clone, Copy)]
pub struct AdminLocaleContext {
    locale: RwSignal<Locale>,
}

impl AdminLocaleContext {
    pub fn current(self) -> Locale {
        self.locale.get()
    }

    pub fn set(self, locale: Locale) {
        if self.locale.get_untracked() == locale {
            return;
        }
        self.locale.set(locale);
        crate::shared::api::set_stored_locale(locale.as_str());
    }

    pub fn translate(self, key: &str) -> String {
        crate::i18n::t(Some(self.current().as_str()), key, key)
    }
}

pub fn use_admin_locale() -> AdminLocaleContext {
    expect_context::<AdminLocaleContext>()
}

#[component]
pub fn AdminLocaleProvider(children: Children) -> impl IntoView {
    let initial_locale =
        Locale::from_effective_locale(crate::shared::api::get_stored_locale().as_deref());
    provide_context(AdminLocaleContext {
        locale: RwSignal::new(initial_locale),
    });
    children()
}
