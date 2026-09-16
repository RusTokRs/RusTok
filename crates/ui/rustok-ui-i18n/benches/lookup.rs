/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use std::hint::black_box;

use criterion::{criterion_group, criterion_main, Criterion};
use rustok_ui_i18n::{build_fluent_catalog, fluent_args, UiTranslator};

const EN: &str = r#"
title = English title
only-default = Default fallback
welcome = Welcome, { $name }!
"#;

const RU: &str = r#"
title = Русский заголовок
welcome = Привет, { $name }!
"#;

fn benchmark_lookup(c: &mut Criterion) {
    let catalog = build_fluent_catalog(&[("en", EN), ("ru", RU)]);
    let translator = UiTranslator::new(&catalog, "en");
    let prepared = translator.for_locale(Some("ru-RU"));
    let args = fluent_args!(name = "Иван");

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
}

criterion_group!(benches, benchmark_lookup);
criterion_main!(benches);
