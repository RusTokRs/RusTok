use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LocaleStrings {
    pub hero_title: &'static str,
    pub hero_subtitle: &'static str,
    pub cta_primary: &'static str,
    pub cta_secondary: &'static str,
    pub featured_title: &'static str,
    pub featured_subtitle: &'static str,
    pub story_title: &'static str,
    pub story_body: &'static str,
    pub newsletter_title: &'static str,
    pub newsletter_body: &'static str,
    pub newsletter_cta: &'static str,
    pub newsletter_placeholder: &'static str,
    pub newsletter_note: &'static str,
    pub cta_view: &'static str,
    pub nav_home: &'static str,
    pub nav_catalog: &'static str,
    pub nav_about: &'static str,
    pub nav_contact: &'static str,
    pub nav_language: &'static str,
    pub footer_tagline: &'static str,
    pub badge_new: &'static str,
}

pub fn locale_strings(locale: &str) -> LocaleStrings {
    match locale {
        "ru" => LocaleStrings {
            hero_title: "Витрина RusToK",
            hero_subtitle: "SSR-платформа на Rust для быстрой коммерции и красивых витрин.",
            cta_primary: "Открыть каталог",
            cta_secondary: "О платформе",
            featured_title: "Избранные коллекции",
            featured_subtitle: "Заполните карточки товарами из Commerce-модуля и настройте промо.",
            story_title: "Бренд, которому доверяют",
            story_body: "RusToK помогает собрать надежную витрину: события, поисковые индексы, многоязычность.",
            newsletter_title: "Подписка на обновления",
            newsletter_body: "Подключайте рассылки и промо-кампании прямо из ядра.",
            newsletter_cta: "Подписаться",
            newsletter_placeholder: "email@rustok.local",
            newsletter_note: "Никакого спама. Отписка в любой момент.",
            cta_view: "Смотреть",
            nav_home: "Главная",
            nav_catalog: "Каталог",
            nav_about: "О нас",
            nav_contact: "Контакты",
            nav_language: "Язык",
            footer_tagline: "RusToK — модульная платформа для современной коммерции.",
            badge_new: "Новинка",
        },
        _ => LocaleStrings {
            hero_title: "RusToK Storefront",
            hero_subtitle: "SSR-first Rust commerce with fast discovery and clean design.",
            cta_primary: "Browse catalog",
            cta_secondary: "Platform overview",
            featured_title: "Featured collections",
            featured_subtitle: "Plug Commerce data in and curate promotions in minutes.",
            story_title: "A brand you can trust",
            story_body: "RusToK ships with events, search indexes, and multilingual support.",
            newsletter_title: "Stay in the loop",
            newsletter_body: "Connect newsletters and promotions from the core.",
            newsletter_cta: "Subscribe",
            newsletter_placeholder: "email@rustok.local",
            newsletter_note: "No spam. Opt-out anytime.",
            cta_view: "View",
            nav_home: "Home",
            nav_catalog: "Catalog",
            nav_about: "About",
            nav_contact: "Contact",
            nav_language: "Language",
            footer_tagline: "RusToK — the modular platform for modern commerce.",
            badge_new: "New",
        },
    }
}

pub fn featured_products(locale: &str) -> Vec<crate::entities::product::ProductCardData> {
    use crate::entities::product::ProductCardData;
    match locale {
        "ru" => vec![
            ProductCardData {
                title: "Смарт-аксессуары",
                description: "Функциональные гаджеты для повседневной жизни.",
                price: "от 4 990 ₽",
                badge: Some("Лидер продаж"),
            },
            ProductCardData {
                title: "Экологичная коллекция",
                description: "Натуральные материалы и премиальная отделка.",
                price: "от 2 490 ₽",
                badge: None,
            },
            ProductCardData {
                title: "Городская классика",
                description: "Минималистичные силуэты для любого сезона.",
                price: "от 6 500 ₽",
                badge: Some("Новинка"),
            },
        ],
        _ => vec![
            ProductCardData {
                title: "Smart accessories",
                description: "Functional gadgets for everyday life.",
                price: "from $89",
                badge: Some("Best seller"),
            },
            ProductCardData {
                title: "Eco collection",
                description: "Natural materials with premium finishing.",
                price: "from $49",
                badge: None,
            },
            ProductCardData {
                title: "City essentials",
                description: "Minimal silhouettes for every season.",
                price: "from $129",
                badge: Some("New"),
            },
        ],
    }
}
