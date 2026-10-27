use super::options::ScaffoldOptions;
use super::write_file_content;
use anyhow::Result;
use std::path::Path;

pub(crate) fn generate_ui(opts: &ScaffoldOptions, module_dir: &Path, pascal: &str) -> Result<()> {
    if opts.ui.has_admin() {
        generate_admin_ui(opts, module_dir, pascal)?;
    }
    if opts.ui.has_storefront() {
        generate_storefront_ui(opts, module_dir, pascal)?;
    }
    Ok(())
}

fn generate_admin_ui(opts: &ScaffoldOptions, module_dir: &Path, pascal: &str) -> Result<()> {
    let slug = &opts.slug;
    let title = &opts.title;
    let desc = &opts.description;
    let crate_name = opts.crate_name();
    let admin_crate_name = format!("{crate_name}-admin");
    let admin_dir = module_dir.join("admin");

    // Cargo.toml
    let cargo_toml = format!(
        r#"[package]
name = "{admin_crate_name}"
version.workspace = true
edition.workspace = true
license.workspace = true
description = "Admin FFA package for {title}"

[package.metadata.docs.rs]
all-features = true
rustdoc-args = ["--cfg", "docsrs"]

[lib]
crate-type = ["rlib"]

[features]
default = []
csr = ["leptos/csr", "rustok-grid-leptos/csr"]
hydrate = ["leptos/hydrate", "rustok-grid-leptos/hydrate"]
ssr = [
  "leptos/ssr",
  "rustok-grid-leptos/ssr",
  "dep:leptos_axum",
  "dep:rustok-core",
  "dep:{crate_name}",
  "rustok-api/server",
]

[dependencies]
rustok-ui-core.workspace = true
rustok-ui-transport.workspace = true
rustok-ui-i18n.workspace = true
rustok-grid.workspace = true
rustok-grid-leptos = {{ workspace = true, default-features = false }}
leptos-ui.workspace = true
leptos.workspace = true
leptos_axum = {{ workspace = true, optional = true }}
leptos-auth.workspace = true
rustok-graphql.workspace = true
leptos-ui-routing.workspace = true
leptos_router.workspace = true
rustok-api = {{ workspace = true, default-features = false }}

rustok-core = {{ workspace = true, optional = true }}
{crate_name} = {{ path = "..", default-features = false, optional = true }}
chrono = {{ workspace = true, features = ["serde"] }}
thiserror = {{ workspace = true }}
serde = {{ workspace = true, features = ["derive"] }}
serde_json.workspace = true
uuid = {{ workspace = true, features = ["serde"] }}
tracing.workspace = true

[package.metadata.cargo-udeps.ignore]
normal = ["leptos-ui", "leptos-ui-routing", "leptos_axum", "{crate_name}", "rustok-core", "rustok-graphql", "leptos-auth"]
"#
    );
    write_file_content(&admin_dir.join("Cargo.toml"), &cargo_toml, opts.dry_run)?;

    // README.md
    let readme = format!(
        r#"# {admin_crate_name}

Admin UI package for `{slug}`, following Fluid Frontend Architecture (FFA).

## Responsibilities
- Pure Rust view-models and search filtering in `core`
- Decoupled backend communication via `transport`
- Leptos UI components and shared `DataGrid` in `ui`
- Localization catalog in `locales/`
"#
    );
    write_file_content(&admin_dir.join("README.md"), &readme, opts.dry_run)?;

    // locales/en.ftl
    let en_ftl = format!(
        r#"{slug}-title = {title}
{slug}-description = Manage {title} configuration and operational workflows
{slug}-empty = No records found
{slug}-save = Save
{slug}-cancel = Cancel
{slug}-success = Operation completed successfully
{slug}-error = An error occurred
{slug}-search-placeholder = Search {title} records...
{slug}-col-title = Title
{slug}-col-status = Status
{slug}-col-created = Created At
{slug}-col-actions = Actions
{slug}-action-create = Create New
{slug}-action-edit = Edit
{slug}-action-delete = Delete
"#
    );
    write_file_content(
        &admin_dir.join("locales").join("en.ftl"),
        &en_ftl,
        opts.dry_run,
    )?;

    // locales/ru.ftl
    let ru_ftl = format!(
        r#"{slug}-title = {title}
{slug}-description = Управление настройками и рабочими процессами {title}
{slug}-empty = Записи не найдены
{slug}-save = Сохранить
{slug}-cancel = Отмена
{slug}-success = Операция успешно выполнена
{slug}-error = Произошла ошибка
{slug}-search-placeholder = Поиск записей {title}...
{slug}-col-title = Название
{slug}-col-status = Статус
{slug}-col-created = Дата создания
{slug}-col-actions = Действия
{slug}-action-create = Создать
{slug}-action-edit = Изменить
{slug}-action-delete = Удалить
"#
    );
    write_file_content(
        &admin_dir.join("locales").join("ru.ftl"),
        &ru_ftl,
        opts.dry_run,
    )?;

    // src/i18n.rs
    let i18n_rs = "rustok_ui_i18n::declare_module_i18n!();\n";
    write_file_content(
        &admin_dir.join("src").join("i18n.rs"),
        i18n_rs,
        opts.dry_run,
    )?;

    // src/core/mod.rs
    let core_rs = format!(
        r#"/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! Pure Rust view-models, search filters, and state machines for {title} admin UI.
//!
//! INVARIANT (FFA): This module MUST NOT import any UI framework types.
//! All validation, search filtering, and state transition logic must be testable with pure #[test].

use serde::{{Deserialize, Serialize}};

/// Table row representation for {title} admin DataGrid.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct {pascal}AdminTableRow {{
    pub id: String,
    pub title: String,
    pub status: String,
    pub created_at: String,
    pub can_edit: bool,
    pub can_delete: bool,
}}

impl {pascal}AdminTableRow {{
    pub fn new(
        id: impl Into<String>,
        title: impl Into<String>,
        status: impl Into<String>,
        created_at: impl Into<String>,
    ) -> Self {{
        Self {{
            id: id.into(),
            title: title.into(),
            status: status.into(),
            created_at: created_at.into(),
            can_edit: true,
            can_delete: true,
        }}
    }}
}}

/// Pure in-memory search filter for {title} admin records.
pub fn filter_rows(
    rows: &[{pascal}AdminTableRow],
    query: Option<&str>,
) -> Vec<{pascal}AdminTableRow> {{
    match query {{
        Some(q) if !q.trim().is_empty() => {{
            let normalized = q.trim().to_lowercase();
            rows.iter()
                .filter(|r| {{
                    r.title.to_lowercase().contains(&normalized)
                        || r.status.to_lowercase().contains(&normalized)
                }})
                .cloned()
                .collect()
        }}
        _ => rows.to_vec(),
    }}
}}

#[cfg(test)]
mod tests {{
    use super::*;

    #[test]
    fn test_filter_rows_matches_title_and_status() {{
        let rows = vec![
            {pascal}AdminTableRow::new("1", "Production Workspace", "active", "2026-01-01"),
            {pascal}AdminTableRow::new("2", "Archived Channel", "archived", "2026-01-02"),
        ];

        let filtered = filter_rows(&rows, Some("production"));
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].id, "1");

        let status_filtered = filter_rows(&rows, Some("archived"));
        assert_eq!(status_filtered.len(), 1);
        assert_eq!(status_filtered[0].id, "2");

        let all = filter_rows(&rows, None);
        assert_eq!(all.len(), 2);
    }}
}}
"#
    );
    write_file_content(
        &admin_dir.join("src").join("core").join("mod.rs"),
        &core_rs,
        opts.dry_run,
    )?;

    // src/transport/mod.rs
    let transport_rs = format!(
        r#"/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! Transport facade connecting UI to backend capabilities.
//! UI components must NEVER call raw endpoints directly.

#[derive(Clone, Debug, Default)]
pub struct {pascal}AdminTransport;

impl {pascal}AdminTransport {{
    pub fn new() -> Self {{
        Self
    }}
}}
"#
    );
    write_file_content(
        &admin_dir.join("src").join("transport").join("mod.rs"),
        &transport_rs,
        opts.dry_run,
    )?;

    // src/ui/mod.rs
    let ui_rs = format!(
        r#"/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! UI components, shared DataGrid, and search integration for {title} admin.

use leptos::prelude::*;
use rustok_grid::column::GridColumnDef;
use rustok_grid::filter::GridFilterType;
use rustok_grid::pagination::GridPagination;
use rustok_grid_leptos::prelude::*;

use crate::core::{{filter_rows, {pascal}AdminTableRow}};
use crate::i18n::t;

#[component]
pub fn {pascal}AdminPage() -> impl IntoView {{
    let (search_query, set_search_query) = signal(String::new());
    let pagination = RwSignal::new(GridPagination::default());

    // Sample/initial data rows — in production, loaded via transport facade
    let sample_rows = vec![
        {pascal}AdminTableRow::new("rec-1", "Default {title} Workspace", "active", "2026-01-01"),
        {pascal}AdminTableRow::new("rec-2", "Secondary {title} Profile", "draft", "2026-01-02"),
    ];
    let rows_store = StoredValue::new(sample_rows);

    let filtered_rows = Memo::new(move |_| {{
        let query = search_query.get();
        let rows = rows_store.get_value();
        filter_rows(&rows, if query.trim().is_empty() {{ None }} else {{ Some(&query) }})
    }});

    // Columns specification for shared DataGrid
    let columns = vec![
        GridColumnDef::checkbox(),
        GridColumnDef::new("title", t(None, "{slug}-col-title", "Title")).filter(GridFilterType::text()),
        GridColumnDef::new("status", t(None, "{slug}-col-status", "Status")),
        GridColumnDef::new("created_at", t(None, "{slug}-col-created", "Created At")),
        GridColumnDef::new("actions", t(None, "{slug}-col-actions", "Actions")),
    ];

    let cell_renderer = Callback::new(move |(row, col_id): ({pascal}AdminTableRow, String)| {{
        let row_id = row.id.clone();
        match col_id.as_str() {{
            "title" => view! {{
                <span class="font-medium text-foreground">{{row.title}}</span>
            }}.into_any(),
            "status" => view! {{
                <span class="inline-flex items-center px-2 py-0.5 rounded text-xs font-medium bg-secondary text-secondary-foreground">
                    {{row.status}}
                </span>
            }}.into_any(),
            "created_at" => view! {{
                <span class="text-xs text-muted-foreground">{{row.created_at}}</span>
            }}.into_any(),
            "actions" => view! {{
                <div class="flex items-center gap-2">
                    // RBAC-guarded action buttons
                    <button
                        type="button"
                        class="text-xs text-primary hover:underline font-medium"
                        disabled=!row.can_edit
                        on:click=move |_| {{
                            tracing::info!(id = %row_id, "edit action triggered");
                        }}
                    >
                        {{t(None, "{slug}-action-edit", "Edit")}}
                    </button>
                    <button
                        type="button"
                        class="text-xs text-destructive hover:underline font-medium"
                        disabled=!row.can_delete
                        on:click=move |_| {{
                            tracing::info!(id = %row_id, "delete action triggered");
                        }}
                    >
                        {{t(None, "{slug}-action-delete", "Delete")}}
                    </button>
                </div>
            }}.into_any(),
            _ => ().into_any(),
        }}
    }});

    view! {{
        <div class="p-6 space-y-6">
            // Header with RBAC create button & title
            <div class="flex flex-col sm:flex-row gap-4 items-start sm:items-center justify-between">
                <div>
                    <h1 class="text-2xl font-bold tracking-tight text-foreground">"{title}"</h1>
                    <p class="text-sm text-muted-foreground">"{desc}"</p>
                </div>
                <div class="flex items-center gap-2">
                    <button
                        type="button"
                        class="px-4 py-2 text-sm font-medium rounded-lg bg-primary text-primary-foreground hover:bg-primary/90 transition-colors shadow-sm"
                        on:click=move |_| {{
                            tracing::info!("create new {slug} record initiated");
                        }}
                    >
                        {{t(None, "{slug}-action-create", "Create New")}}
                    </button>
                </div>
            </div>

            // Toolbar: Local table search & filter input
            <div class="flex items-center justify-between gap-4">
                <div class="relative w-72">
                    <input
                        type="text"
                        placeholder={{t(None, "{slug}-search-placeholder", "Search records...")}}
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
                    key_fn=|row: &{pascal}AdminTableRow| row.id.clone()
                    cell_renderer=cell_renderer
                    empty_message=t(None, "{slug}-empty", "No records found")
                    pagination=pagination
                    on_row_click=Callback::new(|_: {pascal}AdminTableRow| ())
                />
            </div>
        </div>
    }}
}}
"#
    );
    write_file_content(
        &admin_dir.join("src").join("ui").join("mod.rs"),
        &ui_rs,
        opts.dry_run,
    )?;

    // src/lib.rs
    let lib_rs = format!(
        r#"/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

pub mod core;
pub mod i18n;
pub mod transport;
pub mod ui;

pub use core::*;
pub use transport::*;
pub use ui::{pascal}AdminPage;
"#
    );
    write_file_content(&admin_dir.join("src").join("lib.rs"), &lib_rs, opts.dry_run)?;

    Ok(())
}

fn generate_storefront_ui(opts: &ScaffoldOptions, module_dir: &Path, pascal: &str) -> Result<()> {
    let slug = &opts.slug;
    let title = &opts.title;
    let desc = &opts.description;
    let crate_name = opts.crate_name();
    let sf_crate_name = format!("{crate_name}-storefront");
    let sf_dir = module_dir.join("storefront");

    // Cargo.toml
    let cargo_toml = format!(
        r#"[package]
name = "{sf_crate_name}"
version.workspace = true
edition.workspace = true
license.workspace = true
description = "Storefront FFA package for {title}"

[package.metadata.docs.rs]
all-features = true
rustdoc-args = ["--cfg", "docsrs"]

[lib]
crate-type = ["rlib"]

[features]
default = []
csr = ["leptos/csr"]
hydrate = ["leptos/hydrate"]
ssr = [
  "leptos/ssr",
  "dep:leptos_axum",
  "dep:rustok-core",
  "dep:{crate_name}",
  "rustok-api/server",
]

[dependencies]
rustok-ui-core.workspace = true
rustok-ui-transport.workspace = true
rustok-ui-i18n.workspace = true
leptos-ui.workspace = true
leptos.workspace = true
leptos_axum = {{ workspace = true, optional = true }}
leptos-auth.workspace = true
rustok-graphql.workspace = true
leptos-ui-routing.workspace = true
leptos_router.workspace = true
rustok-api = {{ workspace = true, default-features = false }}

rustok-core = {{ workspace = true, optional = true }}
{crate_name} = {{ path = "..", default-features = false, optional = true }}
chrono = {{ workspace = true, features = ["serde"] }}
thiserror = {{ workspace = true }}
serde = {{ workspace = true, features = ["derive"] }}
serde_json.workspace = true
uuid = {{ workspace = true, features = ["serde"] }}
tracing.workspace = true

[package.metadata.cargo-udeps.ignore]
normal = ["leptos-ui", "leptos-auth", "rustok-graphql", "leptos-ui-routing", "leptos_axum", "{crate_name}", "rustok-core"]
"#
    );
    write_file_content(&sf_dir.join("Cargo.toml"), &cargo_toml, opts.dry_run)?;

    // README.md
    let readme = format!(
        r#"# {sf_crate_name}

Storefront UI package for `{slug}`, following Fluid Frontend Architecture (FFA).

## Responsibilities
- Pure Rust view-models in `core`
- Decoupled backend communication via `transport`
- Leptos UI components and templates in `ui`
- Localization catalog in `locales/`
"#
    );
    write_file_content(&sf_dir.join("README.md"), &readme, opts.dry_run)?;

    // locales/en.ftl
    let en_ftl = format!(
        r#"{slug}-storefront-title = {title}
{slug}-storefront-description = {desc}
{slug}-storefront-empty = No items available
"#
    );
    write_file_content(
        &sf_dir.join("locales").join("en.ftl"),
        &en_ftl,
        opts.dry_run,
    )?;

    // locales/ru.ftl
    let ru_ftl = format!(
        r#"{slug}-storefront-title = {title}
{slug}-storefront-description = {desc}
{slug}-storefront-empty = Нет доступных элементов
"#
    );
    write_file_content(
        &sf_dir.join("locales").join("ru.ftl"),
        &ru_ftl,
        opts.dry_run,
    )?;

    // src/i18n.rs
    let i18n_rs = "rustok_ui_i18n::declare_module_i18n!();\n";
    write_file_content(&sf_dir.join("src").join("i18n.rs"), i18n_rs, opts.dry_run)?;

    // src/core.rs
    let core_rs = format!(
        r#"/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! Pure Rust view-models and input validation for {title} storefront UI.

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct {pascal}StorefrontViewModel {{
    pub title: String,
}}

impl {pascal}StorefrontViewModel {{
    pub fn new(title: String) -> Self {{
        Self {{ title }}
    }}
}}

#[cfg(test)]
mod tests {{
    use super::*;

    #[test]
    fn test_storefront_view_model() {{
        let vm = {pascal}StorefrontViewModel::new("Sample Title".to_string());
        assert_eq!(vm.title, "Sample Title");
    }}
}}
"#
    );
    write_file_content(
        &sf_dir.join("src").join("core").join("mod.rs"),
        &core_rs,
        opts.dry_run,
    )?;

    // src/transport/mod.rs
    let transport_rs = format!(
        r#"/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! Transport facade for {title} storefront.

#[derive(Clone, Debug, Default)]
pub struct {pascal}StorefrontTransport;

impl {pascal}StorefrontTransport {{
    pub fn new() -> Self {{
        Self
    }}
}}
"#
    );
    write_file_content(
        &sf_dir.join("src").join("transport").join("mod.rs"),
        &transport_rs,
        opts.dry_run,
    )?;

    // src/ui/mod.rs
    let ui_rs = format!(
        r#"/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use leptos::prelude::*;
use crate::i18n::t;

#[component]
pub fn {pascal}StorefrontView() -> impl IntoView {{
    view! {{
        <div class="rustok-{slug}-storefront-widget p-4">
            <h2 class="text-xl font-semibold">{{t(None, "{slug}-storefront-title", "{title}")}}</h2>
            <p class="text-muted-foreground">{{t(None, "{slug}-storefront-description", "{desc}")}}</p>
        </div>
    }}
}}
"#
    );
    write_file_content(
        &sf_dir.join("src").join("ui").join("mod.rs"),
        &ui_rs,
        opts.dry_run,
    )?;

    // src/lib.rs
    let lib_rs = format!(
        r#"/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

pub mod core;
pub mod i18n;
pub mod transport;
pub mod ui;

pub use core::*;
pub use transport::*;
pub use ui::{pascal}StorefrontView;
"#
    );
    write_file_content(&sf_dir.join("src").join("lib.rs"), &lib_rs, opts.dry_run)?;

    Ok(())
}
