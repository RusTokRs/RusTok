/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

/// Helper macro for constructing `FluentArgs`.
/// Supports both:
/// - `fluent_args!(name = val, count = count)`
/// - `fluent_args!("name" => val, "count" => count)`
#[macro_export]
macro_rules! fluent_args {
    ($($key:ident = $val:expr),* $(,)?) => {{
        let mut args = $crate::FluentArgs::new();
        $(
            args.set(stringify!($key), $crate::FluentValue::from($val));
        )*
        args
    }};
    ($($key:expr => $val:expr),* $(,)?) => {{
        let mut args = $crate::FluentArgs::new();
        $(
            args.set($key, $crate::FluentValue::from($val));
        )*
        args
    }};
}

/// Global macro for resolving messages against a specified `UiMessages` reference.
#[macro_export]
macro_rules! t {
    ($messages:expr, $locale:expr, $key:expr, $fallback:expr) => {
        $messages.t_for_locale($locale, $key, $fallback)
    };
    ($messages:expr, $locale:expr, $key:expr, $fallback:expr, $($arg_name:ident = $arg_val:expr),+ $(,)?) => {
        $messages.format(
            $locale,
            $key,
            Some(&$crate::fluent_args!($($arg_name = $arg_val),+)),
            $fallback,
        )
    };
    ($messages:expr, $locale:expr, $key:expr, $fallback:expr, $($arg_name:expr => $arg_val:expr),+ $(,)?) => {
        $messages.format(
            $locale,
            $key,
            Some(&$crate::fluent_args!($($arg_name => $arg_val),+)),
            $fallback,
        )
    };
}

/// Macro for resolving module-local messages with parameters.
#[macro_export]
macro_rules! module_t {
    ($locale:expr, $key:expr, $fallback:expr) => {
        $crate::t($locale, $key, $fallback)
    };
    ($locale:expr, $key:expr, $fallback:expr, $($arg_name:ident = $arg_val:expr),+ $(,)?) => {
        $crate::format(
            $locale,
            $key,
            Some(&$crate::fluent_args!($($arg_name = $arg_val),+)),
            $fallback,
        )
    };
}

/// Declares canonical module-owned i18n catalogs and accessor functions.
///
/// Default invocation (loads `../locales/en.ftl` and `../locales/ru.ftl`):
/// ```ignore
/// rustok_ui_i18n::declare_module_i18n!();
/// ```
///
/// Custom invocation (with additional locales):
/// ```ignore
/// rustok_ui_i18n::declare_module_i18n!(
///     "en",
///     &[
///         ("en", include_str!("../locales/en.ftl")),
///         ("ru", include_str!("../locales/ru.ftl")),
///         ("ar", include_str!("../locales/ar.ftl")),
///     ]
/// );
/// ```
#[macro_export]
macro_rules! declare_module_i18n {
    () => {
        $crate::declare_module_i18n!(
            "en",
            &[
                ("en", include_str!("../locales/en.ftl")),
                ("ru", include_str!("../locales/ru.ftl")),
            ]
        );
    };
    ($default_locale:expr, $bundles:expr) => {
        static MESSAGES: $crate::UiMessages = $crate::UiMessages::new($default_locale, $bundles);

        #[inline]
        pub fn t(locale: Option<&str>, key: &str, fallback: &str) -> String {
            MESSAGES.t_for_locale(locale, key, fallback)
        }

        #[inline]
        pub fn format<'args>(
            locale: Option<&str>,
            key: &str,
            args: Option<&$crate::FluentArgs<'args>>,
            fallback: &str,
        ) -> String {
            MESSAGES.format(locale, key, args, fallback)
        }
    };
}
