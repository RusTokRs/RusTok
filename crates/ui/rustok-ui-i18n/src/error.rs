/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use std::fmt;

/// Errors that can occur when building and parsing a Fluent bundle.
#[derive(Debug)]
pub enum BundleBuildError {
    /// The specified locale string exceeds the supported bounded input length.
    LocaleTooLong {
        length: usize,
        max_len: usize,
    },
    /// The specified locale string failed to parse into a valid LanguageIdentifier.
    InvalidLocale {
        locale: String,
        source: unic_langid::LanguageIdentifierError,
    },
    /// The configured default locale failed to parse into a valid LanguageIdentifier.
    InvalidDefaultLocale {
        locale: String,
        source: unic_langid::LanguageIdentifierError,
    },
    /// The configured default locale has no exact catalog entry.
    MissingDefaultLocale { locale: String },
    /// The FTL resource syntax is invalid.
    FluentParse {
        locale: String,
        errors: Vec<String>,
    },
    /// Failed to add resource to bundle.
    AddResource {
        locale: String,
        errors: Vec<fluent_bundle::FluentError>,
    },
    /// More than one bundle normalized to the same locale key.
    DuplicateLocale { locale: String },
}

impl fmt::Display for BundleBuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LocaleTooLong { length, max_len } => {
                write!(
                    f,
                    "Locale length {length} bytes exceeds the supported maximum of {max_len} bytes"
                )
            }
            Self::InvalidLocale { locale, source } => {
                write!(f, "Invalid locale '{locale}': {source}")
            }
            Self::InvalidDefaultLocale { locale, source } => {
                write!(f, "Invalid default locale '{locale}': {source}")
            }
            Self::MissingDefaultLocale { locale } => {
                write!(f, "Default locale '{locale}' is not present in the Fluent catalog")
            }
            Self::FluentParse { locale, errors } => {
                write!(f, "Fluent parse errors for locale '{locale}': {errors:?}")
            }
            Self::AddResource { locale, errors } => {
                write!(f, "Failed to add resource for locale '{locale}': {errors:?}")
            }
            Self::DuplicateLocale { locale } => {
                write!(f, "Duplicate Fluent catalog locale '{locale}'")
            }
        }
    }
}

impl std::error::Error for BundleBuildError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidLocale { source, .. } | Self::InvalidDefaultLocale { source, .. } => {
                Some(source)
            }
            _ => None,
        }
    }
}

/// High-level i18n operations error.
#[derive(Debug)]
pub enum I18nError {
    /// Bundle construction failed.
    BundleBuild(BundleBuildError),
    /// Message key was not found.
    MessageNotFound {
        locale: String,
        key: String,
    },
    /// Message formatting encountered errors.
    FormattingFailed {
        locale: String,
        key: String,
        errors: Vec<fluent_bundle::FluentError>,
    },
}

impl fmt::Display for I18nError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BundleBuild(err) => write!(f, "Bundle build error: {err}"),
            Self::MessageNotFound { locale, key } => {
                write!(f, "Message '{key}' not found for locale '{locale}'")
            }
            Self::FormattingFailed { locale, key, errors } => {
                write!(f, "Formatting errors for message '{key}' in locale '{locale}': {errors:?}")
            }
        }
    }
}

impl std::error::Error for I18nError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::BundleBuild(err) => Some(err),
            _ => None,
        }
    }
}

impl From<BundleBuildError> for I18nError {
    fn from(err: BundleBuildError) -> Self {
        Self::BundleBuild(err)
    }
}
