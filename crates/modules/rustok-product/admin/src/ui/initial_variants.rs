/*
 * Copyright (c) 2026 RusTokRs.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */

//! Rendering adapter for the explicit sparse selection owned by `core::initial_catalog`.
use crate::core::initial_catalog;
use crate::model::{ProductEffectiveForm, VariantAxisDraft, VariantAxisValueDraft, VariantDraft};
use leptos::prelude::*;
use leptos_ui::{Alert, AlertVariant, Button, Checkbox, Select, SelectOption};

#[component]
pub(super) fn InitialVariantsSection(
    locale: Option<String>,
    form: Signal<Option<ProductEffectiveForm>>,
    busy: ReadSignal<bool>,
    axes: RwSignal<Vec<VariantAxisDraft>>,
    variants: RwSignal<Vec<VariantDraft>>,
    on_add: Callback<Vec<VariantAxisValueDraft>>,
) -> impl IntoView {
    let (attribute, set_attribute) = signal(String::new());
    let pending = RwSignal::new(Vec::<VariantAxisValueDraft>::new());
    let error = RwSignal::new(None::<String>);
    let picker_locale = StoredValue::new(locale);
    let combination_axes = Signal::derive(move || {
        crate::core::combinations::initial_axes(&axes.get(), form.get().as_ref())
    });
    let combination_members =
        Signal::derive(move || crate::core::combinations::draft_members(&variants.get()));
    let fill_manual_tuple = Callback::new(move |values: Vec<VariantAxisValueDraft>| {
        if busy.get_untracked() {
            return;
        }
        pending.set(values);
        error.set(None);
    });
    let select_combination = Callback::new(move |values: Vec<VariantAxisValueDraft>| {
        if busy.get_untracked() {
            return;
        }
        let mut next = variants.get_untracked();
        match crate::core::combinations::append_initial_combination(
            &axes.get_untracked(),
            &mut next,
            form.get_untracked().as_ref(),
            &values,
        ) {
            Ok(()) => {
                variants.set(next);
                error.set(None);
            }
            Err(problem) => error.set(Some(crate::i18n::t(
                picker_locale.get_value().as_deref(),
                problem.message_key(),
                problem.message_key(),
            ))),
        }
    });
    view! {
        <fieldset disabled=move || busy.get()>
        <section class="space-y-3 rounded-xl border border-border p-4">
            <h3>"Initial axes and selected SKUs"</h3>
            <p class="text-xs text-muted-foreground">"Only the selected rows are created, atomically with the Product. No Cartesian grid is generated. Category changes retain this draft. Use the SKU, price and stock fields below to fill each selected row, then add it here."</p>
            {move || error.get().map(|message| view! { <Alert variant=AlertVariant::Destructive>{message}</Alert> })}
            {move || {
                let options = form.get().map(|form| form.attributes.into_iter()
                    .filter(crate::core::is_eligible_axis)
                    .map(|attribute| SelectOption::new(attribute.attribute_id, attribute.label)).collect()).unwrap_or_default();
                view! { <label><span>"Add an axis"</span><Select options=options value=attribute set_value=set_attribute placeholder="Select an eligible attribute" /></label> }
            }}
            <Button on_click=Box::new(move || {
                let result = form.get_untracked().ok_or_else(|| "Load the selected category form first".to_string()).and_then(|form| {
                    let mut next = axes.get_untracked();
                    initial_catalog::add_axis(&mut next, &attribute.get_untracked(), &form)?;
                    axes.set(next); Ok(())
                });
                error.set(result.err());
            })>"Add selected axis"</Button>
            <Button on_click=Box::new(move || {
                let result = initial_catalog::suggested_axes_for_empty_draft(&axes.get_untracked(), form.get_untracked().as_ref());
                match result { Ok(next) => { axes.set(next); error.set(None); }, Err(message) => error.set(Some(message)) }
            })>"Use required and suggested axes"</Button>
            <For each=move || axes.get() key=|axis| axis.attribute_id.clone() children=move |axis| {
                let id = axis.attribute_id.clone();
                let remove_id = id.clone();
                let assignment_id = id.clone();
                view! {
                    <fieldset class="space-y-2 rounded border p-3">
                        <legend>{id.clone()}</legend>
                        <Button on_click=Box::new(move || axes.update(|rows| initial_catalog::remove_axis(rows, &remove_id)))>"Remove draft axis"</Button>
                        {move || {
                            let attribute_id = id.clone();
                            let attribute = form.get().and_then(|form| form.attributes.into_iter().find(|attribute| attribute.attribute_id == attribute_id));
                            match attribute {
                                Some(attribute) => view! {
                                    <div class="flex flex-wrap gap-2">
                                        {attribute.options.into_iter().map(|option| view! {
                                            <AllowedValue axes=axes attribute_id=attribute_id.clone() option_id=option.id label=option.label />
                                        }).collect_view()}
                                    </div>
                                }.into_any(),
                                None => view! { <Alert variant=AlertVariant::Destructive>"This axis is not in the selected category. Remove or replace the retained draft axis."</Alert> }.into_any(),
                            }
                        }}
                        <AssignmentSelect axes=axes pending=pending attribute_id=assignment_id form=form busy=busy locale=picker_locale.get_value() />
                    </fieldset>
                }
            } />
            <super::combinations::CombinationPicker
                axes=combination_axes members=combination_members
                disabled=Signal::derive(move || busy.get())
                at_capacity=Signal::derive(move || variants.get().len() >= initial_catalog::MAX_INITIAL_VARIANTS)
                on_pick=select_combination locale=picker_locale.get_value() draft=true on_manual=fill_manual_tuple
            />
            <Button on_click=Box::new(move || { pending.set(Vec::new()); })>"Clear the pending assignment tuple"</Button>
            <Button on_click=Box::new(move || on_add.run(pending.get_untracked()))>"Add selected SKU using the fields below"</Button>
            <p class="text-xs">{move || format!("{} / {} selected SKUs", variants.get().len(), initial_catalog::MAX_INITIAL_VARIANTS)}</p>
            {move || variants.get().into_iter().enumerate().map(|(index, variant)| {
                let summary = initial_catalog::initial_variant_summary(&variant);
                view! { <div class="flex items-center gap-2 text-xs"><span>{summary}</span><Button on_click=Box::new(move || variants.update(|rows| { if index < rows.len() { rows.remove(index); } }))>"Remove draft SKU"</Button></div> }
            }).collect_view()}
        </section>
        </fieldset>
    }
}

#[component]
fn AllowedValue(
    axes: RwSignal<Vec<VariantAxisDraft>>,
    attribute_id: String,
    option_id: String,
    label: String,
) -> impl IntoView {
    let checked_initially = axes.get_untracked().iter().any(|axis| {
        axis.attribute_id == attribute_id && axis.allowed_option_ids.contains(&option_id)
    });
    let (checked, set_checked) = signal(checked_initially);
    Effect::new(move |_| {
        let allowed = checked.get();
        axes.update(|axes| {
            initial_catalog::set_allowed_value(axes, &attribute_id, &option_id, allowed)
        });
    });
    view! { <label class="inline-flex items-center gap-1 text-xs"><Checkbox checked=checked set_checked=set_checked />{label}</label> }
}

#[component]
fn AssignmentSelect(
    axes: RwSignal<Vec<VariantAxisDraft>>,
    pending: RwSignal<Vec<VariantAxisValueDraft>>,
    attribute_id: String,
    form: Signal<Option<ProductEffectiveForm>>,
    busy: ReadSignal<bool>,
    locale: Option<String>,
) -> impl IntoView {
    let id = StoredValue::new(attribute_id);
    let locale = StoredValue::new(locale);
    let initial = pending.with_untracked(|values| {
        initial_catalog::pending_assignment_value(values, &id.get_value())
    });
    let (value, set_value) = signal(initial);
    // One-way source -> control reflection. Only an actual DOM change publishes below;
    // mounting, metadata refresh, clear and programmatic picks never erase the tuple.
    Effect::new(move |_| {
        let next = pending
            .with(|values| initial_catalog::pending_assignment_value(values, &id.get_value()));
        if value.get_untracked() != next {
            set_value.set(next);
        }
    });
    view! {
        <label on:change=move |ev| {
            if busy.get_untracked() { return; }
            let selected = event_target_value(&ev);
            pending.update(|values| initial_catalog::set_pending_assignment(values, &id.get_value(), selected));
        }>
            <span>"Value for the next selected SKU"</span>
            {move || {
                let options = initial_catalog::pending_assignment_choices(
                    &axes.get(), &pending.get(), &id.get_value(), form.get().as_ref(),
                ).into_iter().map(|option| {
                    let mut view = SelectOption::new(option.id, if option.available { option.label } else {
                        format!("{} · {}", crate::i18n::t(locale.get_value().as_deref(), "product-combinations-retainedAssignment", "product-combinations-retainedAssignment"), option.label)
                    });
                    view.disabled = !option.available;
                    view
                }).collect();
                view! { <Select options=options value=value disabled=busy.get() placeholder="Select an allowed value" /> }
            }}
        </label>
    }
}
