use leptos::prelude::*;
use leptos_router::hooks::use_navigate;
use leptos_ui_routing::use_route_query_writer;
use rustok_auth_ui::ui::leptos::hooks::{use_tenant, use_token};
use rustok_grid::{
    ColumnFilters, GridPagination, PaginationMode, RowSelection, DEFAULT_PAGE_SIZE,
};
use rustok_grid_leptos::prelude::*;
use rustok_ui_core::UiRouteContext;

use super::catalog_facets::AdminCatalogFacetPanel;
use crate::catalog_controls::{
    build_product_admin_catalog_controls_labels, serialize_attribute_filters,
};
use crate::catalog_transport;
use crate::core::{
    ProductKind, filter_products, item_product_kind, product_admin_pagination_from_response,
    product_grid_columns,
};
use crate::facets::{build_product_admin_facet_labels, build_product_admin_facet_panel};
use crate::model::{ProductList, ProductListItem};

#[component]
pub fn ProductGridPage() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let route_query_writer = use_route_query_writer();
    let locale = route_context.locale.clone();
    let is_ru = locale.as_deref() == Some("ru");
    let base_route = route_context.admin_module_route_base("product");
    let token = use_token();
    let tenant = use_tenant();

    let (refresh_nonce, set_refresh_nonce) = signal(0_u64);
    let (is_busy, set_is_busy) = signal(false);
    let (error_msg, set_error_msg) = signal(Option::<String>::None);
    let (search_query, set_search_query) = signal(String::new());
    let (dropdown_open, set_dropdown_open) = signal(false);
    let (confirm_delete_id, set_confirm_delete_id) = signal(Option::<String>::None);
    let (confirm_bulk, set_confirm_bulk) = signal(false);

    // Grid state
    let columns = product_grid_columns(locale.as_deref());
    let (filters, set_filters) = signal(ColumnFilters::new());
    let selection = RwSignal::new(RowSelection::new());

    // Catalog controls travel through the URL, so the owner list contract
    // (category, typed attribute filters, deterministic date order and page)
    // survives navigation, refresh, and deep links.
    let catalog_controls = catalog_transport::product_admin_list_input_from_route();
    provide_context(catalog_controls.clone());
    let requested_page = usize::try_from(catalog_controls.page).unwrap_or(usize::MAX);
    let requested_page_size =
        usize::try_from(catalog_controls.per_page).unwrap_or(DEFAULT_PAGE_SIZE);
    let pagination = RwSignal::new(GridPagination::new(1, requested_page_size, 0));
    let (last_requested_pagination, set_last_requested_pagination) =
        signal((requested_page, requested_page_size));
    let (page_request_nonce, set_page_request_nonce) = signal(0_u64);
    let request_current_page = {
        let pagination = pagination;
        let last_requested_pagination = last_requested_pagination;
        let set_last_requested_pagination = set_last_requested_pagination;
        let set_page_request_nonce = set_page_request_nonce;
        move || {
            let state = pagination.get_untracked();
            let requested = (state.page, state.page_size);
            if last_requested_pagination.get_untracked() != requested {
                set_last_requested_pagination.set(requested);
                set_page_request_nonce.update(|nonce| *nonce = nonce.saturating_add(1));
            }
        }
    };
    let (infinite_items, set_infinite_items) = signal(Vec::<ProductListItem>::new());
    let (last_loaded_page, set_last_loaded_page) = signal(0_usize);
    let (last_applied_request, set_last_applied_request) = signal(None::<u64>);
    let (last_auth_context, set_last_auth_context) =
        signal((token.get_untracked(), tenant.get_untracked()));
    let auth_context_query_writer = route_query_writer.clone();
    Effect::new(move |_| {
        let current_context = (token.get(), tenant.get());
        if current_context != last_auth_context.get_untracked() {
            set_last_auth_context.set(current_context);
            pagination.update(|state| state.set_page(1));
            let state = pagination.get_untracked();
            set_last_requested_pagination.set((state.page, state.page_size));
            set_page_request_nonce.update(|nonce| *nonce = nonce.saturating_add(1));
            set_last_applied_request.set(None);
            set_infinite_items.set(Vec::new());
            set_last_loaded_page.set(0);
            auth_context_query_writer.replace_value("page", "1");
        }
    });
    let catalog_labels = build_product_admin_catalog_controls_labels(locale.as_deref());
    let current_category = catalog_controls.category_id.clone().unwrap_or_default();
    let current_attribute_filters =
        serialize_attribute_filters(catalog_controls.attribute_filters.as_slice());
    let current_sort_by = catalog_controls
        .sort_by
        .clone()
        .unwrap_or_else(|| "published_at".to_string());
    let current_sort_direction = catalog_controls
        .sort_direction
        .clone()
        .unwrap_or_else(|| "desc".to_string());

    // The grid is served from an `Async` admin route, so its data has to be resolved during the
    // render: a `LocalResource` is `pending()` on the server and the reader gets the fallback.
    // See DECISIONS/2026-11-13-standalone-admin-editor-delivery.md.
    let options_locale = locale.clone();
    let catalog_options_resource = Resource::new(
        move || {
            (
                token.get(),
                tenant.get(),
                options_locale.clone().unwrap_or_default(),
            )
        },
        move |(tok, ten, loc)| async move {
            catalog_transport::fetch_catalog_search_options(tok, ten, loc).await
        },
    );

    // The owner returns one page plus its total. The grid receives the page
    // and metadata separately so it never slices a partial catalog locally.
    let res_locale = locale.clone();
    let res_controls = catalog_controls.clone();
    let products_resource = Resource::new(
        move || {
            (
                token.get_untracked(),
                tenant.get_untracked(),
                res_locale.clone(),
                res_controls.clone(),
                last_requested_pagination.get_untracked(),
                page_request_nonce.get(),
            )
        },
        move |(tok, ten, loc, mut controls, (page, page_size), request_id)| async move {
            controls.page = u64::try_from(page)
                .unwrap_or(u64::MAX)
                .min(i32::MAX as u64);
            controls.per_page = u64::try_from(page_size).unwrap_or(DEFAULT_PAGE_SIZE as u64);
            let bootstrap = catalog_transport::fetch_bootstrap(tok.clone(), ten.clone())
                .await
                .map_err(|e| e.to_string())?;
            let tenant_id = bootstrap.current_tenant.id;
            let mut products = catalog_transport::fetch_products(
                tok.clone(),
                ten.clone(),
                tenant_id.clone(),
                loc.clone(),
                controls.clone(),
            )
            .await
            .map_err(|e| e.to_string())?;

            // Some owner implementations return an empty out-of-range page
            // rather than clamping it. Normalize and retry once so SSR and
            // hydration both receive rows for the last real page.
            if page_request_nonce.get_untracked() != request_id {
                return Ok::<(u64, ProductList), String>((request_id, products));
            }
            let current_pagination = pagination.get_untracked();
            let normalized =
                product_admin_pagination_from_response(current_pagination, &products);
            let response_page = usize::try_from(products.page).unwrap_or(usize::MAX);
            let response_page_size =
                usize::try_from(products.per_page).unwrap_or(DEFAULT_PAGE_SIZE);
            if (response_page, response_page_size) != (normalized.page, normalized.page_size) {
                controls.page = u64::try_from(normalized.page).unwrap_or(u64::MAX);
                controls.per_page =
                    u64::try_from(normalized.page_size).unwrap_or(DEFAULT_PAGE_SIZE as u64);
                products = catalog_transport::fetch_products(
                    tok,
                    ten,
                    tenant_id,
                    loc,
                    controls,
                )
                .await
                .map_err(|e| e.to_string())?;

                let retried =
                    product_admin_pagination_from_response(current_pagination, &products);
                let retried_page = usize::try_from(products.page).unwrap_or(usize::MAX);
                let retried_page_size =
                    usize::try_from(products.per_page).unwrap_or(DEFAULT_PAGE_SIZE);
                if (retried_page, retried_page_size) != (retried.page, retried.page_size) {
                    return Err("Product catalog pagination response is inconsistent".to_string());
                }
            }

            let final_pagination =
                product_admin_pagination_from_response(pagination.get_untracked(), &products);
            if page_request_nonce.get_untracked() == request_id {
                // Resource fetchers also run during SSR; update the controlled
                // grid state here so the first server-rendered page has the
                // owner's page number and total before client effects exist.
                pagination.set(final_pagination);
            }

            Ok::<(u64, ProductList), String>((request_id, products))
        },
    );
    let products_page = Memo::new(move |_| {
        let request_id = page_request_nonce.get();
        products_resource
            .get()
            .and_then(Result::ok)
            .and_then(|(loaded_request, products)| {
                (loaded_request == request_id).then_some(products)
            })
    });
    // Server metadata is authoritative. Out-of-range deep links are clamped
    // to the last real page and fetched again before rendering.
    let page_result_writer = route_query_writer.clone();
    Effect::new(move |_| {
        let Some(products) = products_page.get() else {
            return;
        };
        let request_id = page_request_nonce.get();
        if last_applied_request.get_untracked() == Some(request_id) {
            return;
        }

        let current = pagination.get_untracked();
        let requested = last_requested_pagination.get_untracked();
        let next = product_admin_pagination_from_response(current, &products);
        pagination.set(next);
        set_last_applied_request.set(Some(request_id));

        let response_page = usize::try_from(products.page).unwrap_or(usize::MAX);
        let response_page_size =
            usize::try_from(products.per_page).unwrap_or(DEFAULT_PAGE_SIZE);
        let normalized_pagination = (next.page, next.page_size);
        if (response_page, response_page_size) != normalized_pagination {
            set_last_requested_pagination.set(normalized_pagination);
            set_page_request_nonce.update(|nonce| *nonce = nonce.saturating_add(1));
            page_result_writer.update(
                vec![
                    ("page".to_string(), Some(next.page.to_string())),
                    ("per_page".to_string(), Some(next.page_size.to_string())),
                ],
                true,
            );
            return;
        }

        if requested != normalized_pagination {
            set_last_requested_pagination.set(normalized_pagination);
            page_result_writer.update(
                vec![
                    ("page".to_string(), Some(next.page.to_string())),
                    ("per_page".to_string(), Some(next.page_size.to_string())),
                ],
                true,
            );
        }

        if next.mode == PaginationMode::Infinite
            && next.page > 1
            && last_loaded_page.get_untracked().saturating_add(1) == next.page
        {
            set_infinite_items.update(|items| items.extend(products.items.clone()));
        } else {
            set_infinite_items.set(products.items.clone());
        }
        set_last_loaded_page.set(next.page);
    });

    // Refreshes and mode changes restart at page one. In infinite mode this
    // also prevents a refreshed page from being appended to a stale prefix.
    let (observed_refresh_nonce, set_observed_refresh_nonce) = signal(0_u64);
    let refresh_query_writer = route_query_writer.clone();
    Effect::new(move |_| {
        let generation = refresh_nonce.get();
        if generation != observed_refresh_nonce.get_untracked() {
            set_observed_refresh_nonce.set(generation);
            pagination.update(|state| state.set_page(1));
            let state = pagination.get_untracked();
            set_last_requested_pagination.set((state.page, state.page_size));
            set_page_request_nonce.update(|nonce| *nonce = nonce.saturating_add(1));
            set_infinite_items.set(Vec::new());
            set_last_loaded_page.set(0);
            refresh_query_writer.replace_value("page", "1");
        }
    });

    let (previous_mode, set_previous_mode) = signal(PaginationMode::Paged);
    let mode_query_writer = route_query_writer.clone();
    Effect::new(move |_| {
        let mode = pagination.get().mode;
        if mode != previous_mode.get_untracked() {
            set_previous_mode.set(mode);
            pagination.update(|state| state.set_page(1));
            let state = pagination.get_untracked();
            set_last_requested_pagination.set((state.page, state.page_size));
            set_page_request_nonce.update(|nonce| *nonce = nonce.saturating_add(1));
            set_infinite_items.set(Vec::new());
            set_last_loaded_page.set(0);
            mode_query_writer.replace_value("page", "1");
        }
    });

    // Owner-counted facet buckets for the same URL-driven controls. Counting is optional: no
    // facet capability, no transport, or no countable attribute all degrade to "no panel" instead
    // of an error, because the grid itself must keep working.
    let facets_locale = locale.clone();
    let facets_controls = catalog_controls.clone();
    let facets_resource = Resource::new(
        move || {
            (
                token.get(),
                tenant.get(),
                facets_locale.clone(),
                facets_controls.clone(),
                refresh_nonce.get(),
            )
        },
        move |(tok, ten, loc, controls, _generation)| async move {
            let bootstrap = catalog_transport::fetch_bootstrap(tok.clone(), ten.clone())
                .await
                .ok()?;
            let options = catalog_transport::fetch_catalog_search_options(
                tok.clone(),
                ten.clone(),
                loc.clone().unwrap_or_default(),
            )
            .await
            .ok()?;
            let facet_codes = crate::facets::build_product_admin_facet_codes(&options);
            if facet_codes.is_empty() {
                return Some(Vec::new());
            }
            catalog_transport::fetch_admin_catalog_facets(
                tok,
                ten,
                bootstrap.current_tenant.id,
                loc,
                controls,
                facet_codes,
            )
            .await
        },
    );

    // The panel is a pure view over the owner answer and the controls, so it is rebuilt from the
    // resource instead of being cached: a selection change navigates, and the new render shows the
    // new counts.
    let facet_panel_base = base_route.clone();
    let facet_panel_controls = catalog_controls.clone();
    let facet_panel_labels = build_product_admin_facet_labels(locale.as_deref());

    // Reactive filtering stays layered over the currently loaded page(s);
    // the server-owned total and page boundaries are never recomputed locally.
    let filtered_data = Memo::new(move |_| {
        let raw = if pagination.get().mode == PaginationMode::Infinite {
            infinite_items.get()
        } else {
            products_page
                .get()
                .map(|products| products.items)
                .unwrap_or_default()
        };
        let query = search_query.get().trim().to_lowercase();
        let current_filters = filters.get();

        let filtered = filter_products(&raw, &current_filters);
        if query.is_empty() {
            filtered
        } else {
            filtered
                .into_iter()
                .filter(|item| {
                    item.title.to_lowercase().contains(&query)
                        || item.handle.to_lowercase().contains(&query)
                        || item
                            .seller_id
                            .as_deref()
                            .map(|s| s.to_lowercase().contains(&query))
                            .unwrap_or(false)
                        || item
                            .vendor
                            .as_deref()
                            .map(|v| v.to_lowercase().contains(&query))
                            .unwrap_or(false)
                        || item.tags.iter().any(|t| t.to_lowercase().contains(&query))
                })
                .collect()
        }
    });

    // Bulk actions
    let on_bulk_status = {
        let base_token = token;
        let base_tenant = tenant;
        move |target_status: &'static str| {
            let selected_ids = selection.get().to_vec();
            if selected_ids.is_empty() {
                return;
            }
            set_is_busy.set(true);
            set_error_msg.set(None);

            let tok = base_token.get_untracked();
            let ten = base_tenant.get_untracked();

            leptos::task::spawn_local(async move {
                let Ok(bootstrap) =
                    catalog_transport::fetch_bootstrap(tok.clone(), ten.clone()).await
                else {
                    set_is_busy.set(false);
                    set_error_msg.set(Some(
                        "Failed to load bootstrap for status mutation".to_string(),
                    ));
                    return;
                };

                let mut failed = 0usize;
                let mut first_error = None;
                let total = selected_ids.len();
                for id in selected_ids {
                    if let Err(err) = catalog_transport::change_product_status(
                        tok.clone(),
                        ten.clone(),
                        bootstrap.current_tenant.id.clone(),
                        bootstrap.me.id.clone(),
                        id,
                        target_status,
                    )
                    .await
                    {
                        failed += 1;
                        if first_error.is_none() {
                            first_error = Some(err.to_string());
                        }
                    }
                }

                set_is_busy.set(false);
                selection.update(|s| s.clear());
                set_refresh_nonce.update(|n| *n += 1);
                if let Some(err) = first_error {
                    set_error_msg.set(Some(if is_ru {
                        format!("Статус не изменён у {failed} из {total} товаров: {err}")
                    } else {
                        format!(
                            "The status was not changed for {failed} of {total} products: {err}"
                        )
                    }));
                }
            });
        }
    };

    let on_bulk_delete = {
        let base_token = token;
        let base_tenant = tenant;
        move || {
            let selected_ids = selection.get().to_vec();
            if selected_ids.is_empty() {
                return;
            }
            set_is_busy.set(true);
            set_error_msg.set(None);

            let tok = base_token.get_untracked();
            let ten = base_tenant.get_untracked();

            leptos::task::spawn_local(async move {
                let Ok(bootstrap) =
                    catalog_transport::fetch_bootstrap(tok.clone(), ten.clone()).await
                else {
                    set_is_busy.set(false);
                    set_error_msg.set(Some("Failed to load bootstrap for deletion".to_string()));
                    return;
                };

                let mut failed = 0usize;
                let mut first_error = None;
                let total = selected_ids.len();
                for id in selected_ids {
                    if let Err(err) = catalog_transport::delete_product(
                        tok.clone(),
                        ten.clone(),
                        bootstrap.current_tenant.id.clone(),
                        bootstrap.me.id.clone(),
                        id,
                    )
                    .await
                    {
                        failed += 1;
                        if first_error.is_none() {
                            first_error = Some(err.to_string());
                        }
                    }
                }

                set_is_busy.set(false);
                selection.update(|s| s.clear());
                set_refresh_nonce.update(|n| *n += 1);
                if let Some(err) = first_error {
                    set_error_msg.set(Some(if is_ru {
                        format!("Не удалено {failed} из {total} товаров: {err}")
                    } else {
                        format!("{failed} of {total} products were not deleted: {err}")
                    }));
                }
            });
        }
    };

    // Quick single item status change
    let on_quick_status = {
        let base_token = token;
        let base_tenant = tenant;
        move |id: String, next_status: String| {
            set_is_busy.set(true);
            set_error_msg.set(None);

            let tok = base_token.get_untracked();
            let ten = base_tenant.get_untracked();

            leptos::task::spawn_local(async move {
                let Ok(bootstrap) =
                    catalog_transport::fetch_bootstrap(tok.clone(), ten.clone()).await
                else {
                    set_is_busy.set(false);
                    set_error_msg.set(Some(if is_ru {
                        "Не удалось получить данные сессии".to_string()
                    } else {
                        "Failed to authenticate bootstrap".to_string()
                    }));
                    return;
                };

                match catalog_transport::change_product_status(
                    tok,
                    ten,
                    bootstrap.current_tenant.id,
                    bootstrap.me.id,
                    id,
                    &next_status,
                )
                .await
                {
                    Ok(_) => set_refresh_nonce.update(|n| *n += 1),
                    Err(err) => set_error_msg.set(Some(if is_ru {
                        format!("Статус товара не изменён: {err}")
                    } else {
                        format!("The product status was not changed: {err}")
                    })),
                }

                set_is_busy.set(false);
            });
        }
    };

    // Quick single delete
    let on_quick_delete = {
        let base_token = token;
        let base_tenant = tenant;
        move |id: String| {
            set_is_busy.set(true);
            set_error_msg.set(None);

            let tok = base_token.get_untracked();
            let ten = base_tenant.get_untracked();

            leptos::task::spawn_local(async move {
                let Ok(bootstrap) =
                    catalog_transport::fetch_bootstrap(tok.clone(), ten.clone()).await
                else {
                    set_is_busy.set(false);
                    set_error_msg.set(Some(if is_ru {
                        "Не удалось получить данные сессии".to_string()
                    } else {
                        "Failed to authenticate bootstrap".to_string()
                    }));
                    return;
                };

                match catalog_transport::delete_product(
                    tok,
                    ten,
                    bootstrap.current_tenant.id,
                    bootstrap.me.id,
                    id,
                )
                .await
                {
                    Ok(_) => set_refresh_nonce.update(|n| *n += 1),
                    Err(err) => set_error_msg.set(Some(if is_ru {
                        format!("Товар не удалён: {err}")
                    } else {
                        format!("The product was not deleted: {err}")
                    })),
                }

                set_is_busy.set(false);
            });
        }
    };

    let base_route_for_cell = base_route.clone();
    let base_route_for_click = base_route.clone();
    let on_quick_status_cb = StoredValue::new(on_quick_status);
    let on_quick_delete_cb = StoredValue::new(on_quick_delete);

    // Cell renderer callback
    let cell_locale = locale.clone();
    let cell_renderer = Callback::new(move |(item, col_id): (ProductListItem, String)| {
        let base_route = base_route_for_cell.clone();
        let edit_href = format!("{base_route}/edit/{}", item.id);
        let on_status = on_quick_status_cb.get_value();
        let on_del = on_quick_delete_cb.get_value();

        match col_id.as_str() {
            "image" => {
                view! {
                    <div class="flex items-center justify-center w-full h-full">
                        <div class="w-9 h-9 rounded-lg bg-muted flex items-center justify-center overflow-hidden border border-border/60">
                            <span class="text-base text-muted-foreground/70">"📦"</span>
                        </div>
                    </div>
                }
                .into_any()
            }
            "title" => {
                let tags = item.tags.clone();
                view! {
                    <div class="flex flex-col gap-0.5 max-w-full">
                        <a
                            href=edit_href.clone()
                            class="font-medium text-foreground hover:text-primary transition-colors truncate text-xs"
                            title=item.title.clone()
                        >
                            {item.title.clone()}
                        </a>
                        <div class="flex items-center gap-1.5 flex-wrap">
                            <span class="font-mono text-[10px] text-muted-foreground/70 truncate">
                                {format!("/{}", item.handle)}
                            </span>
                            {tags.into_iter().take(2).map(|tag| {
                                view! {
                                    <span class="inline-flex items-center px-1.5 py-0.2 rounded text-[10px] bg-secondary text-secondary-foreground border border-border/50">
                                        {tag}
                                    </span>
                                }
                            }).collect_view()}
                        </div>
                    </div>
                }
                .into_any()
            }
            "product_type" => {
                let kind = item_product_kind(&item);
                view! {
                    <div class="flex items-center">
                        <span class=kind.badge_class()>
                            {kind.label(cell_locale.as_deref())}
                        </span>
                    </div>
                }
                .into_any()
            }
            "sku" => {
                let display = item
                    .seller_id
                    .as_deref()
                    .or(item.vendor.as_deref())
                    .unwrap_or("—");
                view! {
                    <div class="flex flex-col">
                        <span class="font-mono text-xs text-foreground/90 truncate">{display}</span>
                        {item.vendor.as_ref().map(|v| view! {
                            <span class="text-[10px] text-muted-foreground truncate">{format!("by {}", v)}</span>
                        })}
                    </div>
                }
                .into_any()
            }
            "status" => {
                let status_upper = item.status.to_uppercase();
                let (badge_cls, label) = match status_upper.as_str() {
                    "ACTIVE" => (
                        "bg-emerald-500/10 text-emerald-600 dark:text-emerald-400 border-emerald-500/20",
                        if is_ru { "Активен" } else { "Active" },
                    ),
                    "ARCHIVED" => (
                        "bg-slate-500/10 text-slate-600 dark:text-slate-400 border-slate-500/20",
                        if is_ru { "В архиве" } else { "Archived" },
                    ),
                    _ => (
                        "bg-amber-500/10 text-amber-600 dark:text-amber-400 border-amber-500/20",
                        if is_ru { "Черновик" } else { "Draft" },
                    ),
                };
                view! {
                    <span class=format!("inline-flex items-center px-2 py-0.5 rounded-full text-[11px] font-semibold border {badge_cls}")>
                        {label}
                    </span>
                }
                .into_any()
            }
            "created_at" => {
                let date_str = item.created_at.split('T').next().unwrap_or(&item.created_at);
                view! {
                    <span class="text-xs text-muted-foreground whitespace-nowrap">
                        {date_str.to_string()}
                    </span>
                }
                .into_any()
            }
            "actions" => {
                let item_id_status = item.id.clone();
                let item_id_del = item.id.clone();
                let current_status = item.status.to_uppercase();
                let next_status = if current_status == "ACTIVE" {
                    "DRAFT".to_string()
                } else {
                    "ACTIVE".to_string()
                };
                let status_icon = if current_status == "ACTIVE" { "⏸" } else { "▶" };
                let status_title = if current_status == "ACTIVE" {
                    if is_ru { "Перевести в черновик" } else { "Set to draft" }
                } else {
                    if is_ru { "Опубликовать" } else { "Publish" }
                };

                view! {
                    <div class="flex items-center justify-end gap-1.5">
                        <a
                            href=edit_href
                            class="inline-flex items-center justify-center h-7 px-2.5 rounded-md text-xs font-medium bg-secondary text-secondary-foreground hover:bg-accent transition"
                            title=if is_ru { "Редактировать" } else { "Edit" }
                        >
                            {if is_ru { "Изменить" } else { "Edit" }}
                        </a>
                        <button
                            type="button"
                            class="inline-flex items-center justify-center w-7 h-7 rounded-md text-xs text-muted-foreground hover:text-foreground hover:bg-accent transition"
                            title=status_title
                            on:click=move |_| on_status(item_id_status.clone(), next_status.clone())
                        >
                            {status_icon}
                        </button>
                        {move || {
                            let id_to_check = item_id_del.clone();
                            if confirm_delete_id.get().as_deref() == Some(&id_to_check) {
                                let id_del = id_to_check.clone();
                                view! {
                                    <div class="inline-flex items-center gap-1 animate-in fade-in duration-100">
                                        <button
                                            type="button"
                                            class="inline-flex items-center justify-center h-7 px-2 rounded-md text-xs font-semibold bg-rose-600 text-white hover:bg-rose-700 transition"
                                            title=if is_ru { "Подтвердить удаление" } else { "Confirm deletion" }
                                            on:click=move |_| {
                                                set_confirm_delete_id.set(None);
                                                on_del(id_del.clone());
                                            }
                                        >
                                            "✓"
                                        </button>
                                        <button
                                            type="button"
                                            class="inline-flex items-center justify-center h-7 px-2 rounded-md text-xs border border-border text-muted-foreground hover:text-foreground transition"
                                            title=if is_ru { "Отмена" } else { "Cancel" }
                                            on:click=move |_| set_confirm_delete_id.set(None)
                                        >
                                            "✕"
                                        </button>
                                    </div>
                                }
                                .into_any()
                            } else {
                                let id_del = id_to_check.clone();
                                view! {
                                    <button
                                        type="button"
                                        class="inline-flex items-center justify-center w-7 h-7 rounded-md text-xs text-rose-500 hover:text-rose-700 hover:bg-rose-50 dark:hover:bg-rose-950/40 transition"
                                        title=if is_ru { "Удалить" } else { "Delete" }
                                        on:click=move |_| set_confirm_delete_id.set(Some(id_del.clone()))
                                    >
                                        "🗑"
                                    </button>
                                }
                                .into_any()
                            }
                        }}
                    </div>
                }
                .into_any()
            }
            _ => ().into_any(),
        }
    });

    let navigate = use_navigate();
    let on_row_click = Callback::new(move |item: ProductListItem| {
        let href = format!("{base_route_for_click}/edit/{}", item.id);
        navigate(&href, Default::default());
    });

    let filter_query_writer = route_query_writer.clone();
    let on_filters_change = Callback::new(move |new_filters: ColumnFilters| {
        set_filters.set(new_filters);
        if last_requested_pagination.get_untracked().0 != 1 {
            set_infinite_items.set(Vec::new());
            set_last_loaded_page.set(0);
        }
        filter_query_writer.replace_value("page", "1");
        request_current_page();
    });

    let page_query_writer = route_query_writer.clone();
    let on_page_change = Callback::new(move |page: usize| {
        page_query_writer.replace_value("page", page.to_string());
        request_current_page();
    });

    let page_size_query_writer = route_query_writer.clone();
    let on_page_size_change = Callback::new(move |page_size: usize| {
        set_infinite_items.set(Vec::new());
        set_last_loaded_page.set(0);
        page_size_query_writer.update(
            vec![
                ("page".to_string(), Some("1".to_string())),
                ("per_page".to_string(), Some(page_size.to_string())),
            ],
            true,
        );
        request_current_page();
    });

    let new_simple_href = format!("{base_route}/new?type=simple");
    let new_variable_href = format!("{base_route}/new?type=variable");
    let new_bundle_href = format!("{base_route}/new?type=bundle");
    let new_digital_href = format!("{base_route}/new?type=digital");

    view! {
        <div class="flex flex-col gap-5 w-full">
            // Facet panel: owner-counted buckets for the current admin filter set.
            {move || {
                let facets = facets_resource.get().flatten().unwrap_or_default();
                if facets.is_empty() {
                    return ().into_any();
                }
                let panel = build_product_admin_facet_panel(
                    facet_panel_base.as_str(),
                    facets.as_slice(),
                    &facet_panel_controls,
                    facet_panel_labels.clone(),
                );
                view! { <AdminCatalogFacetPanel panel /> }.into_any()
            }}

            // Top Action Toolbar
            <div class="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4 bg-card rounded-2xl border border-border p-4 shadow-sm">
                <div>
                    <h1 class="text-xl font-bold tracking-tight text-foreground flex items-center gap-2">
                        <span>{if is_ru { "Каталог товаров" } else { "Product Catalog" }}</span>
                        <span class="text-xs font-normal px-2 py-0.5 rounded-full bg-primary/10 text-primary border border-primary/20">
                            {move || pagination.get().total}
                        </span>
                    </h1>
                    <p class="text-xs text-muted-foreground mt-0.5">
                        {if is_ru {
                            "Управление всеми типами товаров: простые, вариативные, комплекты и цифровые"
                        } else {
                            "Manage all product types: simple, variable, bundles, and digital downloads"
                        }}
                    </p>
                </div>

                <div class="flex items-center gap-2.5 flex-wrap">
                    // Search bar
                    <div class="relative min-w-[200px] sm:min-w-[240px]">
                        <input
                            type="text"
                            placeholder=if is_ru {
                                "Поиск загруженных товаров..."
                            } else {
                                "Search loaded products..."
                            }
                            prop:value=move || search_query.get()
                            on:input=move |ev| {
                                set_search_query.set(event_target_value(&ev));
                                if last_requested_pagination.get_untracked().0 != 1 {
                                    set_infinite_items.set(Vec::new());
                                    set_last_loaded_page.set(0);
                                }
                                pagination.update(|p| p.set_page(1));
                                route_query_writer.replace_value("page", "1");
                                request_current_page();
                            }
                            class="w-full text-xs rounded-xl border border-border bg-background px-3 py-2 text-foreground placeholder:text-muted-foreground/60 outline-none focus:border-primary focus:ring-1 focus:ring-primary/20 transition"
                        />
                    </div>

                    // Categories navigation link
                    <a
                        href=format!("{base_route}/categories")
                        class="inline-flex items-center justify-center h-9 px-3 rounded-xl border border-border bg-background text-xs font-medium text-foreground hover:bg-accent transition"
                    >
                        {if is_ru { "📁 Категории" } else { "📁 Categories" }}
                    </a>

                    // Attributes navigation link
                    <a
                        href=format!("{base_route}/attributes")
                        class="inline-flex items-center justify-center h-9 px-3 rounded-xl border border-border bg-background text-xs font-medium text-foreground hover:bg-accent transition"
                    >
                        {if is_ru { "🏷️ Атрибуты" } else { "🏷️ Attributes" }}
                    </a>

                    // Refresh button
                    <button
                        type="button"
                        class="inline-flex items-center justify-center h-9 px-3 rounded-xl border border-border bg-background text-xs font-medium text-foreground hover:bg-accent transition"
                        title=if is_ru { "Обновить" } else { "Refresh" }
                        on:click=move |_| set_refresh_nonce.update(|n| *n += 1)
                    >
                        "↻"
                    </button>

                    // Prominent Split Add Product Dropdown
                    <div class="relative">
                        <div class="inline-flex rounded-xl shadow-sm">
                            <a
                                href=new_simple_href.clone()
                                class="inline-flex items-center justify-center gap-1.5 h-9 px-3.5 rounded-l-xl bg-primary text-primary-foreground text-xs font-semibold hover:bg-primary/90 transition shadow-sm"
                            >
                                <span>"+"</span>
                                <span>{if is_ru { "Добавить товар" } else { "Add Product" }}</span>
                            </a>
                            <button
                                type="button"
                                class="inline-flex items-center justify-center w-8 h-9 rounded-r-xl bg-primary text-primary-foreground border-l border-primary-foreground/20 hover:bg-primary/90 transition text-xs"
                                on:click=move |_| set_dropdown_open.update(|open| *open = !*open)
                            >
                                "▾"
                            </button>
                        </div>

                        // Dropdown menu
                        <Show when=move || dropdown_open.get()>
                            <div
                                class="absolute right-0 top-11 z-50 w-72 rounded-2xl border border-border bg-popover p-2 shadow-xl animate-in fade-in zoom-in-95 duration-100"
                                on:click=move |_| set_dropdown_open.set(false)
                            >
                                <div class="px-2 py-1 text-[11px] font-semibold text-muted-foreground uppercase tracking-wider">
                                    {if is_ru { "Выберите тип товара" } else { "Select Product Type" }}
                                </div>
                                <div class="flex flex-col gap-1 mt-1">
                                    <a
                                        href=new_simple_href.clone()
                                        class="flex items-start gap-2.5 p-2 rounded-xl hover:bg-accent transition-colors"
                                    >
                                        <span class="text-lg">"📦"</span>
                                        <div class="flex flex-col">
                                            <span class="text-xs font-medium text-foreground">{ProductKind::Simple.label(locale.as_deref())}</span>
                                            <span class="text-[10px] text-muted-foreground">{ProductKind::Simple.description(locale.as_deref())}</span>
                                        </div>
                                    </a>
                                    <a
                                        href=new_variable_href.clone()
                                        class="flex items-start gap-2.5 p-2 rounded-xl hover:bg-accent transition-colors"
                                    >
                                        <span class="text-lg">"🎨"</span>
                                        <div class="flex flex-col">
                                            <span class="text-xs font-medium text-foreground">{ProductKind::Variable.label(locale.as_deref())}</span>
                                            <span class="text-[10px] text-muted-foreground">{ProductKind::Variable.description(locale.as_deref())}</span>
                                        </div>
                                    </a>
                                    <a
                                        href=new_bundle_href.clone()
                                        class="flex items-start gap-2.5 p-2 rounded-xl hover:bg-accent transition-colors"
                                    >
                                        <span class="text-lg">"🎁"</span>
                                        <div class="flex flex-col">
                                            <span class="text-xs font-medium text-foreground">{ProductKind::Bundle.label(locale.as_deref())}</span>
                                            <span class="text-[10px] text-muted-foreground">{ProductKind::Bundle.description(locale.as_deref())}</span>
                                        </div>
                                    </a>
                                    <a
                                        href=new_digital_href.clone()
                                        class="flex items-start gap-2.5 p-2 rounded-xl hover:bg-accent transition-colors"
                                    >
                                        <span class="text-lg">"💾"</span>
                                        <div class="flex flex-col">
                                            <span class="text-xs font-medium text-foreground">{ProductKind::Digital.label(locale.as_deref())}</span>
                                            <span class="text-[10px] text-muted-foreground">{ProductKind::Digital.description(locale.as_deref())}</span>
                                        </div>
                                    </a>
                                </div>
                            </div>
                        </Show>
                    </div>
                </div>
            </div>

            // Error alert
            <Show when=move || error_msg.get().is_some()>
                <div class="rounded-xl border border-destructive/30 bg-destructive/10 px-4 py-2.5 text-xs text-destructive flex items-center justify-between">
                    <span>{move || error_msg.get().unwrap_or_default()}</span>
                    <button type="button" class="text-xs font-bold" on:click=move |_| set_error_msg.set(None)>"✕"</button>
                </div>
            </Show>

            // Bulk Actions Bar (Visible when items selected)
            <Show when=move || !selection.get().is_empty()>
                <div class="flex items-center justify-between gap-3 bg-primary/5 border border-primary/20 rounded-xl px-4 py-2.5 animate-in fade-in slide-in-from-top-1 duration-150">
                    <div class="flex items-center gap-2">
                        <span class="w-2 h-2 rounded-full bg-primary animate-pulse" />
                        <span class="text-xs font-semibold text-foreground">
                            {move || format!("{} {}", selection.get().count(), if is_ru { "выбрано" } else { "selected" })}
                        </span>
                    </div>
                    {move || {
                        if confirm_bulk.get() {
                            let bulk_del = on_bulk_delete;
                            view! {
                                <div class="flex items-center gap-2 animate-in fade-in duration-100">
                                    <span class="text-xs font-medium text-rose-600 dark:text-rose-400">
                                        {if is_ru { "Удалить выбранные товары безвозвратно?" } else { "Permanently delete selected products?" }}
                                    </span>
                                    <button
                                        type="button"
                                        class="h-7 px-3 rounded-lg bg-rose-600 text-white text-[11px] font-semibold hover:bg-rose-700 transition"
                                        on:click=move |_| {
                                            set_confirm_bulk.set(false);
                                            bulk_del();
                                        }
                                    >
                                        {if is_ru { "Да, удалить" } else { "Yes, delete" }}
                                    </button>
                                    <button
                                        type="button"
                                        class="h-7 px-2.5 rounded-lg border border-border bg-background text-[11px] hover:bg-accent transition"
                                        on:click=move |_| set_confirm_bulk.set(false)
                                    >
                                        {if is_ru { "Отмена" } else { "Cancel" }}
                                    </button>
                                </div>
                            }
                            .into_any()
                        } else {
                            let bulk_st_act = on_bulk_status;
                            let bulk_st_drf = on_bulk_status;
                            let bulk_st_arc = on_bulk_status;
                            view! {
                                <div class="flex items-center gap-2">
                                    <button
                                        type="button"
                                        class="h-7 px-2.5 rounded-lg border border-border bg-background text-[11px] font-medium text-foreground hover:bg-accent transition"
                                        on:click=move |_| bulk_st_act("ACTIVE")
                                    >
                                        {if is_ru { "Активировать" } else { "Set Active" }}
                                    </button>
                                    <button
                                        type="button"
                                        class="h-7 px-2.5 rounded-lg border border-border bg-background text-[11px] font-medium text-foreground hover:bg-accent transition"
                                        on:click=move |_| bulk_st_drf("DRAFT")
                                    >
                                        {if is_ru { "В черновик" } else { "Set Draft" }}
                                    </button>
                                    <button
                                        type="button"
                                        class="h-7 px-2.5 rounded-lg border border-border bg-background text-[11px] font-medium text-foreground hover:bg-accent transition"
                                        on:click=move |_| bulk_st_arc("ARCHIVED")
                                    >
                                        {if is_ru { "В архив" } else { "Archive" }}
                                    </button>
                                    <button
                                        type="button"
                                        class="h-7 px-2.5 rounded-lg border border-rose-300 dark:border-rose-900 bg-rose-500/10 text-[11px] font-medium text-rose-600 dark:text-rose-400 hover:bg-rose-500/20 transition"
                                        on:click=move |_| set_confirm_bulk.set(true)
                                    >
                                        {if is_ru { "Удалить выбранные" } else { "Delete Selected" }}
                                    </button>
                                    <button
                                        type="button"
                                        class="h-7 px-2 rounded-lg text-[11px] text-muted-foreground hover:text-foreground transition"
                                        on:click=move |_| selection.update(|s| s.clear())
                                    >
                                        {if is_ru { "Снять выбор" } else { "Clear" }}
                                    </button>
                                </div>
                            }
                            .into_any()
                        }
                    }}
                </div>
            </Show>

            // Catalog controls: the GET form keeps the owner list contract
            // (category, typed attribute filters, deterministic date order) in
            // the URL, so deep links and refreshes resolve the same list.
            <form
                method="get"
                class="grid gap-3 rounded-2xl border border-border bg-card p-4 shadow-sm md:grid-cols-2 xl:grid-cols-[minmax(0,1.4fr)_minmax(0,1fr)_minmax(0,1fr)_minmax(0,1fr)_auto] xl:items-end"
            >
                <input type="hidden" name="page" value="1" />
                <input
                    type="hidden"
                    name="per_page"
                    prop:value=move || pagination.get().page_size.to_string()
                />
                <div class="space-y-1 xl:col-span-5">
                    <h2 class="text-sm font-semibold text-foreground">{catalog_labels.title.clone()}</h2>
                    <p class="text-xs text-muted-foreground">{catalog_labels.subtitle.clone()}</p>
                </div>
                <label class="grid gap-2 text-xs text-foreground">
                    <span class="font-medium">{catalog_labels.category.clone()}</span>
                    <select
                        name="category_id"
                        class="rounded-xl border border-border bg-background px-3 py-2 text-xs text-foreground outline-none transition focus:border-primary"
                        prop:value=current_category.clone()
                    >
                        <option value="">{catalog_labels.all_categories.clone()}</option>
                        {move || catalog_options_resource
                            .get()
                            .and_then(Result::ok)
                            .map(|options| options.category_options.into_iter().map(|option| {
                                view! { <option value=option.value>{option.label}</option> }
                            }).collect_view())
                            .unwrap_or_default()}
                    </select>
                </label>
                <label class="grid gap-2 text-xs text-foreground">
                    <span class="font-medium">{catalog_labels.attribute_filters.clone()}</span>
                    <input
                        name="attribute_filters"
                        type="text"
                        value=current_attribute_filters.clone()
                        placeholder=catalog_labels.attribute_filters_placeholder.clone()
                        class="rounded-xl border border-border bg-background px-3 py-2 text-xs text-foreground placeholder:text-muted-foreground/60 outline-none transition focus:border-primary"
                    />
                    <span class="text-[10px] text-muted-foreground">{catalog_labels.attribute_filters_help.clone()}</span>
                </label>
                <label class="grid gap-2 text-xs text-foreground">
                    <span class="font-medium">{catalog_labels.sort_by.clone()}</span>
                    <select
                        name="sort_by"
                        class="rounded-xl border border-border bg-background px-3 py-2 text-xs text-foreground outline-none transition focus:border-primary"
                        prop:value=current_sort_by.clone()
                    >
                        <option value="published_at">{catalog_labels.published_at.clone()}</option>
                        <option value="created_at">{catalog_labels.created_at.clone()}</option>
                    </select>
                </label>
                <label class="grid gap-2 text-xs text-foreground">
                    <span class="font-medium">{catalog_labels.sort_direction.clone()}</span>
                    <select
                        name="sort_direction"
                        class="rounded-xl border border-border bg-background px-3 py-2 text-xs text-foreground outline-none transition focus:border-primary"
                        prop:value=current_sort_direction.clone()
                    >
                        <option value="desc">{catalog_labels.descending.clone()}</option>
                        <option value="asc">{catalog_labels.ascending.clone()}</option>
                    </select>
                </label>
                <button
                    type="submit"
                    class="inline-flex h-9 items-center justify-center rounded-xl bg-primary px-4 text-xs font-medium text-primary-foreground transition hover:bg-primary/90"
                >
                    {catalog_labels.apply.clone()}
                </button>
            </form>

            // Main DataGrid
            <DataGrid
                columns=columns
                data=Signal::derive(move || filtered_data.get())
                key_fn=|item: &ProductListItem| item.id.clone()
                cell_renderer=cell_renderer
                is_loading=Signal::derive(move || {
                    let resource_result = products_resource.get();
                    is_busy.get()
                        || (products_page.get().is_none()
                            && resource_result.map(|result| result.is_ok()).unwrap_or(true))
                })
                empty_message=if is_ru { "Товары не найдены" } else { "No products found" }.to_string()
                selection=selection
                pagination=pagination
                on_page_change=on_page_change
                on_page_size_change=on_page_size_change
                on_filter_change=on_filters_change
                on_row_click=on_row_click
            />
        </div>
    }
}
