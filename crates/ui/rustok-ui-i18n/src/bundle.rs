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
use crate::locale::normalize_locale_tag;

/// A thread-safe, sorted map of normalized locale tags to their concurrent `FluentBundle`.
pub type FluentCatalog = BTreeMap<String, FluentBundle<FluentResource>>;

/// Builds a concurrent `FluentBundle` from raw FTL source string.
///
/// Sets `bundle.set_use_isolating(false)` to generate clean strings without
/// directional isolate characters, matching `@rustok/next-fluent`.
pub fn build_fluent_bundle(
    locale: &str,
    ftl_source: &str,
) -> Result<FluentBundle<FluentResource>, BundleBuildError> {
    let langid: LanguageIdentifier = locale.parse().map_err(|source| BundleBuildError::InvalidLocale {
        locale: locale.to_string(),
        source,
    })?;
    let mut bundle = FluentBundle::new_concurrent(vec![langid]);
    bundle.set_use_isolating(false);
    let resource = FluentResource::try_new(ftl_source.to_string())
        .map_err(|(_, errors)| BundleBuildError::FluentParse {
            locale: locale.to_string(),
            errors: errors.into_iter().map(|e| format!("{e:?}")).collect(),
        })?;
    bundle
        .add_resource(resource)
        .map_err(|errors| BundleBuildError::AddResource {
            locale: locale.to_string(),
            errors,
        })?;
    Ok(bundle)
}

/// Builds an immutable, concurrent `FluentCatalog` from an array of locale/FTL string pairs.
pub fn build_fluent_catalog(bundles: &[(&str, &str)]) -> FluentCatalog {
    let mut catalog = FluentCatalog::new();

    for (locale, ftl_source) in bundles {
        let Some(normalized) = normalize_locale_tag(locale) else {
            continue;
        };

        if let Ok(bundle) = build_fluent_bundle(&normalized, ftl_source) {
            catalog.insert(normalized, bundle);
        }
    }

    catalog
}
