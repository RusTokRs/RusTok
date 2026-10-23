/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! UI components, shared DataGrid, and search integration for WebRTC Calls & Conferencing admin.

use leptos::prelude::*;
use rustok_grid::column::GridColumnDef;
use rustok_grid::filter::GridFilterType;
use rustok_grid::pagination::GridPagination;
use rustok_grid_leptos::prelude::*;

use crate::core::{filter_rows, CallsAdminTableRow};
use crate::i18n::t;

#[component]
pub fn CallsAdminPage() -> impl IntoView {
    let (search_query, set_search_query) = signal(String::new());
    let pagination = RwSignal::new(GridPagination::default());

    // Sample/initial data rows — in production, loaded via transport facade
    let sample_rows = vec![
        CallsAdminTableRow::new("rec-1", "Default WebRTC Calls & Conferencing Workspace", "active", "2026-01-01"),
        CallsAdminTableRow::new("rec-2", "Secondary WebRTC Calls & Conferencing Profile", "draft", "2026-01-02"),
    ];
    let rows_store = StoredValue::new(sample_rows);

    let filtered_rows = Memo::new(move |_| {
        let query = search_query.get();
        let rows = rows_store.get_value();
        filter_rows(&rows, if query.trim().is_empty() { None } else { Some(&query) })
    });

    // Columns specification for shared DataGrid
    let columns = vec![
        GridColumnDef::checkbox(),
        GridColumnDef::new("title", t(None, "calls-col-title", "Title")).filter(GridFilterType::text()),
        GridColumnDef::new("status", t(None, "calls-col-status", "Status")),
        GridColumnDef::new("created_at", t(None, "calls-col-created", "Created At")),
        GridColumnDef::new("actions", t(None, "calls-col-actions", "Actions")),
    ];

    let cell_renderer = Callback::new(move |(row, col_id): (CallsAdminTableRow, String)| {
        let row_id = row.id.clone();
        match col_id.as_str() {
            "title" => view! {
                <span class="font-medium text-foreground">{row.title}</span>
            }.into_any(),
            "status" => view! {
                <span class="inline-flex items-center px-2 py-0.5 rounded text-xs font-medium bg-secondary text-secondary-foreground">
                    {row.status}
                </span>
            }.into_any(),
            "created_at" => view! {
                <span class="text-xs text-muted-foreground">{row.created_at}</span>
            }.into_any(),
            "actions" => view! {
                <div class="flex items-center gap-2">
                    // RBAC-guarded action buttons
                    <button
                        type="button"
                        class="text-xs text-primary hover:underline font-medium"
                        disabled=!row.can_edit
                        on:click=move |_| {
                            tracing::info!(id = %row_id, "edit action triggered");
                        }
                    >
                        {t(None, "calls-action-edit", "Edit")}
                    </button>
                    <button
                        type="button"
                        class="text-xs text-destructive hover:underline font-medium"
                        disabled=!row.can_delete
                        on:click=move |_| {
                            tracing::info!(id = %row_id, "delete action triggered");
                        }
                    >
                        {t(None, "calls-action-delete", "Delete")}
                    </button>
                </div>
            }.into_any(),
            _ => ().into_any(),
        }
    });

    view! {
        <div class="p-6 space-y-6">
            // Header with RBAC create button & title
            <div class="flex flex-col sm:flex-row gap-4 items-start sm:items-center justify-between">
                <div>
                    <h1 class="text-2xl font-bold tracking-tight text-foreground">"WebRTC Calls & Conferencing"</h1>
                    <p class="text-sm text-muted-foreground">"WebRTC Calls & Conferencing capability module for RusToK"</p>
                </div>
                <div class="flex items-center gap-2">
                    <button
                        type="button"
                        class="px-4 py-2 text-sm font-medium rounded-lg bg-primary text-primary-foreground hover:bg-primary/90 transition-colors shadow-sm"
                        on:click=move |_| {
                            tracing::info!("create new calls record initiated");
                        }
                    >
                        {t(None, "calls-action-create", "Create New")}
                    </button>
                </div>
            </div>

            // Toolbar: Local table search & filter input
            <div class="flex items-center justify-between gap-4">
                <div class="relative w-72">
                    <input
                        type="text"
                        placeholder={t(None, "calls-search-placeholder", "Search records...")}
                        prop:value=move || search_query.get()
                        on:input=move |ev| set_search_query.set(event_target_value(&ev))
                        class="w-full text-xs rounded-lg border border-border bg-background px-3 py-2 text-foreground outline-none focus:border-primary transition-all"
                    />
                </div>
                <div class="text-xs text-muted-foreground">
                    "Search indexed globally via "
                    <code class="px-1 py-0.5 bg-muted rounded">"rustok-search"</code>
                </div>
            </div>

            // Shared platform DataGrid
            <div class="rounded-xl border border-border overflow-hidden bg-card shadow-sm">
                <DataGrid
                    columns=columns
                    data=Signal::derive(move || filtered_rows.get())
                    key_fn=|row: &CallsAdminTableRow| row.id.clone()
                    cell_renderer=cell_renderer
                    empty_message=t(None, "calls-empty", "No records found")
                    pagination=pagination
                    on_row_click=Callback::new(|_: CallsAdminTableRow| ())
                />
            </div>
        </div>
    }
}
