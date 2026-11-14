/*
 * Copyright (c) 2026 RusTokRs.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */

//! Rendering adapter for local candidate pages. Creation stays with the existing Product commands.
use crate::core::combinations::{self, CombinationAxis, CombinationError, CombinationMember};
use crate::model::VariantAxisValueDraft;
use leptos::prelude::*;
use rustok_ui_leptos::{Alert, AlertVariant, Button, Select, SelectOption};

#[component]
pub(super) fn CombinationPicker(
    axes: Signal<Result<Vec<CombinationAxis>, CombinationError>>,
    members: Signal<Result<Vec<CombinationMember>, CombinationError>>,
    disabled: Signal<bool>,
    at_capacity: Signal<bool>,
    on_pick: Callback<Vec<VariantAxisValueDraft>>,
    locale: Option<String>,
    #[prop(default = false)] draft: bool,
    #[prop(optional)] on_manual: Option<Callback<Vec<VariantAxisValueDraft>>>,
) -> impl IntoView {
    let locale = StoredValue::new(locale);
    let text = move |key: &str| crate::i18n::t(locale.get_value().as_deref(), key, key);
    let filters = RwSignal::new(Vec::<VariantAxisValueDraft>::new());
    let page = RwSignal::new(0usize);
    let click_problem = RwSignal::new(None::<CombinationError>);
    let preview = Memo::new(move |_| {
        axes.get().and_then(|axes| {
            members.get().and_then(|members| {
                combinations::combination_page(&axes, &filters.get(), &members, page.get(), draft)
            })
        })
    });
    // Source errors are announced below and render no candidates, never a successful empty space.
    view! {
        <fieldset disabled=move || disabled.get() class="space-y-3 rounded-md border p-3">
            <legend class="px-1 text-sm font-medium">{text("product-combinations-title")}</legend>
            <p class="text-xs text-muted-foreground">{text(if draft { "product-combinations-initialHelp" } else { "product-combinations-savedHelp" })}</p>
            <div class="grid gap-3 sm:grid-cols-2">
                <For each=move || match axes.get() { Ok(axes) => axes, Err(_) => Vec::new() } key=|axis| (axis.attribute_id.clone(), axis.label.clone(), axis.options.iter().map(|option| (option.id.clone(), option.label.clone())).collect::<Vec<_>>()) children=move |axis| view! {
                    <CombinationFilter axis=axis filters=filters page=page disabled=disabled click_problem=click_problem locale=locale.get_value() />
                } />
            </div>
            <Button on_click=Box::new(move || { filters.set(Vec::new()); page.set(0); click_problem.set(None); })>{text("product-combinations-reset")}</Button>
            {move || preview.get().err().or(click_problem.get()).map(|problem| view! { <Alert variant=AlertVariant::Destructive>{text(problem.message_key())}</Alert> })}
            {move || at_capacity.get().then(|| view! { <p role="status" class="text-xs text-muted-foreground">{text("product-combinations-capacity")}</p> })}
            {move || match preview.get() {
                Err(_) => None,
                Ok(preview) => Some(view! {
                    <p class="text-xs text-muted-foreground" aria-live="polite">{format!("{} {} · {} {} / {}", text("product-combinations-count"), preview.count, text("product-combinations-page"), preview.page + 1, preview.pages)}</p>
                    {(preview.incomplete_drafts > 0).then(|| view! { <p role="status" class="text-xs">{format!("{} ({})", text("product-combinations-incomplete"), preview.incomplete_drafts)}</p> })}
                }),
            }}
            <ul class="space-y-2">
                <For each=move || match preview.get() { Ok(page) => page.rows, Err(_) => Vec::new() } key=|row| (row.key.clone(), row.labels.clone(), row.existing.as_ref().map(|member| (member.id.clone(), member.sku.clone()))) children=move |row| {
                    let tuple = StoredValue::new(row.axis_values);
                    let summary = StoredValue::new(if row.labels.is_empty() { text("product-combinations-default") } else { row.labels.join(" · ") });
                    view! {
                        <li class="flex flex-wrap items-center justify-between gap-2 rounded border p-2 text-xs">
                            <span>{summary.get_value()}</span>
                            {match row.existing {
                                Some(member) => view! { <span>{format!("{} · {}", text(if draft { "product-combinations-selected" } else { "product-combinations-existing" }), member.sku.filter(|sku| !sku.is_empty()).unwrap_or(member.id))}</span> }.into_any(),
                                None => view! {
                                    <span class="text-muted-foreground">{text("product-combinations-missing")}</span>
                                    {move || view! {
                                        <Button disabled=disabled.get() || at_capacity.get() on_click=Box::new(move || {
                                            if disabled.get_untracked() || at_capacity.get_untracked() { return; }
                                            let result = axes.get_untracked().and_then(|axes| members.get_untracked().and_then(|members| combinations::choose_combination(&axes, &filters.get_untracked(), &members, &tuple.get_value(), draft)));
                                            match result {
                                                Ok(values) => { click_problem.set(None); on_pick.run(values); }
                                                Err(problem) => click_problem.set(Some(problem)),
                                            }
                                        })>{text(if draft { "product-combinations-add" } else { "product-combinations-use" })}<span class="sr-only">" "{summary.get_value()}</span></Button>
                                    }}
                                    {on_manual.map(|on_manual| view! {
                                        {move || view! {
                                            <Button disabled=disabled.get() || at_capacity.get() on_click=Box::new(move || {
                                                if disabled.get_untracked() || at_capacity.get_untracked() { return; }
                                                let result = axes.get_untracked().and_then(|axes| members.get_untracked().and_then(|members| combinations::choose_combination(&axes, &filters.get_untracked(), &members, &tuple.get_value(), draft)));
                                                match result {
                                                    Ok(values) => { click_problem.set(None); on_manual.run(values); }
                                                    Err(problem) => click_problem.set(Some(problem)),
                                                }
                                            })>{text("product-combinations-manual")}<span class="sr-only">" "{summary.get_value()}</span></Button>
                                        }}
                                    })}
                                }.into_any(),
                            }}
                        </li>
                    }
                } />
            </ul>
            <nav aria-label=text("product-combinations-navigation") class="flex gap-2">
                {move || { let current = preview.get().ok(); let previous_disabled = current.as_ref().is_none_or(|page| page.page == 0); view! {
                    <Button disabled=disabled.get() || previous_disabled on_click=Box::new(move || { if let Ok(current) = preview.get_untracked() { page.set(current.page.saturating_sub(1)); } })>{text("product-combinations-previous")}</Button>
                } }}
                {move || { let current = preview.get().ok(); let next_disabled = current.as_ref().is_none_or(|page| page.page + 1 >= page.pages); view! {
                    <Button disabled=disabled.get() || next_disabled on_click=Box::new(move || { if let Ok(current) = preview.get_untracked() { page.set((current.page + 1).min(current.pages - 1)); } })>{text("product-combinations-next")}</Button>
                } }}
            </nav>
        </fieldset>
    }
}

#[component]
fn CombinationFilter(
    axis: CombinationAxis,
    filters: RwSignal<Vec<VariantAxisValueDraft>>,
    page: RwSignal<usize>,
    disabled: Signal<bool>,
    click_problem: RwSignal<Option<CombinationError>>,
    locale: Option<String>,
) -> impl IntoView {
    let id = StoredValue::new(axis.attribute_id);
    let initial = filters.with_untracked(|values| {
        values
            .iter()
            .find(|value| value.attribute_id == id.get_value())
            .map(|value| value.option_id.clone())
            .unwrap_or_default()
    });
    let (value, set_value) = signal(initial.clone());
    let mut options = vec![SelectOption::new(
        "",
        crate::i18n::t(
            locale.as_deref(),
            "product-combinations-any",
            "product-combinations-any",
        ),
    )];
    if !initial.is_empty() && !axis.options.iter().any(|option| option.id == initial) {
        let mut stale = SelectOption::new(
            initial.clone(),
            format!(
                "{} ({initial})",
                crate::i18n::t(
                    locale.as_deref(),
                    "product-combinations-unavailable",
                    "product-combinations-unavailable"
                )
            ),
        );
        stale.disabled = true;
        options.push(stale);
    }
    options.extend(
        axis.options
            .into_iter()
            .map(|option| SelectOption::new(option.id, option.label)),
    );
    // Reflect the source one way. Programmatic resets never emit a DOM change and cannot
    // feed an old local value back into the filters.
    Effect::new(move |_| {
        let next = filters.with(|values| {
            values
                .iter()
                .find(|value| value.attribute_id == id.get_value())
                .map(|value| value.option_id.clone())
                .unwrap_or_default()
        });
        if value.get_untracked() != next {
            set_value.set(next);
        }
    });
    let html_id = format!("combination-filter-{}", id.get_value());
    view! {
        <div class="space-y-1" on:change=move |ev| {
            if disabled.get_untracked() { return; }
            let next = event_target_value(&ev);
            filters.update(|values| combinations::set_filter(values, &id.get_value(), next));
            page.set(0);
            click_problem.set(None);
        }>
            <label for=html_id.clone() class="text-sm font-medium">{axis.label}</label>
            <Select options=options value=value id=html_id />
        </div>
    }
}
