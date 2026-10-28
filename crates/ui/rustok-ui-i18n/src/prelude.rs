/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! Preferred high-level import surface for `rustok-ui-i18n` consumers.
//!
//! This module is intentionally smaller than the crate root. The crate root keeps
//! existing low-level exports for compatibility while pre-1.0 API migration is in
//! progress; new module-owned UI code should prefer this prelude unless it needs a
//! documented lower-level catalog or locale primitive.

pub use crate::{
    AcceptLanguageError, AcceptLanguagePreference, BundleBuildError, FluentArgs, FluentValue,
    I18nError, LazyUiLocaleTranslator, LazyUiMessages, MAX_ACCEPT_LANGUAGE_LEN,
    MAX_ACCEPT_LANGUAGE_RANGES, MAX_LOCALE_TAG_LEN, MAX_MESSAGE_KEY_LEN, MessageKeyError,
    PreparedUiMessages, ResolvedMessage, TextDirection, UiLocaleTranslator, UiMessages,
    UiTranslator, accept_language_catalog_locales, accept_language_locales, declare_module_i18n,
    fluent_args, locale_text_direction, module_t, parse_accept_language,
    preferred_catalog_locale_from_accept_language, preferred_locale_from_accept_language, t,
    try_parse_accept_language, validate_message_attribute, validate_message_key,
};
