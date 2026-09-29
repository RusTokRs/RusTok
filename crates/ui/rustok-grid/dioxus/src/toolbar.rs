//! Toolbar above the table.

use dioxus::prelude::*;

use rustok_grid::GridColumnDef;

/// Grid toolbar: selection banner with bulk actions, active-filter reset and
/// the column visibility picker.
#[component]
pub fn GridToolbar(
    /// Number of records behind the grid (not just the current page).
    total_count: u64,
    /// Number of currently selected rows.
    selected_count: usize,
    /// Whether at least one filter is applied.
    has_active_filters: bool,
    /// All columns, including hidden ones — the picker needs both.
    columns: Vec<GridColumnDef>,
    /// Emitted when the user asks for all filters to be dropped.
    on_clear_filters: EventHandler<()>,
    /// Emitted with the id of the column whose visibility was toggled.
    on_toggle_column_visibility: EventHandler<String>,
    /// Renders actions for the current selection, e.g. "delete selected".
    #[props(default)]
    bulk_actions: Option<Callback<usize, Element>>,
) -> Element {
    let mut show_columns_dropdown = use_signal(|| false);
    let dropdown_open = *show_columns_dropdown.read();

    // Columns the user is allowed to hide: the synthetic checkbox column and
    // pinned columns are structural and always stay visible.
    let toggleable = columns
        .iter()
        .filter(|col| !col.is_checkbox() && col.pinned.is_none())
        .cloned()
        .collect::<Vec<_>>();

    let selection_banner: Element = if selected_count > 0 {
        let actions: Option<Element> = bulk_actions.map(|render| render.call(selected_count));
        rsx! {
            div {
                class: "flex items-center gap-2 px-2.5 py-1 rounded bg-primary/10 text-primary text-xs font-semibold",
                "aria-live": "polite",
                span { "{selected_count} selected" }
                {actions}
            }
        }
    } else {
        rsx! {
            div { class: "flex items-center gap-2 text-muted-foreground text-xs",
                span { "Total items:" }
                span { class: "font-bold text-foreground", "{total_count}" }
            }
        }
    };

    let column_items = toggleable.into_iter().map(|col| {
        let id = col.id.as_str().to_string();
        let title = col.title.clone();
        let visible = col.visible;
        rsx! {
            label {
                key: "{id}",
                class: "flex items-center gap-2 px-1 py-0.5 rounded hover:bg-muted/60 cursor-pointer",
                input {
                    r#type: "checkbox",
                    checked: visible,
                    class: "rounded border-border text-primary focus:ring-primary/20 h-3.5 w-3.5",
                    onchange: move |_| on_toggle_column_visibility.call(id.clone()),
                }
                span { class: "truncate", "{title}" }
            }
        }
    });

    let dropdown: Option<Element> = dropdown_open.then(|| {
        rsx! {
            // Backdrop: closes the popover on any outside click without
            // registering a global document listener.
            div {
                class: "fixed inset-0 z-20",
                "aria-hidden": "true",
                onclick: move |_| show_columns_dropdown.set(false),
            }
            div {
                class: "absolute right-0 mt-1 w-48 rounded-lg border border-border bg-popover text-popover-foreground shadow-lg p-2 z-30 text-xs space-y-1",
                div {
                    class: "font-semibold text-muted-foreground px-1 pb-1 border-b border-border/50",
                    "Toggle Columns"
                }
                div { class: "max-h-56 overflow-y-auto space-y-1 pt-1", {column_items} }
            }
        }
    });

    let filter_badge: Option<Element> = has_active_filters.then(|| {
        rsx! {
            button {
                r#type: "button",
                class: "inline-flex items-center gap-1 text-xs px-2 py-0.5 rounded-full bg-destructive/10 text-destructive hover:bg-destructive/20 transition-colors font-medium",
                onclick: move |_| on_clear_filters.call(()),
                span { "Filters active" }
                span { class: "text-[10px]", "aria-hidden": "true", "✕" }
            }
        }
    });

    rsx! {
        div {
            class: "flex items-center justify-between gap-3 px-4 py-3 border-b border-border/80 bg-background text-sm",
            div { class: "flex items-center gap-3",
                {selection_banner}
                {filter_badge}
            }
            div {
                class: "relative",
                onkeydown: move |ev| {
                    if ev.key().to_string() == "Escape" && *show_columns_dropdown.peek() {
                        ev.stop_propagation();
                        show_columns_dropdown.set(false);
                    }
                },
                button {
                    r#type: "button",
                    "aria-haspopup": "true",
                    "aria-expanded": if dropdown_open { "true" } else { "false" },
                    class: "text-xs px-2.5 py-1 rounded border border-border bg-background hover:bg-muted font-medium text-foreground transition-colors flex items-center gap-1.5 shadow-sm",
                    onclick: move |_| {
                        let open = *show_columns_dropdown.peek();
                        show_columns_dropdown.set(!open);
                    },
                    span { "Columns ⚙" }
                }
                {dropdown}
            }
        }
    }
}
