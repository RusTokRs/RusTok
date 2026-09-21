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

/// Reason why a message key failed validation.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MessageKeyError {
    /// The message key was empty.
    Empty,
    /// The message key length exceeded the maximum supported byte length.
    TooLong { length: usize, max_len: usize },
    /// The message key contains control characters or NUL bytes.
    InvalidCharacters,
}

impl fmt::Display for MessageKeyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "Message key cannot be empty"),
            Self::TooLong { length, max_len } => {
                write!(
                    f,
                    "Message key length {length} bytes exceeds the maximum supported of {max_len} bytes"
                )
            }
            Self::InvalidCharacters => {
                write!(f, "Message key contains control characters or NUL bytes")
            }
        }
    }
}

/// Errors that can occur when building and parsing a Fluent bundle.
///
/// This enum is non-exhaustive so new diagnostics can be added without forcing
/// downstream consumers to update exhaustive matches. Match the variants you
/// need and keep a wildcard arm for forward compatibility.
#[non_exhaustive]
#[derive(Debug)]
pub enum BundleBuildError {
    /// The specified locale string exceeds the supported bounded input length.
    LocaleTooLong { length: usize, max_len: usize },
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
    FluentParse { locale: String, errors: Vec<String> },
    /// Failed to add resource to bundle.
    AddResource {
        locale: String,
        errors: Vec<fluent_bundle::FluentError>,
    },
    /// More than one bundle normalized to the same locale key.
    DuplicateLocale { locale: String },
    /// Message in a non-default locale has a different variable set than the default locale.
    MessageSchemaMismatch {
        locale: String,
        message: String,
        expected: Vec<String>,
        actual: Vec<String>,
    },
    /// Message in a non-default locale does not exist in the default locale schema.
    ExtraMessage { locale: String, message: String },
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
                write!(
                    f,
                    "Default locale '{locale}' is not present in the Fluent catalog"
                )
            }
            Self::FluentParse { locale, errors } => {
                write!(f, "Fluent parse errors for locale '{locale}': {errors:?}")
            }
            Self::AddResource { locale, errors } => {
                write!(
                    f,
                    "Failed to add resource for locale '{locale}': {errors:?}"
                )
            }
            Self::DuplicateLocale { locale } => {
                write!(f, "Duplicate Fluent catalog locale '{locale}'")
            }
            Self::MessageSchemaMismatch {
                locale,
                message,
                expected,
                actual,
            } => {
                write!(
                    f,
                    "Message schema mismatch for '{message}' in locale '{locale}': expected variables {expected:?}, got {actual:?}"
                )
            }
            Self::ExtraMessage { locale, message } => {
                write!(
                    f,
                    "Message '{message}' in locale '{locale}' does not exist in default locale catalog"
                )
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
///
/// This enum is non-exhaustive so the facade can grow new typed failures without
/// turning every downstream exhaustive match into a semver blocker.
#[non_exhaustive]
#[derive(Debug)]
pub enum I18nError {
    /// Bundle construction failed.
    BundleBuild(BundleBuildError),
    /// Message key was not found.
    MessageNotFound { locale: String, key: String },
    /// Message formatting encountered errors.
    FormattingFailed {
        locale: String,
        key: String,
        errors: Vec<fluent_bundle::FluentError>,
    },
    /// Message key is invalid.
    InvalidMessageKey {
        key: String,
        reason: MessageKeyError,
    },
}

impl fmt::Display for I18nError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BundleBuild(err) => write!(f, "Bundle build error: {err}"),
            Self::MessageNotFound { locale, key } => {
                write!(f, "Message '{key}' not found for locale '{locale}'")
            }
            Self::FormattingFailed {
                locale,
                key,
                errors,
            } => {
                write!(
                    f,
                    "Formatting errors for message '{key}' in locale '{locale}': {errors:?}"
                )
            }
            Self::InvalidMessageKey { key, reason } => {
                if key.is_empty() {
                    write!(f, "Invalid message key: {reason}")
                } else {
                    write!(f, "Invalid message key '{key}': {reason}")
                }
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
