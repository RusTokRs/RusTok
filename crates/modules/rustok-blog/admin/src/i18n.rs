rustok_ui_i18n::declare_module_i18n!();

/// Localized preview-token expiry line, which interpolates the expiry
/// timestamp into one message, for example `Expires 2026-11-06T12:00:00Z`.
///
/// The no-argument `t` family cannot serve this key: `fluent-bundle` records a
/// reference error for every unbound placeable, the shared resolver treats any
/// error as a failed lookup, and the caller literal wins — so the catalog text
/// never renders and every locale shows the English fallback. Binding the value
/// as a Fluent argument is the only way to localize the label. The trailing
/// placeholder substitution keeps the caller literal usable for a locale that
/// has no such message.
pub fn preview_expires_message<'a>(
    locale: Option<&str>,
    key: &str,
    fallback: &str,
    at: &'a str,
) -> String {
    let mut arguments: rustok_ui_i18n::FluentArgs<'a> = rustok_ui_i18n::FluentArgs::new();
    arguments.set("at", at);
    let resolved = format(locale, key, Some(&arguments), fallback);
    crate::core::preview_expires_label(resolved.as_str(), at)
}

#[cfg(test)]
mod tests {
    use super::preview_expires_message;

    /// The shared bundle resolves with bidi isolation on, so an interpolated
    /// value is wrapped in invisible FSI/PDI marks. Assertions compare the
    /// visible text.
    fn visible(text: &str) -> String {
        text.replace(['\u{2068}', '\u{2069}'], "")
    }

    #[test]
    fn preview_expires_message_binds_the_timestamp_argument() {
        // `t` on the same key cannot bind `$at`, so it would return the English
        // fallback for a Russian reader; a bound argument is what makes the
        // localized form render.
        assert_eq!(
            visible(
                preview_expires_message(
                    Some("en"),
                    "blog.preview.expires",
                    "Expires {at}",
                    "2026-11-06T12:00:00Z"
                )
                .as_str()
            ),
            "Expires 2026-11-06T12:00:00Z"
        );
        assert_eq!(
            visible(
                preview_expires_message(
                    Some("ru"),
                    "blog.preview.expires",
                    "Expires {at}",
                    "2026-11-06T12:00:00Z"
                )
                .as_str()
            ),
            "Действует до 2026-11-06T12:00:00Z"
        );
    }

    #[test]
    fn preview_expires_message_keeps_the_caller_literal_for_unknown_keys() {
        assert_eq!(
            preview_expires_message(
                Some("en"),
                "blog.preview.absentKey",
                "Expires {at}",
                "2026-11-06T12:00:00Z"
            ),
            "Expires 2026-11-06T12:00:00Z"
        );
    }
}
