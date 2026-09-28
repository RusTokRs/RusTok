use leptos::ev::Event;
use leptos::prelude::*;
use leptos_use::use_debounce_fn;

use crate::core::{FilterOption, FilterValue, GridFilterType};

#[component]
pub fn GridFilterCell(
    column_id: String,
    filter_type: GridFilterType,
    current_value: Signal<Option<FilterValue>>,
    on_change: Callback<(String, FilterValue)>,
) -> impl IntoView {
    match filter_type {
        GridFilterType::Text { placeholder } => {
            let col = column_id.clone();
            let initial_text = current_value
                .get_untracked()
                .and_then(|v| match v {
                    FilterValue::Text(s) => Some(s),
                    _ => None,
                })
                .unwrap_or_default();

            let (text_input, set_text_input) = signal(initial_text);

            let debounced_change = use_debounce_fn(
                move || {
                    let val = text_input.get_untracked();
                    on_change.run((col.clone(), FilterValue::Text(val)));
                },
                250.0,
            );

            let ph = placeholder.unwrap_or_else(|| "Search...".to_string());

            view! {
                <div class="relative w-full">
                    <input
                        type="text"
                        placeholder=ph
                        prop:value=move || text_input.get()
                        on:input=move |ev| {
                            set_text_input.set(event_target_value(&ev));
                            debounced_change();
                        }
                        class="w-full text-xs rounded border border-border/80 bg-background/50 px-2 py-1 placeholder:text-muted-foreground/60 focus:border-primary focus:outline-none focus:ring-1 focus:ring-primary/30"
                    />
                </div>
            }
            .into_any()
        }
        GridFilterType::Select {
            options,
            placeholder,
        } => {
            let col = column_id.clone();
            let selected_val = move || {
                current_value
                    .get()
                    .and_then(|v| match v {
                        FilterValue::Select(s) => Some(s),
                        _ => None,
                    })
                    .unwrap_or_default()
            };

            let ph = placeholder.unwrap_or_else(|| "All".to_string());

            view! {
                <select
                    prop:value=selected_val
                    on:change=move |ev: Event| {
                        let val = event_target_value(&ev);
                        on_change.run((col.clone(), FilterValue::Select(val)));
                    }
                    class="w-full text-xs rounded border border-border/80 bg-background/50 px-1.5 py-1 text-foreground focus:border-primary focus:outline-none"
                >
                    <option value="">{ph}</option>
                    {options
                        .into_iter()
                        .map(|opt: FilterOption| {
                            view! {
                                <option value=opt.value.clone()>
                                    {opt.label}
                                </option>
                            }
                        })
                        .collect_view()}
                </select>
            }
            .into_any()
        }
        GridFilterType::NumberRange {
            min_placeholder,
            max_placeholder,
            step,
        } => {
            let col = column_id.clone();

            let initial_min = current_value
                .get_untracked()
                .and_then(|v| match v {
                    FilterValue::NumberRange { min, .. } => min.map(|n| n.to_string()),
                    _ => None,
                })
                .unwrap_or_default();

            let initial_max = current_value
                .get_untracked()
                .and_then(|v| match v {
                    FilterValue::NumberRange { max, .. } => max.map(|n| n.to_string()),
                    _ => None,
                })
                .unwrap_or_default();

            let (min_input, set_min_input) = signal(initial_min);
            let (max_input, set_max_input) = signal(initial_max);

            let debounced_range = use_debounce_fn(
                move || {
                    let min_parsed = min_input.get_untracked().trim().parse::<f64>().ok();
                    let max_parsed = max_input.get_untracked().trim().parse::<f64>().ok();
                    on_change.run((
                        col.clone(),
                        FilterValue::NumberRange {
                            min: min_parsed,
                            max: max_parsed,
                        },
                    ));
                },
                250.0,
            );

            let step_attr = step.map(|s| s.to_string()).unwrap_or_else(|| "any".to_string());
            let min_ph = min_placeholder.unwrap_or_else(|| "From".to_string());
            let max_ph = max_placeholder.unwrap_or_else(|| "To".to_string());

            let debounced_min = debounced_range.clone();
            let on_min_input = move |ev| {
                set_min_input.set(event_target_value(&ev));
                debounced_min();
            };

            let debounced_max = debounced_range;
            let on_max_input = move |ev| {
                set_max_input.set(event_target_value(&ev));
                debounced_max();
            };

            view! {
                <div class="flex items-center gap-1">
                    <input
                        type="number"
                        step=step_attr.clone()
                        placeholder=min_ph
                        prop:value=move || min_input.get()
                        on:input=on_min_input
                        class="w-1/2 text-xs rounded border border-border/80 bg-background/50 px-1 py-1 placeholder:text-muted-foreground/60 focus:border-primary focus:outline-none"
                    />
                    <span class="text-[10px] text-muted-foreground">-</span>
                    <input
                        type="number"
                        step=step_attr
                        placeholder=max_ph
                        prop:value=move || max_input.get()
                        on:input=on_max_input
                        class="w-1/2 text-xs rounded border border-border/80 bg-background/50 px-1 py-1 placeholder:text-muted-foreground/60 focus:border-primary focus:outline-none"
                    />
                </div>
            }
            .into_any()
        }
        GridFilterType::DateRange {
            from_placeholder,
            to_placeholder,
        } => {
            let col = column_id.clone();
            let from_ph = from_placeholder.unwrap_or_else(|| "From".to_string());
            let to_ph = to_placeholder.unwrap_or_else(|| "To".to_string());

            let from_val = move || {
                current_value
                    .get()
                    .and_then(|v| match v {
                        FilterValue::DateRange { from, .. } => from,
                        _ => None,
                    })
                    .unwrap_or_default()
            };

            let to_val = move || {
                current_value
                    .get()
                    .and_then(|v| match v {
                        FilterValue::DateRange { to, .. } => to,
                        _ => None,
                    })
                    .unwrap_or_default()
            };

            let col_f = col.clone();
            let on_from_change = move |ev: Event| {
                let from = event_target_value(&ev);
                let to = current_value.get().and_then(|v| match v {
                    FilterValue::DateRange { to, .. } => to,
                    _ => None,
                });
                on_change.run((
                    col_f.clone(),
                    FilterValue::DateRange {
                        from: if from.is_empty() { None } else { Some(from) },
                        to,
                    },
                ));
            };

            let col_t = col.clone();
            let on_to_change = move |ev: Event| {
                let to = event_target_value(&ev);
                let from = current_value.get().and_then(|v| match v {
                    FilterValue::DateRange { from, .. } => from,
                    _ => None,
                });
                on_change.run((
                    col_t.clone(),
                    FilterValue::DateRange {
                        from,
                        to: if to.is_empty() { None } else { Some(to) },
                    },
                ));
            };

            view! {
                <div class="flex items-center gap-1">
                    <input
                        type="date"
                        title=from_ph
                        prop:value=from_val
                        on:change=on_from_change
                        class="w-1/2 text-[11px] rounded border border-border/80 bg-background/50 px-1 py-1 text-foreground focus:border-primary focus:outline-none"
                    />
                    <input
                        type="date"
                        title=to_ph
                        prop:value=to_val
                        on:change=on_to_change
                        class="w-1/2 text-[11px] rounded border border-border/80 bg-background/50 px-1 py-1 text-foreground focus:border-primary focus:outline-none"
                    />
                </div>
            }
            .into_any()
        }
        GridFilterType::Boolean {
            true_label,
            false_label,
        } => {
            let col = column_id.clone();
            let bool_val = move || {
                current_value
                    .get()
                    .and_then(|v| match v {
                        FilterValue::Boolean(b) => Some(if b { "true" } else { "false" }.to_string()),
                        _ => None,
                    })
                    .unwrap_or_default()
            };

            view! {
                <select
                    prop:value=bool_val
                    on:change=move |ev: Event| {
                        let val = event_target_value(&ev);
                        match val.as_str() {
                            "true" => on_change.run((col.clone(), FilterValue::Boolean(true))),
                            "false" => on_change.run((col.clone(), FilterValue::Boolean(false))),
                            _ => on_change.run((col.clone(), FilterValue::Text(String::new()))),
                        }
                    }
                    class="w-full text-xs rounded border border-border/80 bg-background/50 px-1.5 py-1 text-foreground focus:border-primary focus:outline-none"
                >
                    <option value="">"All"</option>
                    <option value="true">{true_label}</option>
                    <option value="false">{false_label}</option>
                </select>
            }
            .into_any()
        }
    }
}
