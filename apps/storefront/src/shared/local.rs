use serde::{Deserialize, Serialize};

static MESSAGES: rustok_ui_i18n::UiMessages = rustok_ui_i18n::UiMessages::new(
    "en",
    &[
        ("en", include_str!("../../locales/en.ftl")),
        ("ru", include_str!("../../locales/ru.ftl")),
    ],
);

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LocaleStrings {
    pub hero_title: String,
    pub hero_subtitle: String,
    pub cta_primary: String,
    pub cta_secondary: String,
    pub featured_title: String,
    pub featured_subtitle: String,
    pub story_title: String,
    pub story_body: String,
    pub newsletter_title: String,
    pub newsletter_body: String,
    pub newsletter_cta: String,
    pub newsletter_placeholder: String,
    pub newsletter_note: String,
    pub cta_view: String,
    pub nav_home: String,
    pub nav_catalog: String,
    pub nav_about: String,
    pub nav_contact: String,
    pub nav_language: String,
    pub footer_tagline: String,
    pub badge_new: String,
}

fn message(locale: &str, key: &str) -> String {
    MESSAGES.t(Some(locale), key, key)
}

pub fn locale_strings(locale: &str) -> LocaleStrings {
    LocaleStrings {
        hero_title: message(locale, "hero.title"),
        hero_subtitle: message(locale, "hero.subtitle"),
        cta_primary: message(locale, "cta.primary"),
        cta_secondary: message(locale, "cta.secondary"),
        featured_title: message(locale, "featured.title"),
        featured_subtitle: message(locale, "featured.subtitle"),
        story_title: message(locale, "story.title"),
        story_body: message(locale, "story.body"),
        newsletter_title: message(locale, "newsletter.title"),
        newsletter_body: message(locale, "newsletter.body"),
        newsletter_cta: message(locale, "newsletter.cta"),
        newsletter_placeholder: message(locale, "newsletter.placeholder"),
        newsletter_note: message(locale, "newsletter.note"),
        cta_view: message(locale, "cta.view"),
        nav_home: message(locale, "nav.home"),
        nav_catalog: message(locale, "nav.catalog"),
        nav_about: message(locale, "nav.about"),
        nav_contact: message(locale, "nav.contact"),
        nav_language: message(locale, "nav.language"),
        footer_tagline: message(locale, "footer.tagline"),
        badge_new: message(locale, "badge.new"),
    }
}

pub fn featured_products(locale: &str) -> Vec<crate::entities::product::ProductCardData> {
    use crate::entities::product::ProductCardData;

    vec![
        ProductCardData {
            title: message(locale, "product.smart.title"),
            description: message(locale, "product.smart.description"),
            price: message(locale, "product.smart.price"),
            badge: Some(message(locale, "product.smart.badge")),
        },
        ProductCardData {
            title: message(locale, "product.eco.title"),
            description: message(locale, "product.eco.description"),
            price: message(locale, "product.eco.price"),
            badge: None,
        },
        ProductCardData {
            title: message(locale, "product.city.title"),
            description: message(locale, "product.city.description"),
            price: message(locale, "product.city.price"),
            badge: Some(message(locale, "product.city.badge")),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::{MESSAGES, featured_products, locale_strings};

    #[test]
    fn host_catalogs_are_valid_and_locale_specific() {
        assert!(MESSAGES.initialization_diagnostics().is_empty());
        assert_eq!(locale_strings("en").nav_home, "Home");
        assert_eq!(locale_strings("ru").nav_home, "Главная");
        assert_eq!(featured_products("ru")[0].title, "Смарт-аксессуары");
    }
}
