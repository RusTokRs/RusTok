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

fn parse_language_identifier(locale: &str) -> Result<LanguageIdentifier, BundleBuildError> {
    let trimmed = locale.trim();
    if trimmed.len() > MAX_LOCALE_TAG_LEN {
        return Err(BundleBuildError::LocaleTooLong {
            locale: locale.to_string(),
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
/// Invalid, oversized locale/FTL inputs are skipped, but every skipped entry is
/// logged. Duplicate normalized locale keys are also logged and the first entry
/// wins. Call [`try_build_fluent_catalog`] from validation and CI paths that must
/// fail closed on invalid catalogs.
pub fn build_fluent_catalog(bundles: &[(&str, &str)]) -> FluentCatalog {
    let mut catalog = FluentCatalog::new();

    for (locale, ftl_source) in bundles {
        let langid = match parse_language_identifier(locale) {
            Ok(langid) => langid,
            Err(error) => {
                tracing::error!(%error, locale = *locale, "Skipping invalid Fluent locale");
                continue;
            }
        };
        let normalized = langid.to_string();

        if catalog.contains_key(&normalized) {
            tracing::error!(
                locale = normalized,
                "Skipping duplicate normalized Fluent locale"
            );
            continue;
        }

        match build_fluent_bundle_from_parsed(langid, &normalized, ftl_source) {
            Ok(bundle) => {
                catalog.insert(normalized, bundle);
            }
            Err(error) => {
                tracing::error!(%error, locale = normalized, "Skipping invalid Fluent bundle");
            }
        }
    }

    catalog
}
