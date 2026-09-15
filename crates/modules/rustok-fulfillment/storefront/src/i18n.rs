rustok_ui_i18n::declare_module_i18n!();

#[cfg(test)]
mod tests {
    use super::t;

    #[test]
    fn resolves_regional_russian_locale() {
        assert_eq!(
            t(Some("ru-RU"), "fulfillment.shipping.badge", "Shipping"),
            "Доставка"
        );
    }
}
