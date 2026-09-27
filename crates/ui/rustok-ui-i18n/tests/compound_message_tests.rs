/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use rustok_ui_i18n::{
    BundleBuildError, I18nError, MessageKeyError, UiMessages, extract_locale_entry_schemas,
    fluent_args, validate_message_attribute, validate_message_key,
};

fn strip_bidi_isolates(value: &str) -> String {
    value.replace(['\u{2068}', '\u{2069}'], "")
}

const EN: &str = r#"
-brand = RusTok
field = Name
    .placeholder = Enter { $subject } for { -brand }
    .aria-label = Name field
only-default = Default value
"#;

const RU: &str = r#"
-brand = РусТок
field = Имя
    .aria-label = Поле имени
"#;

static MESSAGES: UiMessages = UiMessages::new("en", &[("en", EN), ("ru", RU)]);

#[test]
fn compound_message_attributes_format_and_fall_back_independently() {
    let prepared = MESSAGES.prepare().expect("compound catalog should validate");
    let args = fluent_args!(subject = "profile");

    assert_eq!(
        strip_bidi_isolates(&prepared.format_attribute(
            Some("en"),
            "field",
            "placeholder",
            Some(&args),
            "fallback",
        )),
        "Enter profile for RusTok"
    );
    assert_eq!(
        strip_bidi_isolates(&prepared.format_attribute(
            Some("ru-RU"),
            "field",
            "placeholder",
            Some(&args),
            "fallback",
        )),
        "Enter profile for RusTok"
    );
    assert_eq!(
        prepared.format_attribute(
            Some("ru-RU"),
            "field",
            "aria-label",
            None,
            "fallback",
        ),
        "Поле имени"
    );
}

#[test]
fn locale_provenance_reports_exact_parent_and_default_sources() {
    let prepared = MESSAGES.prepare().expect("catalog should validate");

    let exact = prepared
        .try_format_with_locale(Some("ru-RU"), "field", None)
        .expect("ru parent should resolve");
    assert_eq!(exact.value(), "Имя");
    assert_eq!(exact.locale(), "ru");

    let default = prepared
        .try_format_with_locale(Some("ru-RU"), "only.default", None)
        .expect("default should resolve");
    assert_eq!(default.value(), "Default value");
    assert_eq!(default.locale(), "en");
    assert_eq!(
        prepared.available_locales().collect::<Vec<_>>(),
        vec!["en", "ru"]
    );
}

#[test]
fn entry_schema_keeps_value_and_attribute_contracts_separate() {
    let schemas = extract_locale_entry_schemas("en", EN).expect("schema should parse");
    let field = schemas.get("field").expect("field schema");

    assert!(
        field
            .value
            .as_ref()
            .expect("field value")
            .variables
            .is_empty()
    );
    assert_eq!(
        field.attributes["placeholder"]
            .variables
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        vec!["subject"]
    );
}

#[test]
fn strict_schema_resolves_transitive_message_and_term_variables() {
    const DEFAULT: &str = r#"
-brand = { $brand }
base = Hello, { $name } from { -brand }
welcome = { base }
"#;
    const INVALID: &str = r#"
-brand = { $brand }
base = Привет, { $user } от { -brand }
welcome = { base }
"#;

    let messages = UiMessages::new("en", &[("en", DEFAULT), ("ru", INVALID)]);
    let error = messages
        .prepare()
        .expect_err("transitive variable mismatch must fail startup");

    assert!(matches!(
        error,
        BundleBuildError::MessageSchemaMismatch {
            locale,
            message,
            expected,
            actual,
        } if locale == "ru"
            && message == "base"
            && expected == vec!["brand".to_string(), "name".to_string()]
            && actual == vec!["brand".to_string(), "user".to_string()]
    ));
}

#[test]
fn strict_schema_rejects_missing_and_cyclic_references_at_startup() {
    let missing = UiMessages::new("en", &[("en", "welcome = { absent }\n")]);
    assert!(matches!(
        missing.prepare(),
        Err(BundleBuildError::UnresolvedReference { .. })
    ));

    let cyclic = UiMessages::new("en", &[("en", "first = { second }\nsecond = { first }\n")]);
    assert!(matches!(
        cyclic.prepare(),
        Err(BundleBuildError::CyclicReference { .. })
    ));
}

#[test]
fn message_and_attribute_identifiers_enforce_fluent_grammar() {
    assert!(validate_message_key("account.profile-title").is_ok());
    assert!(validate_message_attribute("aria-label").is_ok());

    for key in [" leading", ".leading", "ключ", "key/value"] {
        assert!(matches!(
            validate_message_key(key),
            Err(I18nError::InvalidMessageKey {
                reason: MessageKeyError::InvalidSyntax,
                ..
            })
        ));
    }

    assert!(matches!(
        validate_message_key("key\u{0085}"),
        Err(I18nError::InvalidMessageKey {
            reason: MessageKeyError::InvalidCharacters,
            ..
        })
    ));
    assert!(matches!(
        validate_message_attribute("aria.label"),
        Err(I18nError::InvalidMessageAttribute {
            reason: MessageKeyError::InvalidSyntax,
            ..
        })
    ));
}

#[test]
fn strict_attribute_errors_are_typed_and_lenient_calls_use_fallback() {
    let prepared = MESSAGES.prepare().expect("catalog should validate");
    let error = prepared
        .try_format_attribute(Some("ru"), "field", "missing", None)
        .expect_err("missing attribute must be typed");

    assert!(matches!(
        error,
        I18nError::AttributeNotFound {
            locale,
            key,
            attribute,
        } if locale == "ru" && key == "field" && attribute == "missing"
    ));
    assert_eq!(
        prepared.format_attribute(
            Some("ru"),
            "field",
            "missing",
            None,
            "safe fallback",
        ),
        "safe fallback"
    );
}

#[test]
fn term_argument_binding_does_not_leak_internal_parameter_names() {
    const SOURCE: &str = r#"
-case-brand = { $case } RusTok
literal = { -case-brand(case: "for") }
dynamic = { -case-brand(case: $requestedCase) }
"#;

    let schemas = extract_locale_entry_schemas("en", SOURCE).expect("schema should parse");
    assert!(
        schemas["literal"]
            .value
            .as_ref()
            .expect("literal value")
            .variables
            .is_empty()
    );
    assert_eq!(
        schemas["dynamic"]
            .value
            .as_ref()
            .expect("dynamic value")
            .variables
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        vec!["requestedCase"]
    );

    let args = fluent_args!(requestedCase = "from");
    let messages = UiMessages::new("en", &[("en", SOURCE)]);
    assert_eq!(
        strip_bidi_isolates(&messages.format(Some("en"), "dynamic", Some(&args), "fallback")),
        "from RusTok"
    );
}
