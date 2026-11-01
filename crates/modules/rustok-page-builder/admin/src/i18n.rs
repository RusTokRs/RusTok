rustok_ui_i18n::declare_module_i18n!();

/// Localized Fly contribution summary template.
///
/// The Fly workspace substitutes `{registered}` / `{skipped}` itself at render
/// time from the live assembly, so the catalog message carries them as Fluent
/// variables and this wrapper binds the placeholder tokens as their values: a
/// plain `t` lookup cannot bind variables, so the localized text would never
/// render and every locale would fall back to the English literal. The shared
/// resolver wraps interpolated values in bidi isolation marks, which would
/// defeat the Fly placeholder substitution, so they are stripped.
pub fn contributions_summary_template(locale: Option<&str>) -> String {
    let mut arguments = rustok_ui_i18n::FluentArgs::new();
    arguments.set("registered", "{registered}");
    arguments.set("skipped", "{skipped}");
    let resolved = format(
        locale,
        "page_builder.panel.contributionsSummary",
        Some(&arguments),
        "Provider contributions: {registered} registered, {skipped} skipped",
    );
    resolved.replace(['\u{2068}', '\u{2069}'], "")
}

#[cfg(test)]
mod tests {
    use super::contributions_summary_template;

    #[test]
    fn contributions_summary_template_binds_the_placeholder_tokens() {
        // A plain `t` on the same key cannot bind `$registered`/`$skipped`,
        // so it would return the English fallback for a Russian reader; bound
        // arguments are what make the localized form render, and the wrapper
        // keeps the `{registered}`/`{skipped}` tokens the Fly workspace
        // substitutes at render time.
        let en_template = contributions_summary_template(Some("en"));
        assert!(en_template.contains("{registered}"), "{en_template}");
        assert!(en_template.contains("{skipped}"), "{en_template}");

        let ru_template = contributions_summary_template(Some("ru"));
        assert!(ru_template.contains("зарегистрировано"), "{ru_template}");
        assert!(ru_template.contains("{registered}"), "{ru_template}");
        assert!(ru_template.contains("{skipped}"), "{ru_template}");
    }
}
