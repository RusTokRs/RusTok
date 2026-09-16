/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use std::sync::OnceLock;

use fluent_bundle::FluentArgs;
use unic_langid::LanguageIdentifier;

use crate::bundle::{build_fluent_catalog, try_build_fluent_catalog, FluentCatalog};
use crate::error::{BundleBuildError, I18nError};
use crate::locale::locale_candidates;

/// Ephemeral translator facade over a borrowed `FluentCatalog`.
pub struct UiTranslator<'a> {
    fluent_catalog: &'a FluentCatalog,
    default_locale: &'a str,
}

impl<'a> UiTranslator<'a> {
    pub const fn new(fluent_catalog: &'a FluentCatalog, default_locale: &'a str) -> Self {
        Self {
            fluent_catalog,
            default_locale,
        }
    }

    pub fn try_resolve(
        &self,
        locale: Option<&str>,
        key: &str,
    ) -> Result<String, I18nError> {
        try_resolve_fluent_message(self.fluent_catalog, locale, self.default_locale, key, None)
    }

    pub fn resolve(&self, locale: Option<&str>, key: &str) -> Option<String> {
        resolve_fluent_message(self.fluent_catalog, locale, self.default_locale, key, None)
    }

    pub fn t(&self, locale: Option<&str>, key: &str, fallback: &str) -> String {
        self.format_message(locale, key, None, fallback)
    }

    pub fn try_format_message<'args>(
        &self,
        locale: Option<&str>,
        key: &str,
        args: Option<&FluentArgs<'args>>,
    ) -> Result<String, I18nError> {
        try_resolve_fluent_message(
            self.fluent_catalog,
            locale,
            self.default_locale,
            key,
            args,
        )
    }

    pub fn format_message<'args>(
        &self,
        locale: Option<&str>,
        key: &str,
        args: Option<&FluentArgs<'args>>,
        fallback: &str,
    ) -> String {
        if let Some(msg) =
            resolve_fluent_message(self.fluent_catalog, locale, self.default_locale, key, args)
        {
            return msg;
        }
        fallback.to_string()
    }
}

/// A fail-closed, fully built message catalog for production startup paths.
///
/// `PreparedUiMessages` is constructed with [`UiMessages::prepare`]. Unlike the
/// lazy [`UiMessages::fluent_catalog`] path, construction rejects malformed
/// locale tags, malformed FTL resources, duplicate normalized locales, and an
/// invalid configured default locale before any lookup can occur.
pub struct PreparedUiMessages {
    default_locale: &'static str,
    fluent_catalog: FluentCatalog,
}

impl PreparedUiMessages {
    /// Returns the configured default locale.
    pub const fn default_locale(&self) -> &'static str {
        self.default_locale
    }

    /// Returns the validated Fluent catalog.
    pub const fn fluent_catalog(&self) -> &FluentCatalog {
        &self.fluent_catalog
    }

    /// Borrows this prepared catalog through the common translator facade.
    pub const fn translator(&self) -> UiTranslator<'_> {
        UiTranslator::new(&self.fluent_catalog, self.default_locale)
    }

    /// Strictly resolves and formats a message without applying literal fallback text.
    pub fn try_format<'args>(
        &self,
        locale: Option<&str>,
        key: &str,
        args: Option<&FluentArgs<'args>>,
    ) -> Result<String, I18nError> {
        self.translator().try_format_message(locale, key, args)
    }

    /// Resolves and formats a message with an explicit literal fallback.
    pub fn format<'args>(
        &self,
        locale: Option<&str>,
        key: &str,
        args: Option<&FluentArgs<'args>>,
        fallback: &str,
    ) -> String {
        self.translator()
            .format_message(locale, key, args, fallback)
    }

    /// Resolves a simple translation key with an explicit literal fallback.
    pub fn t(&self, locale: Option<&str>, key: &str, fallback: &str) -> String {
        self.translator().t(locale, key, fallback)
    }
}

/// Primary thread-safe (`Send + Sync`) container for module-owned UI translations.
///
/// Stores compile-time embedded message bundles and lazily initializes concurrent
/// Fluent bundles on first message resolution. The lazy path is intentionally
/// lenient for UI rendering; use [`UiMessages::prepare`] when startup must fail
/// closed on catalog/configuration errors.
pub struct UiMessages {
    default_locale: &'static str,
    bundles: &'static [(&'static str, &'static str)],
    fluent_catalog: OnceLock<FluentCatalog>,
}

impl UiMessages {
    /// Creates a new `UiMessages` instance with static bundle pairs.
    pub const fn new(
        default_locale: &'static str,
        bundles: &'static [(&'static str, &'static str)],
    ) -> Self {
        Self {
            default_locale,
            bundles,
            fluent_catalog: OnceLock::new(),
        }
    }

    /// Strictly validates the configured default locale and every embedded bundle.
    ///
    /// This is intended for tests and CI. Production startup code that wants to
    /// validate once and reuse the exact validated catalog should call [`Self::prepare`].
    pub fn validate(&self) -> Result<(), BundleBuildError> {
        validate_default_locale(self.default_locale)?;
        try_build_fluent_catalog(self.bundles).map(|_| ())
    }

    /// Builds a fail-closed catalog once and returns an owned prepared runtime.
    ///
    /// This avoids the validate-then-rebuild pattern: the returned object serves
    /// lookups from the same strict catalog that passed construction.
    pub fn prepare(&self) -> Result<PreparedUiMessages, BundleBuildError> {
        validate_default_locale(self.default_locale)?;
        let fluent_catalog = try_build_fluent_catalog(self.bundles)?;
        Ok(PreparedUiMessages {
            default_locale: self.default_locale,
            fluent_catalog,
        })
    }

    /// Accesses the underlying lazily-initialized lenient `FluentCatalog`.
    ///
    /// Invalid bundle entries are logged and skipped by this convenience path.
    /// Use [`Self::prepare`] when catalog construction errors must be returned.
    pub fn fluent_catalog(&self) -> &FluentCatalog {
        self.fluent_catalog
            .get_or_init(|| build_fluent_catalog(self.bundles))
    }

    /// Resolves a simple translation key for the specified locale, falling back if not found.
    pub fn t(&self, locale: Option<&str>, key: &str, fallback: &str) -> String {
        self.format(locale, key, None, fallback)
    }

    /// Resolves a simple translation key for the specified locale (alias for `t`).
    pub fn t_for_locale(&self, locale: Option<&str>, key: &str, fallback: &str) -> String {
        self.t(locale, key, fallback)
    }

    /// Strictly resolves and formats a message without applying literal fallback text.
    ///
    /// This method is strict about lookup/formatting but uses the lazily initialized
    /// lenient catalog for backward compatibility. Use [`Self::prepare`] when
    /// bundle construction itself must also be fail-closed.
    pub fn try_format<'args>(
        &self,
        locale: Option<&str>,
        key: &str,
        args: Option<&FluentArgs<'args>>,
    ) -> Result<String, I18nError> {
        try_resolve_fluent_message(
            self.fluent_catalog(),
            locale,
            self.default_locale,
            key,
            args,
        )
    }

    /// Resolves and formats a message with parameters.
    pub fn format<'args>(
        &self,
        locale: Option<&str>,
        key: &str,
        args: Option<&FluentArgs<'args>>,
        fallback: &str,
    ) -> String {
        if let Some(msg) =
            resolve_fluent_message(self.fluent_catalog(), locale, self.default_locale, key, args)
        {
            return msg;
        }

        fallback.to_string()
    }
}

fn validate_default_locale(default_locale: &str) -> Result<(), BundleBuildError> {
    let normalized = default_locale.trim().replace('_', "-");
    normalized
        .parse::<LanguageIdentifier>()
        .map(|_| ())
        .map_err(|source| BundleBuildError::InvalidDefaultLocale {
            locale: default_locale.to_string(),
            source,
        })
}

const STACK_KEY_BUF_SIZE: usize = 128;

/// Executes a closure with a kebab-case representation of `key`.
///
/// If `key` contains '.', replaces '.' with '-' using a fixed stack buffer for
/// keys <= 128 bytes, avoiding any heap allocation in the hot rendering path.
#[inline]
pub fn with_kebab_key<R>(key: &str, f: impl FnOnce(&str) -> R) -> R {
    if !key.contains('.') {
        return f(key);
    }

    if key.len() <= STACK_KEY_BUF_SIZE {
        let mut buf = [0u8; STACK_KEY_BUF_SIZE];
        let bytes = key.as_bytes();
        for (i, &b) in bytes.iter().enumerate() {
            buf[i] = if b == b'.' { b'-' } else { b };
        }
        // SAFETY: '.' (0x2E) and '-' (0x2D) are single-byte ASCII characters.
        // Replacing '.' with '-' in valid UTF-8 maintains valid UTF-8.
        let kebab = unsafe { std::str::from_utf8_unchecked(&buf[..key.len()]) };
        f(kebab)
    } else {
        let kebab = key.replace('.', "-");
        f(&kebab)
    }
}

/// Strictly resolves a message against the `FluentCatalog` using the locale
/// fallback candidate chain.
pub fn try_resolve_fluent_message<'args>(
    catalog: &FluentCatalog,
    locale: Option<&str>,
    default_locale: &str,
    key: &str,
    args: Option<&FluentArgs<'args>>,
) -> Result<String, I18nError> {
    let candidates = locale_candidates(locale, default_locale);

    with_kebab_key(key, |lookup_key| {
        for candidate in candidates {
            if let Some(bundle) = catalog.get(candidate.as_str())
                && let Some(message) = bundle.get_message(lookup_key)
                && let Some(pattern) = message.value()
            {
                let mut errors = vec![];
                let formatted = bundle.format_pattern(pattern, args, &mut errors);
                if !errors.is_empty() {
                    return Err(I18nError::FormattingFailed {
                        locale: candidate,
                        key: key.to_string(),
                        errors,
                    });
                }
                return Ok(formatted.to_string());
            }
        }

        Err(I18nError::MessageNotFound {
            locale: locale.unwrap_or(default_locale).to_string(),
            key: key.to_string(),
        })
    })
}

/// Resolves a message using lenient UI semantics.
///
/// Missing messages return `None`. Formatting failures are logged and also
/// return `None`, allowing the caller to use its explicit literal fallback
/// instead of rendering a partially formatted message.
pub fn resolve_fluent_message<'args>(
    catalog: &FluentCatalog,
    locale: Option<&str>,
    default_locale: &str,
    key: &str,
    args: Option<&FluentArgs<'args>>,
) -> Option<String> {
    match try_resolve_fluent_message(catalog, locale, default_locale, key, args) {
        Ok(message) => Some(message),
        Err(I18nError::MessageNotFound { .. }) => None,
        Err(error) => {
            tracing::warn!(%error, key, "Fluent message resolution failed");
            None
        }
    }
}
