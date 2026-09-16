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
    BundleBuildError, FluentArgs, I18nError, PreparedUiMessages, UiLocaleTranslator, UiMessages,
    UiTranslator, declare_module_i18n, fluent_args, module_t, t,
};
