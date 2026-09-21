/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use rustok_ui_i18n::error::{BundleBuildError, I18nError, MessageKeyError};
use rustok_ui_i18n::messages::{MAX_MESSAGE_KEY_LEN, UiMessages, validate_message_key};

#[test]
fn same_variables_across_locales_succeeds() {
    const EN: &str = "welcome = Welcome, { $name }!\n";
    const RU: &str = "welcome = Привет, { $name }!\n";

    let messages = UiMessages::new("en", &[("en", EN), ("ru", RU)]);
    let prepared = messages.prepare().expect("schema should match");
    assert_eq!(prepared.default_locale(), "en");
}

#[test]
fn different_variable_name_fails_with_schema_mismatch() {
    const EN: &str = "welcome = Welcome, { $name }!\n";
    const RU: &str = "welcome = Привет, { $user }!\n";

    let messages = UiMessages::new("en", &[("en", EN), ("ru", RU)]);
    let err = messages
        .prepare()
        .expect_err("should reject variable name mismatch");

    match err {
        BundleBuildError::MessageSchemaMismatch {
            locale,
            message,
            expected,
            actual,
        } => {
            assert_eq!(locale, "ru");
            assert_eq!(message, "welcome");
            assert_eq!(expected, vec!["name".to_string()]);
            assert_eq!(actual, vec!["user".to_string()]);
        }
        other => panic!("Unexpected error variant: {other:?}"),
    }
}

#[test]
fn missing_variable_in_locale_fails_with_schema_mismatch() {
    const EN: &str = "invite = { $inviter } invited you to { $group }!\n";
    const RU: &str = "invite = Вас пригласили в { $group }!\n";

    let messages = UiMessages::new("en", &[("en", EN), ("ru", RU)]);
    let err = messages
        .prepare()
        .expect_err("should reject missing variable in ru");

    match err {
        BundleBuildError::MessageSchemaMismatch {
            locale,
            message,
            expected,
            actual,
        } => {
            assert_eq!(locale, "ru");
            assert_eq!(message, "invite");
            assert_eq!(expected, vec!["group".to_string(), "inviter".to_string()]);
            assert_eq!(actual, vec!["group".to_string()]);
        }
        other => panic!("Unexpected error variant: {other:?}"),
    }
}

#[test]
fn extra_variable_in_locale_fails_with_schema_mismatch() {
    const EN: &str = "status = Online\n";
    const RU: &str = "status = В сети: { $time }\n";

    let messages = UiMessages::new("en", &[("en", EN), ("ru", RU)]);
    let err = messages
        .prepare()
        .expect_err("should reject extra variable in ru");

    match err {
        BundleBuildError::MessageSchemaMismatch {
            locale,
            message,
            expected,
            actual,
        } => {
            assert_eq!(locale, "ru");
            assert_eq!(message, "status");
            assert!(expected.is_empty());
            assert_eq!(actual, vec!["time".to_string()]);
        }
        other => panic!("Unexpected error variant: {other:?}"),
    }
}

#[test]
fn variable_order_differs_is_accepted() {
    const EN: &str = "swap = { $first } and { $second }\n";
    const RU: &str = "swap = { $second } и { $first }\n";

    let messages = UiMessages::new("en", &[("en", EN), ("ru", RU)]);
    assert!(messages.prepare().is_ok());
}

#[test]
fn select_expression_with_same_variable_is_accepted() {
    const EN: &str = "items = { $count ->\n    [one] One item\n   *[other] { $count } items\n}\n";
    const RU: &str = "items = { $count ->\n    [one] { $count } товар\n    [few] { $count } товара\n   *[other] { $count } товаров\n}\n";

    let messages = UiMessages::new("en", &[("en", EN), ("ru", RU)]);
    assert!(messages.prepare().is_ok());
}

#[test]
fn extra_message_in_non_default_locale_fails() {
    const EN: &str = "msg-a = A\n";
    const RU: &str = "msg-a = А\nmsg-b = Б\n";

    let messages = UiMessages::new("en", &[("en", EN), ("ru", RU)]);
    let err = messages
        .prepare()
        .expect_err("should reject extra message in ru");

    match err {
        BundleBuildError::ExtraMessage { locale, message } => {
            assert_eq!(locale, "ru");
            assert_eq!(message, "msg-b");
        }
        other => panic!("Unexpected error variant: {other:?}"),
    }
}

#[test]
fn missing_message_in_non_default_locale_is_accepted_for_fallback() {
    const EN: &str = "msg-a = A\nmsg-b = B\n";
    const RU: &str = "msg-a = А\n";

    let messages = UiMessages::new("en", &[("en", EN), ("ru", RU)]);
    let prepared = messages
        .prepare()
        .expect("missing message should be allowed for fallback");

    // "msg-a" resolves in ru
    assert_eq!(prepared.t(Some("ru"), "msg-a", "fallback"), "А");
    // "msg-b" falls back to en
    assert_eq!(prepared.t(Some("ru"), "msg-b", "fallback"), "B");
}

#[test]
fn multiple_locales_reports_correct_failing_locale() {
    const EN: &str = "greet = Hello, { $name }!\n";
    const ES: &str = "greet = Hola, { $name }!\n";
    const RU: &str = "greet = Привет, { $user }!\n";

    let messages = UiMessages::new("en", &[("en", EN), ("es", ES), ("ru", RU)]);
    let err = messages.prepare().expect_err("should report ru failure");

    match err {
        BundleBuildError::MessageSchemaMismatch { locale, .. } => {
            assert_eq!(locale, "ru");
        }
        other => panic!("Unexpected error variant: {other:?}"),
    }
}

#[test]
fn canonical_default_locale_stored_permanently() {
    const EN: &str = "hello = Hello\n";

    // Pass un-normalized "en_US" as default locale
    let messages = UiMessages::new("en_US", &[("en-US", EN)]);
    let prepared = messages.prepare().expect("catalog should prepare");

    // Canonical tag "en-US" is stored
    assert_eq!(prepared.default_locale(), "en-US");

    // Lookups with variations resolve against canonical catalog
    assert_eq!(prepared.t(Some("en_US"), "hello", "fb"), "Hello");
    assert_eq!(prepared.t(Some("EN-us"), "hello", "fb"), "Hello");
    assert_eq!(prepared.t(Some("en-US"), "hello", "fb"), "Hello");
}

#[test]
fn message_key_validation_bounds_and_characters() {
    // 1. Valid keys
    assert!(validate_message_key("simple").is_ok());
    assert!(validate_message_key("with-hyphens").is_ok());
    assert!(validate_message_key("with.dots").is_ok());

    // 256 bytes key is accepted
    let key_256 = "a".repeat(MAX_MESSAGE_KEY_LEN);
    assert!(validate_message_key(&key_256).is_ok());

    // 2. Empty key rejected
    let err = validate_message_key("").expect_err("empty key should fail");
    match err {
        I18nError::InvalidMessageKey { reason, .. } => {
            assert_eq!(reason, MessageKeyError::Empty);
        }
        other => panic!("Unexpected: {other:?}"),
    }

    // 3. 257 bytes key rejected
    let key_257 = "a".repeat(MAX_MESSAGE_KEY_LEN + 1);
    let err = validate_message_key(&key_257).expect_err("257-byte key should fail");
    match err {
        I18nError::InvalidMessageKey { reason, .. } => {
            assert_eq!(
                reason,
                MessageKeyError::TooLong {
                    length: 257,
                    max_len: 256
                }
            );
        }
        other => panic!("Unexpected: {other:?}"),
    }

    // 4. 1 MB key rejected safely
    let key_1mb = "a".repeat(1024 * 1024);
    assert!(validate_message_key(&key_1mb).is_err());

    // 5. Control characters / NUL rejected
    let err_nul = validate_message_key("key\0with_nul").expect_err("NUL should fail");
    match err_nul {
        I18nError::InvalidMessageKey { reason, .. } => {
            assert_eq!(reason, MessageKeyError::InvalidCharacters);
        }
        other => panic!("Unexpected: {other:?}"),
    }

    let err_newline = validate_message_key("key\nwith_newline").expect_err("newline should fail");
    match err_newline {
        I18nError::InvalidMessageKey { reason, .. } => {
            assert_eq!(reason, MessageKeyError::InvalidCharacters);
        }
        other => panic!("Unexpected: {other:?}"),
    }
}

#[test]
fn lenient_lookup_with_invalid_or_missing_key_returns_fallback_cleanly() {
    const EN: &str = "hello = Hello\n";
    let messages = UiMessages::new("en", &[("en", EN)]);

    // Missing key returns fallback
    assert_eq!(
        messages.t(Some("en"), "non.existent.key", "Fallback"),
        "Fallback"
    );

    // Oversized key returns fallback safely without panic
    let oversized = "k.".repeat(200);
    assert_eq!(messages.t(Some("en"), &oversized, "Fallback"), "Fallback");

    // Empty key returns fallback
    assert_eq!(messages.t(Some("en"), "", "Fallback"), "Fallback");
}
