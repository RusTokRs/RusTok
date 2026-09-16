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

use fluent_bundle::concurrent::FluentBundle;
use fluent_bundle::FluentResource;
use unic_langid::LanguageIdentifier;

use crate::error::BundleBuildError;
use crate::locale::MAX_LOCALE_TAG_LEN;

/// A thread-safe, sorted map of normalized locale tags to their concurrent `FluentBundle`.
pub type FluentCatalog = BTreeMap<String, FluentBundle<FluentResource>>;

/// Result of lenient catalog construction with inspectable skipped-entry diagnostics.
///
/// The catalog contains every valid first-wins locale entry. `diagnostics()` keeps
/// the typed reasons for malformed, oversized, duplicate, or otherwise invalid
/// entries that were skipped while preserving the existing fail-soft rendering
/// semantics.
#[must_use = "inspect diagnostics or consume the resulting catalog"]
pub struct FluentCatalogBuildReport {
    catalog: FluentCatalog,
    diagnostics: Vec<BundleBuildError>,
}

impl FluentCatalogBuildReport {
    /// Returns the successfully built lenient catalog.
    pub fn catalog(&self) -> &FluentCatalog {
        &self.catalog
    }

    /// Returns typed diagnostics for every skipped input entry, in input order.
    pub fn diagnostics(&self) -> &[BundleBuildError] {
        &self.diagnostics
    }

    pub(crate) fn push_diagnostic(&mut self, error: BundleBuildError) {
        self.diagnostics.push(error);
    }

    /// Returns whether every supplied catalog entry was accepted.
    pub fn is_clean(&self) -> bool {
        self.diagnostics.is_empty()
    }

    /// Consumes the report and returns only the successfully built catalog.
    pub fn into_catalog(self) -> FluentCatalog {
        self.catalog
    }

    /// Consumes the report and returns both catalog and diagnostics.
    pub fn into_parts(self) -> (FluentCatalog, Vec<BundleBuildError>) {
        (self.catalog, self.diagnostics)
    }
}

fn parse_language_identifier(locale: &str) -> Result<LanguageIdentifier, BundleBuildError> {
    let trimmed = locale.trim();
    if trimmed.len() > MAX_LOCALE_TAG_LEN {
        return Err(BundleBuildError::LocaleTooLong {
            length: trimmed.len(),
            max_len: MAX_LOCALE_TAG_LEN,
        });
    }

    let normalized = trimmed.replace('_', "-");
    normalized
        .parse()
        .map_err(|source| BundleBuildError::InvalidLocale {
            locale: locale.to_string(),
            source,
        })
}

fn build_fluent_bundle_from_parsed(
    langid: LanguageIdentifier,
    normalized_locale: &str,
    ftl_source: &str,
) -> Result<FluentBundle<FluentResource>, BundleBuildError> {
    let mut bundle = FluentBundle::new_concurrent(vec![langid]);
    bundle.set_use_isolating(true);
    let resource = FluentResource::try_new(ftl_source.to_string())
        .map_err(|(_, errors)| BundleBuildError::FluentParse {
            locale: normalized_locale.to_string(),
            errors: errors.into_iter().map(|e| format!("{e:?}")).collect(),
        })?;
    bundle
        .add_resource(resource)
        .map_err(|errors| BundleBuildError::AddResource {
            locale: normalized_locale.to_string(),
            errors,
        })?;
    Ok(bundle)
}

/// Builds a concurrent `FluentBundle` from raw FTL source string.
///
/// Locale tags are normalized before parsing, so underscore-separated tags such
/// as `ru_RU` are accepted consistently with `normalize_locale_tag`. The same
/// bounded locale-input contract used by runtime lookup is enforced before
/// normalization allocates, preventing catalogs that lookup can never select.
/// Oversized-input errors report only lengths and never retain the untrusted
/// locale payload.
///
/// Unicode directional isolation is explicitly enabled. Fluent therefore wraps
/// interpolated values with FSI/PDI markers where appropriate, preventing mixed
/// LTR/RTL arguments from changing the surrounding message direction.
pub fn build_fluent_bundle(
    locale: &str,
    ftl_source: &str,
) -> Result<FluentBundle<FluentResource>, BundleBuildError> {
    let langid = parse_language_identifier(locale)?;
    let normalized = langid.to_string();
    build_fluent_bundle_from_parsed(langid, &normalized, ftl_source)
}

/// Strictly builds an immutable, concurrent `FluentCatalog`.
///
/// Unlike [`build_fluent_catalog`], this function never skips invalid input:
/// malformed/oversized locale tags, invalid FTL resources, and duplicate
/// normalized locale keys are returned to the caller as errors.
pub fn try_build_fluent_catalog(
    bundles: &[(&str, &str)],
) -> Result<FluentCatalog, BundleBuildError> {
    let mut catalog = FluentCatalog::new();

    for (locale, ftl_source) in bundles {
        let langid = parse_language_identifier(locale)?;
        let normalized = langid.to_string();
        if catalog.contains_key(&normalized) {
            return Err(BundleBuildError::DuplicateLocale { locale: normalized });
        }

        let bundle = build_fluent_bundle_from_parsed(langid, &normalized, ftl_source)?;
        catalog.insert(normalized, bundle);
    }

    Ok(catalog)
}

/// Builds an immutable, concurrent `FluentCatalog` using lenient UI semantics.
///
/// This preserves the original convenience API: invalid entries are logged and
/// skipped, duplicate normalized locales are first-wins, and only the usable
/// catalog is returned. Call [`build_fluent_catalog_report`] when the same
/// fail-soft behavior also needs typed, inspectable initialization diagnostics.
/// Call [`try_build_fluent_catalog`] when any invalid input must fail closed.
pub fn build_fluent_catalog(bundles: &[(&str, &str)]) -> FluentCatalog {
    build_fluent_catalog_report(bundles).into_catalog()
}

/// Builds a lenient catalog while retaining typed diagnostics for skipped input.
///
/// The rendering semantics are identical to [`build_fluent_catalog`], including
/// first-wins duplicate handling and tracing diagnostics. Unlike the convenience
/// API, this function also returns every skipped-entry error to the caller so
/// startup health checks and observability code do not have to scrape logs.
pub fn build_fluent_catalog_report(bundles: &[(&str, &str)]) -> FluentCatalogBuildReport {
    let mut catalog = FluentCatalog::new();
    let mut diagnostics = Vec::new();

    for (locale, ftl_source) in bundles {
        let langid = match parse_language_identifier(locale) {
            Ok(langid) => langid,
            Err(error) => {
                match &error {
                    BundleBuildError::LocaleTooLong { length, max_len } => {
                        tracing::error!(
                            length,
                            max_len,
                            "Skipping oversized Fluent locale"
                        );
                    }
                    _ => {
                        tracing::error!(%error, locale = *locale, "Skipping invalid Fluent locale");
                    }
                }
                diagnostics.push(error);
                continue;
            }
        };
        let normalized = langid.to_string();

        if catalog.contains_key(&normalized) {
            let error = BundleBuildError::DuplicateLocale {
                locale: normalized.clone(),
            };
            tracing::error!(%error, locale = normalized, "Skipping duplicate normalized Fluent locale");
            diagnostics.push(error);
            continue;
        }

        match build_fluent_bundle_from_parsed(langid, &normalized, ftl_source) {
            Ok(bundle) => {
                catalog.insert(normalized, bundle);
            }
            Err(error) => {
                tracing::error!(%error, locale = normalized, "Skipping invalid Fluent bundle");
                diagnostics.push(error);
            }
        }
    }

    FluentCatalogBuildReport {
        catalog,
        diagnostics,
    }
}
