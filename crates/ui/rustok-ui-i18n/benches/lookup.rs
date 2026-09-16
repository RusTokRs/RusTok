/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use std::fmt::Write as _;
use std::hint::black_box;

use criterion::{criterion_group, criterion_main, Criterion};
use rustok_ui_i18n::{
    build_fluent_catalog, fluent_args, locale_candidates, UiTranslator,
};

const EN: &str = r#"
title = English title
only-default = Default fallback
welcome = Welcome, { $name }!
"#;

const RU: &str = r#"
title = Русский заголовок
welcome = Привет, { $name }!
"#;

const MANY_LOCALES: &[(&str, &str)] = &[
    ("en-US", EN),
    ("en-GB", EN),
    ("fr-FR", EN),
    ("de-DE", EN),
    ("es-ES", EN),
    ("pt-BR", EN),
    ("it-IT", EN),
    ("nl-NL", EN),
    ("pl-PL", EN),
    ("cs-CZ", EN),
    ("sk-SK", EN),
    ("uk-UA", EN),
    ("ru-RU", RU),
    ("tr-TR", EN),
    ("sv-SE", EN),
    ("da-DK", EN),
    ("fi-FI", EN),
    ("nb-NO", EN),
    ("ja-JP", EN),
    ("ko-KR", EN),
    ("zh-Hans-CN", EN),
    ("zh-Hant-TW", EN),
];

fn large_catalog_source(message_count: usize) -> String {
    let mut source = String::with_capacity(message_count * 32);
    for index in 0..message_count {
        writeln!(&mut source, "message-{index} = Value {index}")
            .expect("writing to String cannot fail");
    }
    source
}

fn benchmark_lookup(c: &mut Criterion) {
    let catalog = build_fluent_catalog(&[("en", EN), ("ru", RU)]);
    let translator = UiTranslator::new(&catalog, "en");
    let prepared = translator.for_locale(Some("ru-RU"));
    let args = fluent_args!(name = "Иван");

    c.bench_function("i18n/locale_candidates/construct", |b| {
        b.iter(|| {
            black_box(locale_candidates(
                black_box(Some("zh-Hant-TW")),
                black_box("en-US"),
            ))
        })
    });

    c.bench_function("i18n/prepared_locale/construct", |b| {
        b.iter(|| {
            black_box(translator.for_locale(black_box(Some("ru-RU"))))
        })
    });

    c.bench_function("i18n/dynamic_locale/direct_key", |b| {
        b.iter(|| {
            translator.t(
                black_box(Some("ru-RU")),
                black_box("title"),
                black_box("fallback"),
            )
        })
    });

    c.bench_function("i18n/prepared_locale/direct_key", |b| {
        b.iter(|| prepared.t(black_box("title"), black_box("fallback")))
    });

    c.bench_function("i18n/dynamic_locale/default_fallback", |b| {
        b.iter(|| {
            translator.t(
                black_box(Some("ru-RU")),
                black_box("only.default"),
                black_box("fallback"),
            )
        })
    });

    c.bench_function("i18n/prepared_locale/default_fallback", |b| {
        b.iter(|| prepared.t(black_box("only.default"), black_box("fallback")))
    });

    c.bench_function("i18n/dynamic_locale/missing_key", |b| {
        b.iter(|| {
            translator.t(
                black_box(Some("ru-RU")),
                black_box("missing.key"),
                black_box("fallback"),
            )
        })
    });

    c.bench_function("i18n/prepared_locale/missing_key", |b| {
        b.iter(|| prepared.t(black_box("missing.key"), black_box("fallback")))
    });

    c.bench_function("i18n/dynamic_locale/interpolation", |b| {
        b.iter(|| {
            translator.format_message(
                black_box(Some("ru-RU")),
                black_box("welcome"),
                Some(black_box(&args)),
                black_box("fallback"),
            )
        })
    });

    c.bench_function("i18n/prepared_locale/interpolation", |b| {
        b.iter(|| {
            prepared.format(
                black_box("welcome"),
                Some(black_box(&args)),
                black_box("fallback"),
            )
        })
    });

    let many_catalog = build_fluent_catalog(MANY_LOCALES);
    let many_translator = UiTranslator::new(&many_catalog, "en-US");
    let many_prepared = many_translator.for_locale(Some("zh-Hant-TW"));

    c.bench_function("i18n/catalog_22_locales/prepared_direct_key", |b| {
        b.iter(|| many_prepared.t(black_box("title"), black_box("fallback")))
    });

    c.bench_function("i18n/catalog_22_locales/dynamic_direct_key", |b| {
        b.iter(|| {
            many_translator.t(
                black_box(Some("zh-Hant-TW")),
                black_box("title"),
                black_box("fallback"),
            )
        })
    });

    let large_source = large_catalog_source(1_000);
    let large_catalog = build_fluent_catalog(&[("en", large_source.as_str())]);
    let large_translator = UiTranslator::new(&large_catalog, "en");
    let large_prepared = large_translator.for_locale(Some("en"));

    c.bench_function("i18n/catalog_1000_messages/direct_last_key", |b| {
        b.iter(|| large_prepared.t(black_box("message-999"), black_box("fallback")))
    });

    c.bench_function("i18n/catalog_1000_messages/missing_key", |b| {
        b.iter(|| large_prepared.t(black_box("message-missing"), black_box("fallback")))
    });
}

criterion_group!(benches, benchmark_lookup);
criterion_main!(benches);
