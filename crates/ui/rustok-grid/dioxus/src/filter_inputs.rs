//! Controls rendered in the filter row, one per column.

use dioxus::prelude::*;

use rustok_grid::{FilterOption, FilterValue, GridFilterType};

/// When a free-text / number filter publishes its value.
///
/// Dioxus has no cross-platform timer in its own dependency set, so instead of
/// a debounce this adapter makes the trade-off explicit and lets the caller
/// pick. Discrete controls (select, date, boolean) always publish immediately —
/// there is nothing to debounce there.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FilterCommitMode {
    /// Publish when the user is done: `Enter`, `blur`, or a native `change`
    /// event. `Escape` reverts to the last applied value.
    ///
    /// This is the default because a server-backed grid would otherwise issue
    /// one request per keystroke.
    #[default]
    OnCommit,
    /// Publish on every keystroke. Use it for purely client-side data sets.
    Live,
}

impl FilterCommitMode {
    /// `true` when every keystroke should be published.
    pub fn is_live(self) -> bool {
        matches!(self, FilterCommitMode::Live)
    }
}

/// Local editing state of a text control.
///
/// `base` is the externally applied value at the moment editing started. While
/// it still matches the incoming value, the user's draft wins; as soon as the
/// outside world changes the filter (reset, restore from URL, another tab) the
/// draft is silently dropped. This keeps the control semi-controlled *without*
/// writing to signals during rendering.
#[derive(Clone, Debug, Default, PartialEq)]
struct TextDraft {
    base: String,
    text: String,
    editing: bool,
}

/// Same idea as [`TextDraft`], for the two inputs of a numeric range.
#[derive(Clone, Debug, Default, PartialEq)]
struct RangeDraft {
    base: (String, String),
    min: String,
    max: String,
    editing: bool,
}

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

fn parse_bound(raw: &str) -> Option<f64> {
    raw.trim().parse::<f64>().ok().filter(|n| n.is_finite())
}

/// One cell of the filter row, rendered according to the column's
/// [`GridFilterType`].
#[component]
pub fn GridFilterCell(
    /// Column this control filters.
    column_id: String,
    /// Column title — used to give every control an accessible name.
    #[props(default)]
    column_title: Option<String>,
    /// Control to render.
    filter_type: GridFilterType,
    /// Currently applied value, owned by the grid.
    #[props(default)]
    current_value: Option<FilterValue>,
    /// When free-text input is published.
    #[props(default)]
    commit_mode: FilterCommitMode,
    /// Emitted with `(column_id, value)` whenever a new value is published.
    on_change: EventHandler<(String, FilterValue)>,
) -> Element {
    // Hooks are called unconditionally, before the match: the rules of hooks
    // do not allow the number of hooks to depend on the filter type.
    let text_draft = use_signal(TextDraft::default);
    let range_draft = use_signal(RangeDraft::default);

    let title = column_title.unwrap_or_default();
    let label = |suffix: &str| {
        if title.trim().is_empty() {
            format!("Filter{suffix}")
        } else {
            format!("Filter by {title}{suffix}")
        }
    };

    match filter_type {
        GridFilterType::Text { placeholder } => {
            let external = text_of(&current_value);
            let mut draft = text_draft;
            let displayed = {
                let state = draft.read();
                if state.editing && state.base == external {
                    state.text.clone()
                } else {
                    external.clone()
                }
            };

            let live = commit_mode.is_live();
            let placeholder = placeholder.unwrap_or_else(|| "Search...".to_string());
            let aria_label = label("");

            let id_input = column_id.clone();
            let id_commit = column_id.clone();
            let id_keys = column_id;
            let ext_input = external.clone();
            let ext_commit = external;

            rsx! {
                div { class: "relative w-full",
                    input {
                        r#type: "text",
                        placeholder: "{placeholder}",
                        "aria-label": "{aria_label}",
                        value: "{displayed}",
                        class: "w-full text-xs rounded border border-border/80 bg-background/50 px-2 py-1 placeholder:text-muted-foreground/60 focus:border-primary focus:outline-none focus:ring-1 focus:ring-primary/30",
                        oninput: move |ev| {
                            let value = ev.value();
                            if live {
                                draft.set(TextDraft::default());
                                on_change.call((id_input.clone(), FilterValue::Text(value)));
                            } else {
                                draft
                                    .set(TextDraft {
                                        base: ext_input.clone(),
                                        text: value,
                                        editing: true,
                                    });
                            }
                        },
                        // Fires on blur and on Enter — the browser's own
                        // definition of "the user is done typing".
                        onchange: move |ev| {
                            let value = ev.value();
                            draft.set(TextDraft::default());
                            if value != ext_commit {
                                on_change.call((id_commit.clone(), FilterValue::Text(value)));
                            }
                        },
                        onkeydown: move |ev| {
                            match ev.key().to_string().as_str() {
                                "Escape" => {
                                    ev.stop_propagation();
                                    draft.set(TextDraft::default());
                                }
                                "Enter" => {
                                    ev.prevent_default();
                                    let pending = {
                                        let state = draft.read();
                                        state.editing.then(|| state.text.clone())
                                    };
                                    if let Some(value) = pending {
                                        draft.set(TextDraft::default());
                                        on_change
                                            .call((id_keys.clone(), FilterValue::Text(value)));
                                    }
                                }
                                _ => {}
                            }
                        },
                    }
                }
            }
        }

        GridFilterType::Select {
            options,
            placeholder,
        } => {
            let selected = match current_value {
                Some(FilterValue::Select(ref s)) => s.clone(),
                _ => String::new(),
            };
            let placeholder = placeholder.unwrap_or_else(|| "All".to_string());
            let aria_label = label("");
            // The empty value is already rendered as the placeholder; a second
            // one would be a dead entry.
            let options = options
                .into_iter()
                .filter(|opt: &FilterOption| !opt.value.is_empty())
                .collect::<Vec<_>>();

            rsx! {
                select {
                    "aria-label": "{aria_label}",
                    value: "{selected}",
                    class: "w-full text-xs rounded border border-border/80 bg-background/50 px-1.5 py-1 text-foreground focus:border-primary focus:outline-none",
                    onchange: move |ev| {
                        on_change.call((column_id.clone(), FilterValue::Select(ev.value())));
                    },
                    option { value: "", selected: selected.is_empty(), "{placeholder}" }
                    {
                        options
                            .into_iter()
                            .map(|opt| {
                                let is_selected = opt.value == selected;
                                rsx! {
                                    option {
                                        key: "{opt.value}",
                                        value: "{opt.value}",
                                        selected: is_selected,
                                        "{opt.label}"
                                    }
                                }
                            })
                    }
                }
            }
        }

        GridFilterType::NumberRange {
            min_placeholder,
            max_placeholder,
            step,
        } => {
            let external = number_bounds(&current_value);
            let mut draft = range_draft;
            let (shown_min, shown_max) = {
                let state = draft.read();
                if state.editing && state.base == external {
                    (state.min.clone(), state.max.clone())
                } else {
                    external.clone()
                }
            };

            let live = commit_mode.is_live();
            let step_attr = step
                .filter(|s| s.is_finite() && *s > 0.0)
                .map(|s| s.to_string())
                .unwrap_or_else(|| "any".to_string());
            let min_ph = min_placeholder.unwrap_or_else(|| "From".to_string());
            let max_ph = max_placeholder.unwrap_or_else(|| "To".to_string());
            let min_label = label(" (minimum)");
            let max_label = label(" (maximum)");

            // Publishing needs both bounds, so every handler goes through the
            // same place: whatever is on screen, parsed and normalized.
            let mut publish = move |id: String, min_raw: String, max_raw: String| {
                draft.set(RangeDraft::default());
                on_change.call((
                    id,
                    FilterValue::NumberRange {
                        min: parse_bound(&min_raw),
                        max: parse_bound(&max_raw),
                    },
                ));
            };

            let id_min_input = column_id.clone();
            let id_max_input = column_id.clone();
            let id_min_commit = column_id.clone();
            let id_max_commit = column_id;
            let ext_min_input = external.clone();
            let ext_max_input = external.clone();
            let shown_min_for_max = shown_max.clone();
            let shown_max_for_min = shown_min.clone();
            let commit_min_other = shown_max.clone();
            let commit_max_other = shown_min.clone();

            rsx! {
                div { class: "flex items-center gap-1",
                    input {
                        r#type: "number",
                        step: "{step_attr}",
                        placeholder: "{min_ph}",
                        "aria-label": "{min_label}",
                        value: "{shown_min}",
                        class: "w-1/2 text-xs rounded border border-border/80 bg-background/50 px-1 py-1 placeholder:text-muted-foreground/60 focus:border-primary focus:outline-none",
                        oninput: move |ev| {
                            let value = ev.value();
                            if live {
                                publish(id_min_input.clone(), value, shown_max_for_min.clone());
                            } else {
                                draft
                                    .set(RangeDraft {
                                        base: ext_min_input.clone(),
                                        min: value,
                                        max: shown_max_for_min.clone(),
                                        editing: true,
                                    });
                            }
                        },
                        onchange: move |ev| {
                            publish(id_min_commit.clone(), ev.value(), commit_min_other.clone());
                        },
                    }
                    span { class: "text-[10px] text-muted-foreground", "aria-hidden": "true", "-" }
                    input {
                        r#type: "number",
                        step: "{step_attr}",
                        placeholder: "{max_ph}",
                        "aria-label": "{max_label}",
                        value: "{shown_max}",
                        class: "w-1/2 text-xs rounded border border-border/80 bg-background/50 px-1 py-1 placeholder:text-muted-foreground/60 focus:border-primary focus:outline-none",
                        oninput: move |ev| {
                            let value = ev.value();
                            if live {
                                publish(id_max_input.clone(), shown_min_for_max.clone(), value);
                            } else {
                                draft
                                    .set(RangeDraft {
                                        base: ext_max_input.clone(),
                                        min: shown_min_for_max.clone(),
                                        max: value,
                                        editing: true,
                                    });
                            }
                        },
                        onchange: move |ev| {
                            publish(id_max_commit.clone(), commit_max_other.clone(), ev.value());
                        },
                    }
                }
            }
        }

        GridFilterType::DateRange {
            from_placeholder,
            to_placeholder,
        } => {
            let (from, to) = match current_value {
                Some(FilterValue::DateRange { ref from, ref to }) => (
                    from.clone().unwrap_or_default(),
                    to.clone().unwrap_or_default(),
                ),
                _ => (String::new(), String::new()),
            };
            let from_ph = from_placeholder.unwrap_or_else(|| "From".to_string());
            let to_ph = to_placeholder.unwrap_or_else(|| "To".to_string());
            let from_label = label(" (from)");
            let to_label = label(" (to)");

            let id_from = column_id.clone();
            let id_to = column_id;
            let to_for_from = to.clone();
            let from_for_to = from.clone();

            rsx! {
                div { class: "flex items-center gap-1",
                    input {
                        r#type: "date",
                        title: "{from_ph}",
                        "aria-label": "{from_label}",
                        value: "{from}",
                        class: "w-1/2 text-[11px] rounded border border-border/80 bg-background/50 px-1 py-1 text-foreground focus:border-primary focus:outline-none",
                        onchange: move |ev| {
                            let next = ev.value();
                            let other = to_for_from.clone();
                            on_change
                                .call((
                                    id_from.clone(),
                                    FilterValue::DateRange {
                                        from: (!next.is_empty()).then_some(next),
                                        to: (!other.is_empty()).then_some(other),
                                    },
                                ));
                        },
                    }
                    input {
                        r#type: "date",
                        title: "{to_ph}",
                        "aria-label": "{to_label}",
                        value: "{to}",
                        class: "w-1/2 text-[11px] rounded border border-border/80 bg-background/50 px-1 py-1 text-foreground focus:border-primary focus:outline-none",
                        onchange: move |ev| {
                            let next = ev.value();
                            let other = from_for_to.clone();
                            on_change
                                .call((
                                    id_to.clone(),
                                    FilterValue::DateRange {
                                        from: (!other.is_empty()).then_some(other),
                                        to: (!next.is_empty()).then_some(next),
                                    },
                                ));
                        },
                    }
                }
            }
        }

        GridFilterType::Boolean {
            true_label,
            false_label,
        } => {
            let selected = match current_value {
                Some(FilterValue::Boolean(true)) => "true",
                Some(FilterValue::Boolean(false)) => "false",
                _ => "",
            };
            let aria_label = label("");

            rsx! {
                select {
                    "aria-label": "{aria_label}",
                    value: "{selected}",
                    class: "w-full text-xs rounded border border-border/80 bg-background/50 px-1.5 py-1 text-foreground focus:border-primary focus:outline-none",
                    onchange: move |ev| {
                        let value = match ev.value().as_str() {
                            "true" => FilterValue::Boolean(true),
                            "false" => FilterValue::Boolean(false),
                            _ => FilterValue::Empty,
                        };
                        on_change.call((column_id.clone(), value));
                    },
                    option { value: "", selected: selected.is_empty(), "All" }
                    option { value: "true", selected: selected == "true", "{true_label}" }
                    option { value: "false", selected: selected == "false", "{false_label}" }
                }
            }
        }
    }
}
