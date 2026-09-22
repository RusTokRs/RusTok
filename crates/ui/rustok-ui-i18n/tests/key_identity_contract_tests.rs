/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use rustok_ui_i18n::UiMessages;

#[test]
fn dotted_and_kebab_spellings_share_one_message_identity() {
    static MESSAGES: UiMessages =
        UiMessages::new("en", &[("en", "account-profile-title = Profile title\n")]);

    assert_eq!(
        MESSAGES.t(Some("en"), "account.profile.title", "fallback"),
        "Profile title"
    );
    assert_eq!(
        MESSAGES.t(Some("en"), "account-profile-title", "fallback"),
        "Profile title"
    );
}

#[test]
fn aliases_keep_identical_fallback_behavior() {
    static MESSAGES: UiMessages = UiMessages::new(
        "en",
        &[
            ("en", "account-profile-title = English title\n"),
            ("ru", "other = Другое\n"),
        ],
    );

    assert_eq!(
        MESSAGES.t(Some("ru-RU"), "account.profile.title", "fallback"),
        "English title"
    );
    assert_eq!(
        MESSAGES.t(Some("ru-RU"), "account-profile-title", "fallback"),
        "English title"
    );
}
