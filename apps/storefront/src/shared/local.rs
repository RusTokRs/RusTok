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

    let is_ru = locale.starts_with("ru");

    vec![
        ProductCardData {
            title: message(locale, "product.smart.title"),
            description: message(locale, "product.smart.description"),
            price: message(locale, "product.smart.price"),
            badge: Some(message(locale, "product.smart.badge")),
            image_url: Some("https://images.unsplash.com/photo-1546868871-7041f2a55e12?w=600&auto=format&fit=crop&q=80".to_string()),
            category: Some(if is_ru { "Гаджеты".to_string() } else { "Smart Tech".to_string() }),
            rating: Some(4.9),
            review_count: Some(142),
            original_price: Some(if is_ru { "6 490 ₽".to_string() } else { "$119".to_string() }),
        },
        ProductCardData {
            title: message(locale, "product.eco.title"),
            description: message(locale, "product.eco.description"),
            price: message(locale, "product.eco.price"),
            badge: None,
            image_url: Some("https://images.unsplash.com/photo-1544816155-12df9643f363?w=600&auto=format&fit=crop&q=80".to_string()),
            category: Some(if is_ru { "Эко-стиль".to_string() } else { "Sustainable".to_string() }),
            rating: Some(4.8),
            review_count: Some(89),
            original_price: Some(if is_ru { "3 200 ₽".to_string() } else { "$69".to_string() }),
        },
        ProductCardData {
            title: message(locale, "product.city.title"),
            description: message(locale, "product.city.description"),
            price: message(locale, "product.city.price"),
            badge: Some(message(locale, "product.city.badge")),
            image_url: Some("https://images.unsplash.com/photo-1585336261026-613d54f59c87?w=600&auto=format&fit=crop&q=80".to_string()),
            category: Some(if is_ru { "Город".to_string() } else { "Everyday Carry".to_string() }),
            rating: Some(5.0),
            review_count: Some(214),
            original_price: Some(if is_ru { "7 900 ₽".to_string() } else { "$159".to_string() }),
        },
        ProductCardData {
            title: if is_ru { "Беспроводные наушники RusTok Pro".to_string() } else { "RusTok Pro Sound".to_string() },
            description: if is_ru {
                "Студийные беспроводные наушники с адаптивным ANC и кристальным Hi-Fi звуком.".to_string()
            } else {
                "Studio-grade wireless headphones with adaptive ANC and ultra-low latency.".to_string()
            },
            price: if is_ru { "8 990 ₽".to_string() } else { "$139".to_string() },
            badge: Some(if is_ru { "Хит".to_string() } else { "Best Seller".to_string() }),
            image_url: Some("https://images.unsplash.com/photo-1505740420928-5e560c06d30e?w=600&auto=format&fit=crop&q=80".to_string()),
            category: Some(if is_ru { "Аудио".to_string() } else { "Hi-Fi Audio".to_string() }),
            rating: Some(4.9),
            review_count: Some(95),
            original_price: Some(if is_ru { "10 500 ₽".to_string() } else { "$169".to_string() }),
        },
        ProductCardData {
            title: if is_ru { "Магнитная станция 3-в-1".to_string() } else { "Magnetic 3-in-1 Dock".to_string() },
            description: if is_ru {
                "Быстрая беспроводная зарядка из авиационного алюминия для смартфона, часов и наушников.".to_string()
            } else {
                "Aircraft-grade aluminum fast wireless charging station for all your Apple & Android gear.".to_string()
            },
            price: if is_ru { "3 790 ₽".to_string() } else { "$59".to_string() },
            badge: Some(if is_ru { "Новинка".to_string() } else { "New".to_string() }),
            image_url: Some("https://images.unsplash.com/photo-1588872657578-7efd1f1555ed?w=600&auto=format&fit=crop&q=80".to_string()),
            category: Some(if is_ru { "Зарядка".to_string() } else { "Power & Desk".to_string() }),
            rating: Some(4.7),
            review_count: Some(63),
            original_price: Some(if is_ru { "4 500 ₽".to_string() } else { "$75".to_string() }),
        },
        ProductCardData {
            title: if is_ru { "Керамическая термокружка".to_string() } else { "Artisan Thermal Flask".to_string() },
            description: if is_ru {
                "Вакуумная изоляция до 12 часов с керамическим внутренним покрытием без металлического привкуса.".to_string()
            } else {
                "Ceramic interior vacuum insulation maintaining optimal temperature up to 12 hours.".to_string()
            },
            price: if is_ru { "1 990 ₽".to_string() } else { "$29".to_string() },
            badge: Some("-25%".to_string()),
            image_url: Some("https://images.unsplash.com/photo-1514432324607-a09d9b4aefdd?w=600&auto=format&fit=crop&q=80".to_string()),
            category: Some(if is_ru { "Стиль".to_string() } else { "Lifestyle".to_string() }),
            rating: Some(4.8),
            review_count: Some(118),
            original_price: Some(if is_ru { "2 650 ₽".to_string() } else { "$39".to_string() }),
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
