//! Mounted admin facet panel: renders the shared grid panel view model as URL-driven links.
//!
//! Every bucket is an `<a>` that toggles exactly one `attribute_filters` selection, so the panel
//! keeps no client-side state, deep links stay shareable, and the grid below reloads through the
//! same controls the operator sees.

use leptos::prelude::*;

use crate::facets::{ProductAdminFacetPanelView, ProductAdminFacetView};

#[component]
pub fn AdminCatalogFacetPanel(panel: ProductAdminFacetPanelView) -> impl IntoView {
    let ProductAdminFacetPanelView {
        title,
        facets,
        show_empty_state,
        empty_message,
        clear_href,
        clear_label,
    } = panel;

    view! {
        <div class="flex flex-col gap-3 rounded-2xl border border-border bg-card p-4 shadow-sm">
            <div class="flex items-center justify-between gap-3">
                <span class="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                    {title}
                </span>
                {clear_href
                    .map(|href| {
                        let label = clear_label.clone();
                        view! {
                            <a
                                href=href
                                class="text-xs font-medium text-muted-foreground transition hover:text-foreground"
                            >
                                {label}
                            </a>
                        }
                    })}
            </div>

            <Show
                when=move || show_empty_state
                fallback=move || {
                    view! {
                        <div class="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3">
                            {facets
                                .clone()
                                .into_iter()
                                .map(render_facet)
                                .collect_view()}
                        </div>
                    }
                }
            >
                {
                    let empty_message = empty_message.clone();
                    view! {
                        <p class="text-xs text-muted-foreground">{move || empty_message.clone()}</p>
                    }
                }
            </Show>
        </div>
    }
}

fn render_facet(facet: ProductAdminFacetView) -> impl IntoView {
    let ProductAdminFacetView {
        code,
        label,
        is_enumerable: _,
        unbounded_hint,
        is_truncated: _,
        truncated_hint,
        clear_href,
        clear_label,
        values,
    } = facet;

    view! {
        <div class="flex flex-col gap-1.5" data-facet=code>
            <div class="flex items-center justify-between gap-2">
                <span class="text-xs font-semibold text-foreground">{label}</span>
                {clear_href
                    .map(|href| {
                        let clear_label = clear_label.clone();
                        view! {
                            <a
                                href=href
                                class="text-[11px] font-medium text-muted-foreground transition hover:text-foreground"
                            >
                                {clear_label}
                            </a>
                        }
                    })}
            </div>

            {unbounded_hint
                .map(|hint| view! { <p class="text-[11px] text-muted-foreground">{hint}</p> })}

            <ul class="flex flex-col gap-1">
                {values
                    .into_iter()
                    .map(|value| {
                        view! {
                            <li>
                                <a
                                    href=value.href
                                    aria-pressed=if value.selected { "true" } else { "false" }
                                    class=if value.selected {
                                        "flex items-center justify-between gap-2 rounded-lg bg-primary/10 px-2 py-1 text-xs font-medium text-foreground transition"
                                    } else {
                                        "flex items-center justify-between gap-2 rounded-lg px-2 py-1 text-xs text-muted-foreground transition hover:bg-accent hover:text-foreground"
                                    }
                                >
                                    <span class="truncate">
                                        <span class="mr-1.5 font-mono text-[10px] text-muted-foreground">
                                            {value.marker}
                                        </span>
                                        {value.label}
                                    </span>
                                    <span class="tabular-nums text-[11px] text-muted-foreground">
                                        {value.count_label}
                                    </span>
                                </a>
                            </li>
                        }
                    })
                    .collect_view()}
            </ul>

            {truncated_hint
                .map(|hint| view! { <p class="text-[11px] text-muted-foreground">{hint}</p> })}
        </div>
    }
}
