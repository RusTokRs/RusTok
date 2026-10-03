//! Locale identity behind a trait, so Fly does not hard-depend on the platform i18n crate.
//!
//! # Why this exists
//!
//! Fly is meant to be extractable into a standalone repository (see `standalone-Cargo.toml`), and
//! the single thing preventing that was two function calls into `rustok-ui-i18n` from
//! `runtime_locale.rs`. That crate is backed by ICU4X/CLDR data; reimplementing it inside Fly
//! would be both futile and wrong. So the dependency is *inverted* rather than removed: Fly
//! declares what it needs, and the platform supplies it.
//!
//! # Capability levels
//!
//! | Resolver | Canonicalization | Likely subtags |
//! |---|---|---|
//! | [`PlatformLocaleResolver`] (feature `platform-i18n`, on by default) | full CLDR | yes |
//! | [`BasicLocaleResolver`] | structural BCP-47 | no |
//!
//! The difference is not cosmetic. With CLDR likely-subtag data, `zh-TW` falls back through
//! `zh-Hant`; without it, `zh-TW` falls back straight to `zh` and a `zh-Hant` translation is never
//! consulted. Legacy language codes (`iw` → `he`) are likewise only canonicalized by the platform
//! resolver. Host builds therefore keep the default feature on; only a standalone extraction runs
//! with `--no-default-features`, and accepts the documented loss.

/// Locale identity operations Fly needs from its host.
pub trait LocaleResolver {
    /// Canonicalize a locale tag, or return `None` if it is not a valid locale.
    ///
    /// Must be idempotent: `normalize_tag(normalize_tag(x)) == normalize_tag(x)`.
    fn normalize_tag(&self, locale: &str) -> Option<String>;

    /// The fallback chain for a locale, most specific first, including the locale itself.
    ///
    /// Must return canonicalized tags, and must be empty for an invalid locale.
    fn fallback_chain(&self, locale: &str) -> Vec<String>;
}

/// Structural BCP-47 resolver with no CLDR data.
///
/// Used when the `platform-i18n` feature is off. It canonicalizes subtag casing and peels
/// specificity layers (variants, then region, then script), which is correct as far as it goes —
/// it simply cannot infer what it does not know. See the module docs for what is lost.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BasicLocaleResolver;

/// A locale tag decomposed into BCP-47 subtags.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedTag {
    language: String,
    script: Option<String>,
    region: Option<String>,
    variants: Vec<String>,
}

impl ParsedTag {
    fn render(&self) -> String {
        let mut output = self.language.clone();
        if let Some(script) = &self.script {
            output.push('-');
            output.push_str(script);
        }
        if let Some(region) = &self.region {
            output.push('-');
            output.push_str(region);
        }
        for variant in &self.variants {
            output.push('-');
            output.push_str(variant);
        }
        output
    }
}

fn is_alpha(value: &str) -> bool {
    !value.is_empty() && value.chars().all(|c| c.is_ascii_alphabetic())
}

fn is_digits(value: &str) -> bool {
    !value.is_empty() && value.chars().all(|c| c.is_ascii_digit())
}

fn is_alphanumeric(value: &str) -> bool {
    !value.is_empty() && value.chars().all(|c| c.is_ascii_alphanumeric())
}

/// Parse a tag into subtags, rejecting anything that is not well-formed BCP-47.
///
/// Uses an index cursor rather than `Peekable`: in a let-chain the borrow from `peek()` lives for
/// the whole `if` body, which would conflict with advancing the iterator inside it.
fn parse_tag(locale: &str) -> Option<ParsedTag> {
    let trimmed = locale.trim();
    // A sanity bound; the longest realistic tag is far shorter, and this keeps the parser from
    // being handed unbounded input.
    if trimmed.is_empty() || trimmed.len() > 64 {
        return None;
    }

    // `_` is accepted as a separator because POSIX-style tags (`ru_RU`) reach Fly from stored
    // data, and the platform resolver accepts them too.
    let parts = trimmed.split(['-', '_']).collect::<Vec<_>>();
    let mut cursor = 0usize;

    let language = parts.first()?;
    // 1- and 4-letter primary subtags are not valid languages.
    if !is_alpha(language) || language.len() == 1 || language.len() == 4 || language.len() > 8 {
        return None;
    }
    let language = language.to_ascii_lowercase();
    cursor += 1;

    let mut script = None;
    if let Some(candidate) = parts.get(cursor)
        && candidate.len() == 4
        && is_alpha(candidate)
    {
        let mut value = candidate.to_ascii_lowercase();
        value[..1].make_ascii_uppercase();
        script = Some(value);
        cursor += 1;
    }

    let mut region = None;
    if let Some(candidate) = parts.get(cursor)
        && ((candidate.len() == 2 && is_alpha(candidate))
            || (candidate.len() == 3 && is_digits(candidate)))
    {
        region = Some(candidate.to_ascii_uppercase());
        cursor += 1;
    }

    let mut variants: Vec<String> = Vec::new();
    for part in &parts[cursor..] {
        // A singleton (`u`, `t`, `x`, ...) starts an extension. Extensions take no part in catalog
        // selection, so parsing stops here — everything before it was valid, so the tag is
        // accepted rather than rejected.
        if part.len() == 1 {
            if !is_alphanumeric(part) {
                return None;
            }
            break;
        }
        let is_variant = ((5..=8).contains(&part.len()) && is_alphanumeric(part))
            || (part.len() == 4
                && part.starts_with(|c: char| c.is_ascii_digit())
                && is_alphanumeric(part));
        if !is_variant {
            return None;
        }
        let variant = part.to_ascii_lowercase();
        if !variants.contains(&variant) {
            variants.push(variant);
        }
    }
    // Canonical order for what is semantically an unordered set, so a tag always renders
    // identically no matter how it was written.
    variants.sort();

    Some(ParsedTag {
        language,
        script,
        region,
        variants,
    })
}

impl LocaleResolver for BasicLocaleResolver {
    fn normalize_tag(&self, locale: &str) -> Option<String> {
        parse_tag(locale).map(|tag| tag.render())
    }

    fn fallback_chain(&self, locale: &str) -> Vec<String> {
        let Some(mut tag) = parse_tag(locale) else {
            return Vec::new();
        };
        let mut chain = vec![tag.render()];
        let push = |chain: &mut Vec<String>, tag: &ParsedTag| {
            let rendered = tag.render();
            if !chain.contains(&rendered) {
                chain.push(rendered);
            }
        };

        // Variants are one unordered specificity layer, so they are dropped together: peeling them
        // one at a time would manufacture partial-variant parents that never existed.
        if !tag.variants.is_empty() {
            tag.variants.clear();
            push(&mut chain, &tag);
        }
        if tag.region.is_some() {
            tag.region = None;
            push(&mut chain, &tag);
        }
        if tag.script.is_some() {
            tag.script = None;
            push(&mut chain, &tag);
        }
        chain
    }
}

/// Resolver backed by the platform's CLDR implementation.
#[cfg(feature = "platform-i18n")]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PlatformLocaleResolver;

#[cfg(feature = "platform-i18n")]
impl LocaleResolver for PlatformLocaleResolver {
    fn normalize_tag(&self, locale: &str) -> Option<String> {
        rustok_ui_i18n::normalize_locale_tag(locale)
    }

    fn fallback_chain(&self, locale: &str) -> Vec<String> {
        rustok_ui_i18n::locale_fallback_chain(locale)
    }
}

/// The resolver backing the crate-level locale functions.
#[cfg(feature = "platform-i18n")]
pub type DefaultLocaleResolver = PlatformLocaleResolver;

/// The resolver backing the crate-level locale functions.
#[cfg(not(feature = "platform-i18n"))]
pub type DefaultLocaleResolver = BasicLocaleResolver;

/// The resolver used by [`crate::normalize_locale_tag`] and runtime locale materialization.
///
/// Spelled out per configuration rather than constructing through the alias: a type alias is not
/// usable in expression position, so `DefaultLocaleResolver` as a value would not compile.
#[cfg(feature = "platform-i18n")]
pub fn default_locale_resolver() -> DefaultLocaleResolver {
    PlatformLocaleResolver
}

/// The resolver used by [`crate::normalize_locale_tag`] and runtime locale materialization.
#[cfg(not(feature = "platform-i18n"))]
pub fn default_locale_resolver() -> DefaultLocaleResolver {
    BasicLocaleResolver
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonicalizes_subtag_casing_and_separators() {
        let resolver = BasicLocaleResolver;
        for (input, expected) in [
            ("ru", "ru"),
            ("RU", "ru"),
            ("ru_ru", "ru-RU"),
            ("ru-RU", "ru-RU"),
            ("pt_br", "pt-BR"),
            ("zh-hant", "zh-Hant"),
            ("zh-HANT-tw", "zh-Hant-TW"),
            ("es-419", "es-419"),
            ("de-DE-1996", "de-DE-1996"),
        ] {
            assert_eq!(
                resolver.normalize_tag(input).as_deref(),
                Some(expected),
                "input {input}"
            );
        }
    }

    #[test]
    fn rejects_malformed_tags() {
        let resolver = BasicLocaleResolver;
        for input in ["", "  ", "e", "en-*", "12", "abcd", "en-USA", "en--US"] {
            assert_eq!(resolver.normalize_tag(input), None, "accepted {input}");
        }
    }

    #[test]
    fn normalization_is_idempotent() {
        let resolver = BasicLocaleResolver;
        for input in ["ru_ru", "zh-hant-TW", "es-419", "de-DE-1996", "EN"] {
            let once = resolver.normalize_tag(input).expect("valid");
            let twice = resolver.normalize_tag(&once).expect("valid");
            assert_eq!(once, twice, "not idempotent for {input}");
        }
    }

    #[test]
    fn peels_one_specificity_layer_at_a_time() {
        let resolver = BasicLocaleResolver;
        assert_eq!(
            resolver.fallback_chain("zh-Hant-TW"),
            vec!["zh-Hant-TW", "zh-Hant", "zh"]
        );
        assert_eq!(resolver.fallback_chain("ru-RU"), vec!["ru-RU", "ru"]);
        assert_eq!(resolver.fallback_chain("ru"), vec!["ru"]);
        assert!(resolver.fallback_chain("not a locale").is_empty());
    }

    #[test]
    fn variants_are_dropped_as_one_layer() {
        // Peeling `de-DE-1996` one hyphen at a time would invent `de-DE-199`-style parents, or at
        // best produce a partial-variant tag that no catalog can contain.
        let resolver = BasicLocaleResolver;
        assert_eq!(
            resolver.fallback_chain("de-DE-1996"),
            vec!["de-DE-1996", "de-DE", "de"]
        );
    }

    #[test]
    fn extensions_do_not_affect_catalog_selection() {
        let resolver = BasicLocaleResolver;
        assert_eq!(resolver.normalize_tag("en-US-u-ca-gregory").as_deref(), Some("en-US"));
    }

    /// The two resolvers must agree wherever CLDR data is not involved.
    ///
    /// This is the test that keeps the standalone build honest: if the structural resolver drifts
    /// from the platform one on plain tags, a extracted Fly would silently select different
    /// translations from the same project.
    #[cfg(feature = "platform-i18n")]
    #[test]
    fn both_resolvers_agree_on_tags_that_need_no_cldr_data() {
        let basic = BasicLocaleResolver;
        let platform = PlatformLocaleResolver;
        for input in [
            "ru", "ru_ru", "ru-RU", "pt_br", "zh-Hant", "zh-Hant-TW", "es-419", "en", "en-GB",
            "de-DE-1996",
        ] {
            assert_eq!(
                basic.normalize_tag(input),
                platform.normalize_tag(input),
                "normalization drift for {input}"
            );
        }
        // Chains are compared as a subsequence rather than for equality: the platform resolver
        // may legitimately *insert* an inferred branch (`ru-RU` -> `ru-Cyrl` -> `ru`). What must
        // never happen is that it drops a structural step the basic resolver produces, or orders
        // them differently — that would mean the two disagree about specificity itself.
        for input in ["ru-RU", "zh-Hant-TW", "en", "de-DE-1996", "es-419"] {
            let basic_chain = basic.fallback_chain(input);
            let platform_chain = platform.fallback_chain(input);
            let mut remaining = platform_chain.iter();
            for step in &basic_chain {
                assert!(
                    remaining.any(|candidate| candidate == step),
                    "platform chain {platform_chain:?} does not contain `{step}` from {basic_chain:?} in order"
                );
            }
        }
    }

    /// Both resolvers must reject the same malformed input.
    #[cfg(feature = "platform-i18n")]
    #[test]
    fn both_resolvers_reject_the_same_malformed_tags() {
        let basic = BasicLocaleResolver;
        let platform = PlatformLocaleResolver;
        for input in ["", "e", "en-*", "12", "abcd"] {
            assert_eq!(
                basic.normalize_tag(input).is_none(),
                platform.normalize_tag(input).is_none(),
                "disagreement on {input}"
            );
        }
    }

    /// Document the known divergence rather than pretending it does not exist.
    #[cfg(feature = "platform-i18n")]
    #[test]
    fn only_the_platform_resolver_infers_likely_subtags() {
        let basic = BasicLocaleResolver;
        let platform = PlatformLocaleResolver;
        // `zh-TW` should reach a `zh-Hant` catalog, but that requires CLDR likely-subtag data.
        assert_eq!(basic.fallback_chain("zh-TW"), vec!["zh-TW", "zh"]);
        assert!(
            platform.fallback_chain("zh-TW").contains(&"zh-Hant".to_string()),
            "platform resolver lost likely-subtag inference: {:?}",
            platform.fallback_chain("zh-TW")
        );
    }
}
