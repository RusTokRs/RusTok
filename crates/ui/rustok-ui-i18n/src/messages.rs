/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Once, OnceLock};

use fluent_bundle::FluentArgs;
use fluent_syntax::ast;
use unic_langid::LanguageIdentifier;

use crate::bundle::{
    FluentCatalog, FluentCatalogBuildReport, build_fluent_catalog_report,
    parse_language_identifier, try_build_fluent_catalog,
};
use crate::error::{BundleBuildError, I18nError, MessageKeyError};
use crate::locale::{MAX_LOCALE_TAG_LEN, locale_candidates};

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

    /// Prepares one locale fallback chain for reuse across multiple message lookups.
    ///
    /// Prefer this in request/render scopes that resolve many keys for the same
    /// effective locale. It avoids reparsing the locale and reallocating the
    /// fallback candidate vector on every lookup.
    pub fn for_locale(&self, locale: Option<&str>) -> UiLocaleTranslator<'a> {
        UiLocaleTranslator::new(self.fluent_catalog, locale, self.default_locale)
    }

    pub fn try_resolve(&self, locale: Option<&str>, key: &str) -> Result<String, I18nError> {
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
        try_resolve_fluent_message(self.fluent_catalog, locale, self.default_locale, key, args)
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

/// Translator bound to one effective locale with a precomputed fallback chain.
///
/// Construction performs locale normalization and candidate allocation once;
/// subsequent key lookups reuse the stored candidates. The effective diagnostic
/// locale is borrowed from that same candidate chain, avoiding a duplicate owned
/// `String` per prepared translator.
pub struct UiLocaleTranslator<'a> {
    fluent_catalog: &'a FluentCatalog,
    candidates: Vec<String>,
}

impl<'a> UiLocaleTranslator<'a> {
    pub fn new(
        fluent_catalog: &'a FluentCatalog,
        locale: Option<&str>,
        default_locale: &str,
    ) -> Self {
        Self {
            fluent_catalog,
            candidates: locale_candidates(locale, default_locale),
        }
    }

    /// Returns the normalized fallback candidates reused by this translator.
    pub fn candidates(&self) -> &[String] {
        &self.candidates
    }

    pub fn try_resolve(&self, key: &str) -> Result<String, I18nError> {
        try_resolve_fluent_candidates(
            self.fluent_catalog,
            &self.candidates,
            effective_locale(&self.candidates),
            key,
            None,
        )
    }

    pub fn resolve(&self, key: &str) -> Option<String> {
        resolve_fluent_candidates(self.fluent_catalog, &self.candidates, key, None)
    }

    pub fn t(&self, key: &str, fallback: &str) -> String {
        self.format(key, None, fallback)
    }

    pub fn try_format<'args>(
        &self,
        key: &str,
        args: Option<&FluentArgs<'args>>,
    ) -> Result<String, I18nError> {
        try_resolve_fluent_candidates(
            self.fluent_catalog,
            &self.candidates,
            effective_locale(&self.candidates),
            key,
            args,
        )
    }

    pub fn format<'args>(
        &self,
        key: &str,
        args: Option<&FluentArgs<'args>>,
        fallback: &str,
    ) -> String {
        resolve_fluent_candidates(self.fluent_catalog, &self.candidates, key, args)
            .unwrap_or_else(|| fallback.to_string())
    }
}

/// A fail-closed, fully built message catalog for production startup paths.
///
/// `PreparedUiMessages` is constructed with [`UiMessages::prepare`]. Unlike the
/// lazy [`UiMessages::fluent_catalog`] path, construction rejects malformed
/// locale tags, malformed FTL resources, duplicate normalized locales, an
/// invalid configured default locale, a default locale without an exact
/// normalized catalog entry, and schema mismatches between locales before
/// any lookup can occur.
pub struct PreparedUiMessages {
    default_locale: String,
    fluent_catalog: FluentCatalog,
}

impl std::fmt::Debug for PreparedUiMessages {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PreparedUiMessages")
            .field("default_locale", &self.default_locale)
            .field("locales", &self.fluent_catalog.keys().collect::<Vec<_>>())
            .finish()
    }
}

impl PreparedUiMessages {
    /// Returns the validated and normalized default locale tag.
    pub fn default_locale(&self) -> &str {
        &self.default_locale
    }

    /// Returns the validated Fluent catalog.
    pub const fn fluent_catalog(&self) -> &FluentCatalog {
        &self.fluent_catalog
    }

    /// Borrows this prepared catalog through the common translator facade.
    pub fn translator(&self) -> UiTranslator<'_> {
        UiTranslator::new(&self.fluent_catalog, &self.default_locale)
    }

    /// Prepares one effective locale for repeated lookups against this validated catalog.
    pub fn for_locale(&self, locale: Option<&str>) -> UiLocaleTranslator<'_> {
        UiLocaleTranslator::new(&self.fluent_catalog, locale, &self.default_locale)
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

/// Message schema representing the set of variables used by a message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageSchema {
    /// Ordered, deduplicated set of variable identifier names required by this message.
    pub variables: BTreeSet<String>,
}

fn collect_pattern_variables(pattern: &ast::Pattern<&str>, out: &mut BTreeSet<String>) {
    for element in &pattern.elements {
        if let ast::PatternElement::Placeable { expression } = element {
            collect_expression_variables(expression, out);
        }
    }
}

fn collect_expression_variables(expr: &ast::Expression<&str>, out: &mut BTreeSet<String>) {
    match expr {
        ast::Expression::Inline(inline) => collect_inline_variables(inline, out),
        ast::Expression::Select { selector, variants } => {
            collect_inline_variables(selector, out);
            for variant in variants {
                collect_pattern_variables(&variant.value, out);
            }
        }
    }
}

fn collect_inline_variables(inline: &ast::InlineExpression<&str>, out: &mut BTreeSet<String>) {
    match inline {
        ast::InlineExpression::VariableReference { id } => {
            out.insert(id.name.to_string());
        }
        ast::InlineExpression::Placeable { expression } => {
            collect_expression_variables(expression, out);
        }
        ast::InlineExpression::FunctionReference { arguments, .. } => {
            for pos in &arguments.positional {
                collect_inline_variables(pos, out);
            }
            for named in &arguments.named {
                collect_inline_variables(&named.value, out);
            }
        }
        ast::InlineExpression::TermReference {
            arguments: Some(args),
            ..
        } => {
            for pos in &args.positional {
                collect_inline_variables(pos, out);
            }
            for named in &args.named {
                collect_inline_variables(&named.value, out);
            }
        }
        _ => {}
    }
}

fn collect_message_variables(msg: &ast::Message<&str>) -> BTreeSet<String> {
    let mut vars = BTreeSet::new();
    if let Some(ref pattern) = msg.value {
        collect_pattern_variables(pattern, &mut vars);
    }
    for attr in &msg.attributes {
        collect_pattern_variables(&attr.value, &mut vars);
    }
    vars
}

/// Parses an FTL resource and extracts the variable schema for each message entry.
pub fn extract_locale_schemas(
    locale: &str,
    ftl_source: &str,
) -> Result<BTreeMap<String, MessageSchema>, BundleBuildError> {
    let resource = fluent_syntax::parser::parse(ftl_source).map_err(|(_, errors)| {
        BundleBuildError::FluentParse {
            locale: locale.to_string(),
            errors: errors.into_iter().map(|e| format!("{e:?}")).collect(),
        }
    })?;

    let mut schemas = BTreeMap::new();
    for entry in resource.body {
        if let ast::Entry::Message(msg) = entry {
            let variables = collect_message_variables(&msg);
            schemas.insert(msg.id.name.to_string(), MessageSchema { variables });
        }
    }
    Ok(schemas)
}

/// Validates that message schemas across all bundles match the default locale schema.
///
/// Invariants enforced:
/// 1. Messages present in both the default locale and a non-default locale must use the exact same variable names.
/// 2. Non-default locales must not declare extra messages that are missing from the default locale catalog.
pub fn validate_catalog_schemas(
    bundles: &[(&str, &str)],
    default_locale: &str,
) -> Result<(), BundleBuildError> {
    let mut locale_schemas: BTreeMap<String, BTreeMap<String, MessageSchema>> = BTreeMap::new();

    for (locale_tag, ftl_source) in bundles {
        let langid = parse_language_identifier(locale_tag)?;
        let normalized = langid.to_string();
        let schemas = extract_locale_schemas(&normalized, ftl_source)?;
        locale_schemas.insert(normalized, schemas);
    }

    let default_schemas = match locale_schemas.get(default_locale) {
        Some(schemas) => schemas,
        None => {
            return Err(BundleBuildError::MissingDefaultLocale {
                locale: default_locale.to_string(),
            });
        }
    };

    for (locale, schemas) in &locale_schemas {
        if locale == default_locale {
            continue;
        }

        // Check for extra messages in non-default locale that default locale lacks
        for msg_id in schemas.keys() {
            if !default_schemas.contains_key(msg_id) {
                return Err(BundleBuildError::ExtraMessage {
                    locale: locale.clone(),
                    message: msg_id.clone(),
                });
            }
        }

        // Check for variable parity for messages that exist in both
        for (msg_id, default_schema) in default_schemas {
            if let Some(locale_schema) = schemas.get(msg_id)
                && locale_schema.variables != default_schema.variables
            {
                return Err(BundleBuildError::MessageSchemaMismatch {
                    locale: locale.clone(),
                    message: msg_id.clone(),
                    expected: default_schema.variables.iter().cloned().collect(),
                    actual: locale_schema.variables.iter().cloned().collect(),
                });
            }
        }
    }

    Ok(())
}

/// Primary thread-safe (`Send + Sync`) container for module-owned UI translations.
///
/// Stores compile-time embedded message bundles and lazily initializes one
/// lenient catalog build report on first message resolution or diagnostic access.
/// The cached report owns both the usable concurrent Fluent catalog and typed
/// skipped-entry/configuration diagnostics. Use [`UiMessages::prepare`] when
/// startup must fail closed on catalog/configuration errors.
pub struct UiMessages {
    default_locale: &'static str,
    bundles: &'static [(&'static str, &'static str)],
    fluent_catalog: OnceLock<FluentCatalogBuildReport>,
    diagnostics_logged: Once,
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
            diagnostics_logged: Once::new(),
        }
    }

    /// Strictly validates the configured default locale and every embedded bundle.
    ///
    /// The normalized default locale must also have an exact catalog entry, matching
    /// the `@rustok/next-fluent` configuration invariant that `defaultLocale` is one
    /// of the configured locales. Additionally, cross-locale message schemas are
    /// validated for variable parity against the default locale.
    ///
    /// This is intended for tests and CI. Production startup code that wants to
    /// validate once and reuse the exact validated catalog should call [`Self::prepare`].
    pub fn validate(&self) -> Result<(), BundleBuildError> {
        let default_locale = normalize_default_locale(self.default_locale)?;
        let fluent_catalog = try_build_fluent_catalog(self.bundles)?;
        ensure_default_locale_present(&fluent_catalog, &default_locale)?;
        validate_catalog_schemas(self.bundles, &default_locale)
    }

    /// Builds a fail-closed catalog once and returns an owned prepared runtime.
    ///
    /// This avoids the validate-then-rebuild pattern: the returned object serves
    /// lookups from the same strict catalog that passed construction. The normalized
    /// default locale must be present in that exact catalog, and message schemas
    /// must match across all locales.
    pub fn prepare(&self) -> Result<PreparedUiMessages, BundleBuildError> {
        let default_locale = normalize_default_locale(self.default_locale)?;
        let fluent_catalog = try_build_fluent_catalog(self.bundles)?;
        ensure_default_locale_present(&fluent_catalog, &default_locale)?;
        validate_catalog_schemas(self.bundles, &default_locale)?;
        Ok(PreparedUiMessages {
            default_locale,
            fluent_catalog,
        })
    }

    fn fluent_catalog_report(&self) -> &FluentCatalogBuildReport {
        let report = self.fluent_catalog.get_or_init(|| {
            let mut report = build_fluent_catalog_report(self.bundles);

            match normalize_default_locale(self.default_locale) {
                Ok(default_locale) => {
                    if let Err(error) =
                        ensure_default_locale_present(report.catalog(), &default_locale)
                    {
                        report.push_diagnostic(error);
                    }
                }
                Err(error) => {
                    report.push_diagnostic(error);
                }
            }

            report
        });

        self.diagnostics_logged.call_once(|| {
            for diagnostic in report.diagnostics() {
                match diagnostic {
                    BundleBuildError::LocaleTooLong { length, max_len } => {
                        tracing::error!(length, max_len, "Skipping oversized Fluent locale");
                    }
                    BundleBuildError::InvalidLocale { locale, .. } => {
                        tracing::error!(%diagnostic, locale = locale.as_str(), "Skipping invalid Fluent locale");
                    }
                    BundleBuildError::DuplicateLocale { locale } => {
                        tracing::error!(%diagnostic, locale = locale.as_str(), "Skipping duplicate normalized Fluent locale");
                    }
                    BundleBuildError::MissingDefaultLocale { locale } => {
                        tracing::error!(%diagnostic, default_locale = locale.as_str(), "Configured Fluent default locale is absent from the usable catalog");
                    }
                    BundleBuildError::InvalidDefaultLocale { locale, .. } => {
                        tracing::error!(%diagnostic, default_locale = locale.as_str(), "Configured Fluent default locale is invalid");
                    }
                    _ => {
                        tracing::error!(%diagnostic, "Skipping invalid Fluent bundle entry");
                    }
                }
            }
        });

        report
    }

    /// Accesses the underlying lazily initialized lenient `FluentCatalog`.
    ///
    /// Invalid bundle entries are logged and skipped by this convenience path.
    /// The same one-time initialization also retains typed entry/configuration
    /// diagnostics, available through [`Self::initialization_diagnostics`]. Use
    /// [`Self::prepare`] when catalog construction errors must fail closed instead.
    pub fn fluent_catalog(&self) -> &FluentCatalog {
        self.fluent_catalog_report().catalog()
    }

    /// Returns typed diagnostics retained by the lazy lenient initialization.
    ///
    /// Diagnostics include both skipped catalog entries and invalid/missing default
    /// locale configuration. Calling this before the first lookup triggers the same
    /// one-time `OnceLock` initialization used by [`Self::fluent_catalog`]; it never
    /// rebuilds the catalog solely to recover diagnostics. The returned slice remains
    /// stable for the lifetime of this `UiMessages` value.
    pub fn initialization_diagnostics(&self) -> &[BundleBuildError] {
        self.fluent_catalog_report().diagnostics()
    }

    /// Prepares one effective locale for repeated lookups through the lazy catalog.
    pub fn for_locale(&self, locale: Option<&str>) -> UiLocaleTranslator<'_> {
        UiLocaleTranslator::new(self.fluent_catalog(), locale, self.default_locale)
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
        if let Some(msg) = resolve_fluent_message(
            self.fluent_catalog(),
            locale,
            self.default_locale,
            key,
            args,
        ) {
            return msg;
        }

        fallback.to_string()
    }
}

fn normalize_default_locale(default_locale: &str) -> Result<String, BundleBuildError> {
    if default_locale.len() > MAX_LOCALE_TAG_LEN {
        return Err(BundleBuildError::LocaleTooLong {
            length: default_locale.len(),
            max_len: MAX_LOCALE_TAG_LEN,
        });
    }

    let trimmed = default_locale.trim();
    let normalized = trimmed.replace('_', "-");
    normalized
        .parse::<LanguageIdentifier>()
        .map(|langid| langid.to_string())
        .map_err(|source| BundleBuildError::InvalidDefaultLocale {
            locale: default_locale.to_string(),
            source,
        })
}

fn ensure_default_locale_present(
    fluent_catalog: &FluentCatalog,
    default_locale: &str,
) -> Result<(), BundleBuildError> {
    if fluent_catalog.contains_key(default_locale) {
        Ok(())
    } else {
        Err(BundleBuildError::MissingDefaultLocale {
            locale: default_locale.to_string(),
        })
    }
}

#[inline]
fn effective_locale(candidates: &[String]) -> &str {
    candidates.first().map(String::as_str).unwrap_or("en")
}

const STACK_KEY_BUF_SIZE: usize = 128;

/// Executes a closure with a kebab-case representation of `key`.
///
/// If `key` contains '.', replaces '.' with '-' using a fixed stack buffer for
/// keys <= 128 bytes, avoiding heap allocation on the normal stack path. A safe
/// UTF-8 validation guards the stack slice; an unexpected validation failure
/// falls back to the ordinary allocating replacement instead of invoking `unsafe`.
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

        let Ok(kebab) = std::str::from_utf8(&buf[..key.len()]) else {
            let kebab = key.replace('.', "-");
            return f(&kebab);
        };
        return f(kebab);
    }

    let kebab = key.replace('.', "-");
    f(&kebab)
}

/// Maximum supported byte length for message keys.
pub const MAX_MESSAGE_KEY_LEN: usize = 256;

/// Validates that a message key is non-empty, within length bounds, and free of control or NUL characters.
pub fn validate_message_key(key: &str) -> Result<(), I18nError> {
    if key.is_empty() {
        return Err(I18nError::InvalidMessageKey {
            key: String::new(),
            reason: MessageKeyError::Empty,
        });
    }

    if key.len() > MAX_MESSAGE_KEY_LEN {
        return Err(I18nError::InvalidMessageKey {
            key: truncate_for_diagnostic(key, 128),
            reason: MessageKeyError::TooLong {
                length: key.len(),
                max_len: MAX_MESSAGE_KEY_LEN,
            },
        });
    }

    if key.bytes().any(|b| b.is_ascii_control() || b == 0) {
        return Err(I18nError::InvalidMessageKey {
            key: truncate_for_diagnostic(key, 128),
            reason: MessageKeyError::InvalidCharacters,
        });
    }

    Ok(())
}

fn truncate_for_diagnostic(s: &str, max_bytes: usize) -> String {
    if s.len() <= max_bytes {
        s.to_string()
    } else {
        let mut truncated = String::with_capacity(max_bytes + 3);
        for c in s.chars() {
            if truncated.len() + c.len_utf8() > max_bytes {
                break;
            }
            truncated.push(c);
        }
        truncated.push_str("...");
        truncated
    }
}

enum LookupResult {
    Found(String),
    Missing,
    Failed(I18nError),
}

fn lookup_fluent_candidates<'args>(
    catalog: &FluentCatalog,
    candidates: &[String],
    key: &str,
    args: Option<&FluentArgs<'args>>,
) -> LookupResult {
    if let Err(err) = validate_message_key(key) {
        return LookupResult::Failed(err);
    }

    with_kebab_key(key, |lookup_key| {
        for candidate in candidates {
            if let Some(bundle) = catalog.get(candidate.as_str())
                && let Some(message) = bundle.get_message(lookup_key)
                && let Some(pattern) = message.value()
            {
                let mut errors = vec![];
                let formatted = bundle.format_pattern(pattern, args, &mut errors);
                if !errors.is_empty() {
                    return LookupResult::Failed(I18nError::FormattingFailed {
                        locale: candidate.clone(),
                        key: key.to_string(),
                        errors,
                    });
                }
                return LookupResult::Found(formatted.to_string());
            }
        }

        LookupResult::Missing
    })
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
    try_resolve_fluent_candidates(
        catalog,
        &candidates,
        effective_locale(&candidates),
        key,
        args,
    )
}

fn try_resolve_fluent_candidates<'args>(
    catalog: &FluentCatalog,
    candidates: &[String],
    effective_locale: &str,
    key: &str,
    args: Option<&FluentArgs<'args>>,
) -> Result<String, I18nError> {
    match lookup_fluent_candidates(catalog, candidates, key, args) {
        LookupResult::Found(msg) => Ok(msg),
        LookupResult::Missing => Err(I18nError::MessageNotFound {
            locale: effective_locale.to_string(),
            key: key.to_string(),
        }),
        LookupResult::Failed(err) => Err(err),
    }
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
    let candidates = locale_candidates(locale, default_locale);
    resolve_fluent_candidates(catalog, &candidates, key, args)
}

fn resolve_fluent_candidates<'args>(
    catalog: &FluentCatalog,
    candidates: &[String],
    key: &str,
    args: Option<&FluentArgs<'args>>,
) -> Option<String> {
    match lookup_fluent_candidates(catalog, candidates, key, args) {
        LookupResult::Found(message) => Some(message),
        LookupResult::Missing => None,
        LookupResult::Failed(error) => {
            tracing::warn!(%error, key = %truncate_for_diagnostic(key, 128), "Fluent message resolution failed");
            None
        }
    }
}
