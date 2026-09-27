/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use std::collections::BTreeMap;
use std::sync::{Once, OnceLock};

use fluent_bundle::FluentArgs;

use crate::bundle::{build_fluent_bundle, parse_language_identifier};
use crate::error::{BundleBuildError, I18nError};
use crate::locale::locale_candidates;
use crate::messages::{
    ConcurrentFluentBundle, MessagePart, PreparedUiMessages, ResolvedMessage, UiMessages,
    effective_locale, normalize_default_locale, resolve_candidates_with_provider,
    try_resolve_candidates_with_provider,
};

struct LazyBundle {
    source: &'static str,
    bundle: OnceLock<Result<ConcurrentFluentBundle, BundleBuildError>>,
    diagnostic_logged: Once,
}

impl LazyBundle {
    const fn new(source: &'static str) -> Self {
        Self {
            source,
            bundle: OnceLock::new(),
            diagnostic_logged: Once::new(),
        }
    }

    fn load(&self, locale: &str) -> Option<&ConcurrentFluentBundle> {
        let result = self
            .bundle
            .get_or_init(|| build_fluent_bundle(locale, self.source));

        if let Err(error) = result {
            self.diagnostic_logged.call_once(|| {
                tracing::error!(%error, locale, "Skipping invalid lazily loaded Fluent bundle");
            });
        }

        result.as_ref().ok()
    }

    fn loaded_error(&self) -> Option<&BundleBuildError> {
        self.bundle
            .get()
            .and_then(|result| result.as_ref().err())
    }

    fn is_loaded(&self) -> bool {
        self.bundle.get().is_some_and(Result::is_ok)
    }
}

struct LazyCatalogIndex {
    entries: BTreeMap<String, LazyBundle>,
    diagnostics: Vec<BundleBuildError>,
}

impl LazyCatalogIndex {
    fn new(
        default_locale: &str,
        bundles: &'static [(&'static str, &'static str)],
    ) -> Self {
        let mut entries = BTreeMap::new();
        let mut diagnostics = Vec::new();

        for &(locale, source) in bundles {
            match parse_language_identifier(locale) {
                Ok(langid) => {
                    let normalized = langid.to_string();
                    if entries.contains_key(&normalized) {
                        diagnostics.push(BundleBuildError::DuplicateLocale {
                            locale: normalized,
                        });
                    } else {
                        entries.insert(normalized, LazyBundle::new(source));
                    }
                }
                Err(error) => diagnostics.push(error),
            }
        }

        match normalize_default_locale(default_locale) {
            Ok(normalized) if !entries.contains_key(&normalized) => {
                diagnostics.push(BundleBuildError::MissingDefaultLocale { locale: normalized });
            }
            Ok(_) => {}
            Err(error) => diagnostics.push(error),
        }

        Self {
            entries,
            diagnostics,
        }
    }
}

/// A thread-safe embedded Fluent catalog that parses each locale bundle on first use.
///
/// Locale declarations are indexed and canonicalized together, but FTL resources are
/// not parsed until their locale appears in a lookup fallback chain. This keeps startup
/// parsing and resident bundle state proportional to the locales actually requested.
/// The embedded `include_str!` bytes still contribute to native/WASM binary size; hosts
/// that need downloadable catalogs require a separate storage/loading adapter.
///
/// Lazy resolution is intentionally fail-soft, matching [`UiMessages`]: malformed
/// bundles are skipped and lookup continues to the next candidate. Call [`Self::validate`]
/// in CI or [`Self::prepare`] at fail-closed startup to validate every embedded locale.
pub struct LazyUiMessages {
    default_locale: &'static str,
    bundles: &'static [(&'static str, &'static str)],
    index: OnceLock<LazyCatalogIndex>,
    diagnostics_logged: Once,
}

impl std::fmt::Debug for LazyUiMessages {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LazyUiMessages")
            .field("default_locale", &self.default_locale)
            .field(
                "declared_locales",
                &self.available_locales().collect::<Vec<_>>(),
            )
            .field(
                "loaded_locales",
                &self.loaded_locales().collect::<Vec<_>>(),
            )
            .finish()
    }
}

impl LazyUiMessages {
    /// Creates a locale-lazy catalog from compile-time embedded locale/source pairs.
    pub const fn new(
        default_locale: &'static str,
        bundles: &'static [(&'static str, &'static str)],
    ) -> Self {
        Self {
            default_locale,
            bundles,
            index: OnceLock::new(),
            diagnostics_logged: Once::new(),
        }
    }

    fn index(&self) -> &LazyCatalogIndex {
        let index = self
            .index
            .get_or_init(|| LazyCatalogIndex::new(self.default_locale, self.bundles));

        self.diagnostics_logged.call_once(|| {
            for diagnostic in &index.diagnostics {
                tracing::error!(%diagnostic, "Invalid lazy Fluent catalog declaration");
            }
        });

        index
    }

    fn bundle(&self, locale: &str) -> Option<&ConcurrentFluentBundle> {
        self.index().entries.get(locale)?.load(locale)
    }

    fn try_resolve_part<'args>(
        &self,
        locale: Option<&str>,
        key: &str,
        part: MessagePart<'_>,
        args: Option<&FluentArgs<'args>>,
    ) -> Result<ResolvedMessage, I18nError> {
        let candidates = locale_candidates(locale, self.default_locale);
        try_resolve_candidates_with_provider(
            &candidates,
            effective_locale(&candidates),
            key,
            part,
            args,
            |candidate| self.bundle(candidate),
        )
    }

    fn resolve_part<'args>(
        &self,
        locale: Option<&str>,
        key: &str,
        part: MessagePart<'_>,
        args: Option<&FluentArgs<'args>>,
    ) -> Option<ResolvedMessage> {
        let candidates = locale_candidates(locale, self.default_locale);
        resolve_candidates_with_provider(&candidates, key, part, args, |candidate| {
            self.bundle(candidate)
        })
    }

    /// Strictly validates all embedded locale declarations, resources, and schemas.
    pub fn validate(&self) -> Result<(), BundleBuildError> {
        UiMessages::new(self.default_locale, self.bundles).validate()
    }

    /// Builds an eager fail-closed catalog after validating every embedded locale.
    pub fn prepare(&self) -> Result<PreparedUiMessages, BundleBuildError> {
        UiMessages::new(self.default_locale, self.bundles).prepare()
    }

    /// Iterates canonical declared locale identities without parsing their FTL resources.
    ///
    /// A locale can still fail when first loaded if its resource is malformed. Use
    /// [`Self::validate`] when every declaration must be proven usable beforehand.
    pub fn available_locales(&self) -> impl ExactSizeIterator<Item = &str> {
        self.index().entries.keys().map(String::as_str)
    }

    /// Iterates locales whose Fluent resources have already loaded successfully.
    pub fn loaded_locales(&self) -> impl Iterator<Item = &str> {
        self.index()
            .entries
            .iter()
            .filter(|(_, entry)| entry.is_loaded())
            .map(|(locale, _)| locale.as_str())
    }

    /// Returns declaration/default diagnostics discovered without parsing FTL resources.
    pub fn initialization_diagnostics(&self) -> &[BundleBuildError] {
        &self.index().diagnostics
    }

    /// Iterates errors from locale bundles whose lazy initialization was attempted.
    pub fn loaded_bundle_diagnostics(&self) -> impl Iterator<Item = &BundleBuildError> {
        self.index()
            .entries
            .values()
            .filter_map(LazyBundle::loaded_error)
    }

    /// Precomputes one locale fallback chain while retaining per-locale lazy loading.
    pub fn for_locale(&self, locale: Option<&str>) -> LazyUiLocaleTranslator<'_> {
        LazyUiLocaleTranslator {
            messages: self,
            candidates: locale_candidates(locale, self.default_locale),
        }
    }

    /// Resolves a simple translation with an explicit literal fallback.
    pub fn t(&self, locale: Option<&str>, key: &str, fallback: &str) -> String {
        self.format(locale, key, None, fallback)
    }

    /// Alias for [`Self::t`] retained for module-facade symmetry.
    pub fn t_for_locale(&self, locale: Option<&str>, key: &str, fallback: &str) -> String {
        self.t(locale, key, fallback)
    }

    /// Strictly resolves and formats a message.
    pub fn try_format<'args>(
        &self,
        locale: Option<&str>,
        key: &str,
        args: Option<&FluentArgs<'args>>,
    ) -> Result<String, I18nError> {
        self.try_resolve_part(locale, key, MessagePart::Value, args)
            .map(ResolvedMessage::into_value)
    }

    /// Strictly formats a message and preserves its canonical source locale.
    pub fn try_format_with_locale<'args>(
        &self,
        locale: Option<&str>,
        key: &str,
        args: Option<&FluentArgs<'args>>,
    ) -> Result<ResolvedMessage, I18nError> {
        self.try_resolve_part(locale, key, MessagePart::Value, args)
    }

    /// Strictly resolves and formats a Fluent message attribute.
    pub fn try_format_attribute<'args>(
        &self,
        locale: Option<&str>,
        key: &str,
        attribute: &str,
        args: Option<&FluentArgs<'args>>,
    ) -> Result<String, I18nError> {
        self.try_resolve_part(locale, key, MessagePart::Attribute(attribute), args)
            .map(ResolvedMessage::into_value)
    }

    /// Resolves a message with an explicit literal fallback.
    pub fn format<'args>(
        &self,
        locale: Option<&str>,
        key: &str,
        args: Option<&FluentArgs<'args>>,
        fallback: &str,
    ) -> String {
        self.resolve_part(locale, key, MessagePart::Value, args)
            .map(ResolvedMessage::into_value)
            .unwrap_or_else(|| fallback.to_string())
    }

    /// Resolves a Fluent attribute with an explicit literal fallback.
    pub fn format_attribute<'args>(
        &self,
        locale: Option<&str>,
        key: &str,
        attribute: &str,
        args: Option<&FluentArgs<'args>>,
        fallback: &str,
    ) -> String {
        self.resolve_part(locale, key, MessagePart::Attribute(attribute), args)
            .map(ResolvedMessage::into_value)
            .unwrap_or_else(|| fallback.to_string())
    }
}

/// A borrowed lazy catalog bound to one precomputed locale fallback chain.
pub struct LazyUiLocaleTranslator<'a> {
    messages: &'a LazyUiMessages,
    candidates: Vec<String>,
}

impl<'a> LazyUiLocaleTranslator<'a> {
    /// Returns the canonical fallback candidates reused by this translator.
    pub fn candidates(&self) -> &[String] {
        &self.candidates
    }

    fn try_resolve_part<'args>(
        &self,
        key: &str,
        part: MessagePart<'_>,
        args: Option<&FluentArgs<'args>>,
    ) -> Result<ResolvedMessage, I18nError> {
        try_resolve_candidates_with_provider(
            &self.candidates,
            effective_locale(&self.candidates),
            key,
            part,
            args,
            |candidate| self.messages.bundle(candidate),
        )
    }

    fn resolve_part<'args>(
        &self,
        key: &str,
        part: MessagePart<'_>,
        args: Option<&FluentArgs<'args>>,
    ) -> Option<ResolvedMessage> {
        resolve_candidates_with_provider(&self.candidates, key, part, args, |candidate| {
            self.messages.bundle(candidate)
        })
    }

    pub fn try_resolve(&self, key: &str) -> Result<String, I18nError> {
        self.try_format(key, None)
    }

    pub fn resolve(&self, key: &str) -> Option<String> {
        self.resolve_part(key, MessagePart::Value, None)
            .map(ResolvedMessage::into_value)
    }

    /// Resolves a message and preserves its canonical source locale.
    pub fn resolve_with_locale(&self, key: &str) -> Option<ResolvedMessage> {
        self.resolve_part(key, MessagePart::Value, None)
    }

    pub fn t(&self, key: &str, fallback: &str) -> String {
        self.format(key, None, fallback)
    }

    pub fn try_format<'args>(
        &self,
        key: &str,
        args: Option<&FluentArgs<'args>>,
    ) -> Result<String, I18nError> {
        self.try_resolve_part(key, MessagePart::Value, args)
            .map(ResolvedMessage::into_value)
    }

    /// Strictly formats a message and preserves its canonical source locale.
    pub fn try_format_with_locale<'args>(
        &self,
        key: &str,
        args: Option<&FluentArgs<'args>>,
    ) -> Result<ResolvedMessage, I18nError> {
        self.try_resolve_part(key, MessagePart::Value, args)
    }

    pub fn try_format_attribute<'args>(
        &self,
        key: &str,
        attribute: &str,
        args: Option<&FluentArgs<'args>>,
    ) -> Result<String, I18nError> {
        self.try_resolve_part(key, MessagePart::Attribute(attribute), args)
            .map(ResolvedMessage::into_value)
    }

    pub fn format<'args>(
        &self,
        key: &str,
        args: Option<&FluentArgs<'args>>,
        fallback: &str,
    ) -> String {
        self.resolve_part(key, MessagePart::Value, args)
            .map(ResolvedMessage::into_value)
            .unwrap_or_else(|| fallback.to_string())
    }

    pub fn format_attribute<'args>(
        &self,
        key: &str,
        attribute: &str,
        args: Option<&FluentArgs<'args>>,
        fallback: &str,
    ) -> String {
        self.resolve_part(key, MessagePart::Attribute(attribute), args)
            .map(ResolvedMessage::into_value)
            .unwrap_or_else(|| fallback.to_string())
    }
}
