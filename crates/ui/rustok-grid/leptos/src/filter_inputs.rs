use leptos::ev::Event;
use leptos::prelude::*;
use leptos_use::use_debounce_fn;

use rustok_grid::{FilterOption, FilterValue, GridFilterType};

/// Debounce applied to free-text/number inputs before a filter change is
/// published, so typing does not trigger a request per keystroke.
const INPUT_DEBOUNCE_MS: f64 = 250.0;

fn text_of(value: &Option<FilterValue>) -> String {
    match value {
        Some(FilterValue::Text(s)) => s.clone(),
        _ => String::new(),
    }
}

fn number_bounds(value: &Option<FilterValue>) -> (String, String) {
    match value {
        Some(FilterValue::NumberRange { min, max }) => (
            min.map(|n| n.to_string()).unwrap_or_default(),
            max.map(|n| n.to_string()).unwrap_or_default(),
        ),
        _ => (String::new(), String::new()),
    }
}

/// One cell of the filter row, rendered according to the column's
/// [`GridFilterType`].
///
/// Inputs are *semi-controlled*: the user keeps typing into local state while
/// a debounce is pending, but any externally applied value (a programmatic
/// change, "reset all filters", state restored from a URL) is adopted
/// immediately.
#[component]
pub fn GridFilterCell(
    column_id: String,
    /// Column title — used to give every control an accessible name.
    #[prop(optional)]
    column_title: Option<String>,
    filter_type: GridFilterType,
    current_value: Signal<Option<FilterValue>>,
    on_change: Callback<(String, FilterValue)>,
) -> impl IntoView {
    let column = StoredValue::new(column_id);
    let title = column_title.unwrap_or_default();
    let label = move |suffix: &str| {
        if title.trim().is_empty() {
            format!("Filter{suffix}")
        } else {
            format!("Filter by {title}{suffix}")
        }
    };
    let emit = move |value: FilterValue| {
        on_change.run((column.get_value(), value));
    };

    match filter_type {
        GridFilterType::Text { placeholder } => {
            let initial = text_of(&current_value.get_untracked());
            let (text_input, set_text_input) = signal(initial.clone());
            // Last value we published; used to tell "the parent changed this"
            // from "our own change came back to us".
            let last_published = StoredValue::new(initial.trim().to_string());

            Effect::new(move |_| {
                let external = text_of(&current_value.get());
                if external != last_published.get_value() {
                    last_published.set_value(external.clone());
                    set_text_input.set(external);
                }
            });

            let debounced_change = use_debounce_fn(
                move || {
                    let value = text_input.get_untracked();
                    last_published.set_value(value.trim().to_string());
                    emit(FilterValue::Text(value));
                },
                INPUT_DEBOUNCE_MS,
            );

            let placeholder = placeholder.unwrap_or_else(|| "Search...".to_string());
            let aria_label = label("");

            view! {
                <div class="relative w-full">
                    <input
                        type="text"
                        placeholder=placeholder
                        aria-label=aria_label
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
            let selected = move || match current_value.get() {
                Some(FilterValue::Select(s)) => s,
                _ => String::new(),
            };
            let placeholder = placeholder.unwrap_or_else(|| "All".to_string());
            let aria_label = label("");

            view! {
                <select
                    aria-label=aria_label
                    prop:value=selected
                    on:change=move |ev: Event| emit(FilterValue::Select(event_target_value(&ev)))
                    class="w-full text-xs rounded border border-border/80 bg-background/50 px-1.5 py-1 text-foreground focus:border-primary focus:outline-none"
                >
                    <option value="">{placeholder}</option>
                    {options
                        .into_iter()
                        // The empty value is already rendered as the
                        // placeholder; a second one would be a dead entry.
                        .filter(|opt: &FilterOption| !opt.value.is_empty())
                        .map(|opt| {
                            let value = opt.value.clone();
                            view! {
                                <option value=opt.value selected=move || selected() == value>
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
            let (initial_min, initial_max) = number_bounds(&current_value.get_untracked());
            let (min_input, set_min_input) = signal(initial_min.clone());
            let (max_input, set_max_input) = signal(initial_max.clone());
            let last_published = StoredValue::new((initial_min, initial_max));

            Effect::new(move |_| {
                let external = number_bounds(&current_value.get());
                if external != last_published.get_value() {
                    last_published.set_value(external.clone());
                    set_min_input.set(external.0);
                    set_max_input.set(external.1);
                }
            });

            let publish = move || {
                let min_raw = min_input.get_untracked();
                let max_raw = max_input.get_untracked();
                let min = min_raw.trim().parse::<f64>().ok().filter(|n| n.is_finite());
                let max = max_raw.trim().parse::<f64>().ok().filter(|n| n.is_finite());
                last_published.set_value((
                    min.map(|n| n.to_string()).unwrap_or_default(),
                    max.map(|n| n.to_string()).unwrap_or_default(),
                ));
                emit(FilterValue::NumberRange { min, max });
            };
            let debounced_range = use_debounce_fn(publish, INPUT_DEBOUNCE_MS);

            let step_attr = step
                .filter(|s| s.is_finite() && *s > 0.0)
                .map(|s| s.to_string())
                .unwrap_or_else(|| "any".to_string());
            let min_ph = min_placeholder.unwrap_or_else(|| "From".to_string());
            let max_ph = max_placeholder.unwrap_or_else(|| "To".to_string());
            let min_label = label(" (minimum)");
            let max_label = label(" (maximum)");

            let debounced_min = debounced_range.clone();
            let debounced_max = debounced_range;

            view! {
                <div class="flex items-center gap-1">
                    <input
                        type="number"
                        step=step_attr.clone()
                        placeholder=min_ph
                        aria-label=min_label
                        prop:value=move || min_input.get()
                        on:input=move |ev| {
                            set_min_input.set(event_target_value(&ev));
                            debounced_min();
                        }
                        class="w-1/2 text-xs rounded border border-border/80 bg-background/50 px-1 py-1 placeholder:text-muted-foreground/60 focus:border-primary focus:outline-none"
                    />
                    <span class="text-[10px] text-muted-foreground" aria-hidden="true">
                        "-"
                    </span>
                    <input
                        type="number"
                        step=step_attr
                        placeholder=max_ph
                        aria-label=max_label
                        prop:value=move || max_input.get()
                        on:input=move |ev| {
                            set_max_input.set(event_target_value(&ev));
                            debounced_max();
                        }
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
            let range = move || match current_value.get() {
                Some(FilterValue::DateRange { from, to }) => {
                    (from.unwrap_or_default(), to.unwrap_or_default())
                }
                _ => (String::new(), String::new()),
            };
            let from_ph = from_placeholder.unwrap_or_else(|| "From".to_string());
            let to_ph = to_placeholder.unwrap_or_else(|| "To".to_string());
            let from_label = label(" (from)");
            let to_label = label(" (to)");

            let on_from_change = move |ev: Event| {
                let from = event_target_value(&ev);
                let (_, to) = range();
                emit(FilterValue::DateRange {
                    from: (!from.is_empty()).then_some(from),
                    to: (!to.is_empty()).then_some(to),
                });
            };

            let on_to_change = move |ev: Event| {
                let to = event_target_value(&ev);
                let (from, _) = range();
                emit(FilterValue::DateRange {
                    from: (!from.is_empty()).then_some(from),
                    to: (!to.is_empty()).then_some(to),
                });
            };

            view! {
                <div class="flex items-center gap-1">
                    <input
                        type="date"
                        title=from_ph
                        aria-label=from_label
                        prop:value=move || range().0
                        on:change=on_from_change
                        class="w-1/2 text-[11px] rounded border border-border/80 bg-background/50 px-1 py-1 text-foreground focus:border-primary focus:outline-none"
                    />
                    <input
                        type="date"
                        title=to_ph
                        aria-label=to_label
                        prop:value=move || range().1
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
            let selected = move || match current_value.get() {
                Some(FilterValue::Boolean(true)) => "true".to_string(),
                Some(FilterValue::Boolean(false)) => "false".to_string(),
                _ => String::new(),
            };
            let aria_label = label("");

            view! {
                <select
                    aria-label=aria_label
                    prop:value=selected
                    on:change=move |ev: Event| {
                        match event_target_value(&ev).as_str() {
                            "true" => emit(FilterValue::Boolean(true)),
                            "false" => emit(FilterValue::Boolean(false)),
                            _ => emit(FilterValue::Empty),
                        }
                    }
                    class="w-full text-xs rounded border border-border/80 bg-background/50 px-1.5 py-1 text-foreground focus:border-primary focus:outline-none"
                >
                    <option value="" selected=move || selected().is_empty()>
                        "All"
                    </option>
                    <option value="true" selected=move || selected() == "true">
                        {true_label}
                    </option>
                    <option value="false" selected=move || selected() == "false">
                        {false_label}
                    </option>
                </select>
            }
            .into_any()
        }
    }
}
