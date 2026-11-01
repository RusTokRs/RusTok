use leptos::ev::SubmitEvent;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_auth::hooks::{use_tenant, use_token};
use rustok_api::WritePathIssue;
use rustok_ui_core::UiRouteContext;

use crate::core;
use crate::i18n::t;
use crate::model::BlogCategoryDetail;
use crate::transport;

use super::components::local_resource;

/// Category management workspace rendered for the `/modules/blog/categories`
/// child route. Capability parity with the Next admin categories page: list
/// with client-side search (name/slug/description), create with parent and
/// canonical position, edit of localized copy (name/slug/description with
/// clear semantics) and delete with an explicit confirmation step. Moving a
/// category in the hierarchy or changing the sibling position stays an owner
/// command, exactly as in the Next admin.
#[component]
pub fn BlogCategoriesAdmin() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let ui_locale = route_context.locale.clone();
    // The Next modal derives the command locale from the admin locale, not
    // from the list contents; the Leptos route locale plays the same role.
    let default_locale = ui_locale.clone().unwrap_or_default();

    let token = use_token();
    let tenant = use_tenant();

    let shell_classes = core::blog_post_admin_shell_classes_view();
    let form_field_classes = core::blog_post_admin_editor_field_classes_view();

    let (refresh_nonce, set_refresh_nonce) = signal(0_u64);
    let (busy_key, set_busy_key) = signal(Option::<String>::None);
    let (submit_error, set_submit_error) = signal(Option::<WritePathIssue>::None);
    let (search, set_search) = signal(String::new());
    let (editing_category_id, set_editing_category_id) = signal(Option::<String>::None);
    let (editing_loaded_description, set_editing_loaded_description) =
        signal(Option::<String>::None);
    let (delete_confirm_id, set_delete_confirm_id) = signal(Option::<String>::None);
    let (name, set_name) = signal(String::new());
    let (slug, set_slug) = signal(String::new());
    let (description, set_description) = signal(String::new());
    let (parent_id, set_parent_id) = signal(String::new());
    let (position, set_position) = signal("0".to_string());

    // The Next list pages through the owner list in the admin locale (the
    // GraphQL resolver falls back to the request locale the same way); the
    // Leptos route locale plays that role on the native path.
    let list_locale = default_locale.clone();
    let categories_resource = local_resource(
        move || (token.get(), tenant.get(), list_locale.clone(), refresh_nonce.get()),
        move |(token_value, tenant_value, locale_value, _)| async move {
            transport::fetch_category_details(
                token_value,
                tenant_value,
                core::locale_arg(locale_value.as_str()),
            )
            .await
        },
    );

    let load_error_label = t(
        ui_locale.as_deref(),
        "blog.categories.error.load",
        "Failed to load categories",
    );
    let empty_label = t(
        ui_locale.as_deref(),
        "blog.categories.empty",
        "No categories created yet.",
    );
    let search_placeholder = t(
        ui_locale.as_deref(),
        "blog.categories.searchPlaceholder",
        "Search categories...",
    );
    let new_label = t(ui_locale.as_deref(), "blog.categories.new", "New Category");
    let locale_header = t(ui_locale.as_deref(), "blog.categories.localeHeader", "Locale");
    let name_header = t(ui_locale.as_deref(), "blog.categories.nameHeader", "Name");
    let slug_header = t(ui_locale.as_deref(), "blog.categories.slugHeader", "Slug");
    let order_header = t(ui_locale.as_deref(), "blog.categories.orderHeader", "Order");
    let actions_header = t(ui_locale.as_deref(), "blog.categories.actionsHeader", "Actions");
    let edit_label = t(ui_locale.as_deref(), "blog.categories.edit", "Edit");
    let delete_label = t(ui_locale.as_deref(), "blog.categories.delete", "Delete");
    let confirm_delete_label = t(
        ui_locale.as_deref(),
        "blog.categories.confirmDelete",
        "Confirm delete",
    );
    let cancel_label = t(ui_locale.as_deref(), "blog.categories.cancel", "Cancel");
    let create_new_instead_label = t(
        ui_locale.as_deref(),
        "blog.categories.createNewInstead",
        "Create new instead",
    );
    let editing_banner_label = t(
        ui_locale.as_deref(),
        "blog.categories.editing",
        "Editing category",
    );
    let parent_label = t(
        ui_locale.as_deref(),
        "blog.categories.form.parent",
        "Parent category",
    );
    let root_label = t(
        ui_locale.as_deref(),
        "blog.categories.form.root",
        "Root (no parent)",
    );
    let position_label = t(
        ui_locale.as_deref(),
        "blog.categories.form.position",
        "Order",
    );
    let create_category_label = t(
        ui_locale.as_deref(),
        "blog.categories.create",
        "Create category",
    );
    let update_category_label = t(
        ui_locale.as_deref(),
        "blog.categories.update",
        "Update category",
    );

    // The resource resolves into row/error signals, matching the posts list
    // pattern: render reads the signals and refreshes keep the stale rows
    // until the new page lands (no skeleton flash between mutations).
    let (categories_rows, set_categories_rows) = signal(Vec::<BlogCategoryDetail>::new());
    let (categories_error, set_categories_error) = signal(Option::<String>::new());
    let (categories_resolved, set_categories_resolved) = signal(false);
    let effect_load_error_label = load_error_label.clone();
    Effect::new(move |_| {
        match categories_resource.get() {
            Some(Ok(categories)) => {
                set_categories_rows.set(categories);
                set_categories_error.set(None);
                set_categories_resolved.set(true);
            }
            Some(Err(_)) => {
                set_categories_error.set(Some(effect_load_error_label.clone()));
                set_categories_resolved.set(true);
            }
            None => {}
        }
    });

    let reset_form = Callback::new(move |_: ()| {
        set_editing_category_id.set(None);
        set_editing_loaded_description.set(None);
        set_delete_confirm_id.set(None);
        set_name.set(String::new());
        set_slug.set(String::new());
        set_description.set(String::new());
        set_parent_id.set(String::new());
        set_position.set("0".to_string());
        set_submit_error.set(None);
    });

    let start_editing = Callback::new(move |category: BlogCategoryDetail| {
        set_editing_category_id.set(Some(category.id));
        set_editing_loaded_description.set(category.description.clone());
        set_delete_confirm_id.set(None);
        set_name.set(category.name);
        set_slug.set(category.slug);
        set_description.set(category.description.unwrap_or_default());
        set_parent_id.set(category.parent_id.unwrap_or_default());
        set_position.set(category.position.to_string());
        set_submit_error.set(None);
    });

    let submit_locale = ui_locale.clone();
    let submit = Callback::new(move |ev: SubmitEvent| {
        ev.prevent_default();
        set_submit_error.set(None);
        let submit_ui_locale = submit_locale.clone();

        let name_too_short_message = t(
            submit_ui_locale.as_deref(),
            "blog.categories.error.nameTooShort",
            "Category name must be at least 2 characters.",
        );
        let invalid_position_message = t(
            submit_ui_locale.as_deref(),
            "blog.categories.error.invalidPosition",
            "Category order must be a non-negative number.",
        );
        let failure_message = t(
            submit_ui_locale.as_deref(),
            "blog.categories.error.save",
            "Failed to save category",
        );

        let parent_value = parent_id.get_untracked();
        let command = match core::prepare_blog_category_save_command(
            core::BlogCategoryFormInput {
                editing_category_id: editing_category_id.get_untracked().as_deref(),
                loaded_description: editing_loaded_description.get_untracked().as_deref(),
                locale: default_locale.as_str(),
                name: &name.get_untracked(),
                slug: &slug.get_untracked(),
                description: &description.get_untracked(),
                parent_id: Some(parent_value.as_str()),
                position: &position.get_untracked(),
            },
            name_too_short_message,
            invalid_position_message,
        ) {
            Ok(command) => command,
            Err(issue) => {
                set_submit_error.set(Some(issue));
                return;
            }
        };

        let token_value = token.get_untracked();
        let tenant_value = tenant.get_untracked();
        set_busy_key.set(Some(command.busy_key.clone()));

        spawn_local(async move {
            let result = match command.operation {
                core::BlogCategorySaveOperation::Create(draft) => {
                    transport::create_category(token_value, tenant_value, draft).await.map(|_| ())
                }
                core::BlogCategorySaveOperation::Update { category_id, update } => {
                    transport::update_category(token_value, tenant_value, category_id, update)
                        .await
                        .map(|_| ())
                }
            };

            match result {
                Ok(()) => {
                    reset_form.run(());
                    set_refresh_nonce.update(|value| *value += 1);
                }
                Err(err) => {
                    set_submit_error.set(Some(core::blog_post_transport_failure_issue(
                        &failure_message,
                        &err.to_string(),
                    )));
                }
            }

            set_busy_key.set(None);
        });
    });

    let delete_failure_message = t(
        ui_locale.as_deref(),
        "blog.categories.error.delete",
        "Failed to delete category",
    );
    let on_delete = Callback::new(move |category_id: String| {
        set_submit_error.set(None);
        set_delete_confirm_id.set(None);
        let busy = core::busy_key_for_category_delete(category_id.as_str());
        set_busy_key.set(Some(busy));

        spawn_local(async move {
            match transport::delete_category(token.get_untracked(), tenant.get_untracked(), category_id.clone()).await {
                Ok(_) => {
                    if editing_category_id.get_untracked().as_deref() == Some(category_id.as_str()) {
                        reset_form.run(());
                    }
                    set_refresh_nonce.update(|value| *value += 1);
                }
                Err(err) => {
                    set_submit_error.set(Some(core::blog_post_transport_failure_issue(
                        &delete_failure_message,
                        &err.to_string(),
                    )));
                }
            }

            set_busy_key.set(None);
        });
    });

    view! {
        <div class=shell_classes.page>
            <header class=shell_classes.header>
                <div class=shell_classes.header_content>
                    <span class=shell_classes.badge>
                        {t(ui_locale.as_deref(), "blog.badge", "blog")}
                    </span>
                    <h1 class=shell_classes.title>
                        {t(ui_locale.as_deref(), "blog.categories.title", "Blog Categories")}
                    </h1>
                    <p class=shell_classes.subtitle>
                        {t(
                            ui_locale.as_deref(),
                            "blog.categories.subtitle",
                            "Taxonomy category management through the Blog owner contract.",
                        )}
                    </p>
                </div>
            </header>

            <section class=shell_classes.layout>
                <div class=shell_classes.list_card>
                    <div class="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
                        <input
                            type="text"
                            class=shell_classes.locale_filter_input
                            placeholder=search_placeholder.clone()
                            prop:value=search
                            on:input=move |ev| set_search.set(event_target_value(&ev))
                        />
                        <button
                            type="button"
                            class=form_field_classes.submit_button
                            disabled=move || busy_key.get().is_some()
                            on:click=move |_| reset_form.run(())
                        >
                            {new_label.clone()}
                        </button>
                    </div>

                    {move || {
                        let categories = categories_rows.get();
                        let load_error = categories_error.get();
                        let resolved = categories_resolved.get();
                        let query = search.get();
                        let busy = busy_key.get();
                        let confirm_id_source = delete_confirm_id.get();
                        view! {
                            {load_error.map(|message| view! {
                                <div class=shell_classes.load_error>{message}</div>
                            })}
                            {if !resolved {
                                view! {
                                    <div class=shell_classes.skeleton_stack>
                                        {(0..4).map(|_| view! {
                                            <div class=shell_classes.skeleton_row></div>
                                        }).collect_view()}
                                    </div>
                                }.into_any()
                            } else if categories.is_empty() {
                                view! {
                                    <div class="py-10 text-center text-sm text-muted-foreground">
                                        {empty_label.clone()}
                                    </div>
                                }.into_any()
                            } else {
                                view! {
                                    <div class="overflow-x-auto">
                                        <table class="w-full text-left text-sm">
                                            <thead class="border-b border-border text-xs uppercase text-muted-foreground">
                                                <tr>
                                                    <th class="px-4 py-3">{name_header.clone()}</th>
                                                    <th class="px-4 py-3">{slug_header.clone()}</th>
                                                    <th class="px-4 py-3">{locale_header.clone()}</th>
                                                    <th class="px-4 py-3">{order_header.clone()}</th>
                                                    <th class="px-4 py-3 text-right">{actions_header.clone()}</th>
                                                </tr>
                                            </thead>
                                            <tbody class="divide-y divide-border">
                                                {categories
                                                    .iter()
                                                    .filter(|category| {
                                                        let row = core::blog_category_admin_table_row_view(category);
                                                        core::blog_category_matches_query(&row, &query)
                                                    })
                                                    .map(|category| {
                                                        let row = core::blog_category_admin_table_row_view(category);
                                                        let edit_category = category.clone();
                                                        let delete_id = row.id.clone();
                                                        let confirm_id = row.id.clone();
                                                        let confirming = confirm_id_source
                                                            .as_deref()
                                                            .is_some_and(|id| id == row.id.as_str());
                                                        view! {
                                                            <tr class="transition-colors hover:bg-muted/50">
                                                                <td class="px-4 py-3">
                                                                    <div class="font-medium">{row.name.clone()}</div>
                                                                    {(!row.description.is_empty()).then(|| view! {
                                                                        <div class="max-w-xs truncate text-xs text-muted-foreground">
                                                                            {row.description.clone()}
                                                                        </div>
                                                                    })}
                                                                </td>
                                                                <td class="px-4 py-3 font-mono text-xs text-muted-foreground">
                                                                    {row.slug.clone()}
                                                                </td>
                                                                <td class="px-4 py-3 text-xs text-muted-foreground">
                                                                    {row.locale.clone()}
                                                                </td>
                                                                <td class="px-4 py-3 text-xs text-muted-foreground">
                                                                    {row.position}
                                                                </td>
                                                                <td class="space-x-2 px-4 py-3 text-right">
                                                                    {if confirming {
                                                                        view! {
                                                                            <span class="inline-flex items-center gap-2">
                                                                                <button
                                                                                    type="button"
                                                                                    class="rounded-lg bg-destructive px-3 py-1.5 text-xs font-medium text-destructive-foreground hover:bg-destructive/90"
                                                                                    disabled=busy.is_some()
                                                                                    on:click=move |_| on_delete.run(confirm_id.clone())
                                                                                >
                                                                                    {confirm_delete_label.clone()}
                                                                                </button>
                                                                                <button
                                                                                    type="button"
                                                                                    class="rounded-lg border border-border px-3 py-1.5 text-xs text-foreground hover:bg-muted"
                                                                                    disabled=busy.is_some()
                                                                                    on:click=move |_| set_delete_confirm_id.set(None)
                                                                                >
                                                                                    {cancel_label.clone()}
                                                                                </button>
                                                                            </span>
                                                                        }.into_any()
                                                                    } else {
                                                                        view! {
                                                                            <span class="inline-flex items-center gap-2">
                                                                                <button
                                                                                    type="button"
                                                                                    class="rounded-lg border border-border px-3 py-1.5 text-xs text-foreground hover:bg-muted"
                                                                                    disabled=busy.is_some()
                                                                                    on:click=move |_| start_editing.run(edit_category.clone())
                                                                                >
                                                                                    {edit_label.clone()}
                                                                                </button>
                                                                                <button
                                                                                    type="button"
                                                                                    class="rounded-lg bg-destructive px-3 py-1.5 text-xs font-medium text-destructive-foreground hover:bg-destructive/90"
                                                                                    disabled=busy.is_some()
                                                                                    on:click=move |_| set_delete_confirm_id.set(Some(delete_id.clone()))
                                                                                >
                                                                                    {delete_label.clone()}
                                                                                </button>
                                                                            </span>
                                                                        }.into_any()
                                                                    }}
                                                                </td>
                                                            </tr>
                                                        }
                                                    })
                                                    .collect_view()}
                                            </tbody>
                                        </table>
                                    </div>
                                }.into_any()
                            }}
                        }
                    }}
                </div>

                <div class=shell_classes.sidebar>
                    <section class=shell_classes.form_card>
                        <div class=shell_classes.form_header>
                            <h2 class=shell_classes.form_title>
                                {t(ui_locale.as_deref(), "blog.categories.form.title", "Category")}
                            </h2>
                            <p class=shell_classes.form_subtitle>
                                {t(
                                    ui_locale.as_deref(),
                                    "blog.categories.form.subtitle",
                                    "Create a category or edit the selected one.",
                                )}
                            </p>
                        </div>

                        <Show when=move || submit_error.get().is_some()>
                            <div class=shell_classes.load_error>
                                {move || {
                                    submit_error
                                        .get()
                                        .map(|issue| issue.message)
                                        .unwrap_or_default()
                                }}
                            </div>
                        </Show>

                        <Show when=move || editing_category_id.get().is_some()>
                            <div class="flex items-center justify-between gap-3 rounded-lg border border-primary/40 bg-primary/5 px-4 py-2">
                                <div class="text-sm text-muted-foreground">
                                    {move || {
                                        let label = editing_banner_label.clone();
                                        let current_name = name.get();
                                        if current_name.is_empty() {
                                            label
                                        } else {
                                            format!("{label}: {current_name}")
                                        }
                                    }}
                                </div>
                                <button
                                    type="button"
                                    class="text-xs font-medium text-primary hover:underline"
                                    on:click=move |_| reset_form.run(())
                                >
                                    {create_new_instead_label.clone()}
                                </button>
                            </div>
                        </Show>

                        <form class="mt-5 space-y-4" on:submit=move |event: SubmitEvent| submit.run(event)>
                            <label class=shell_classes.locale_filter_label>
                                <span class=form_field_classes.label_text>
                                    {t(ui_locale.as_deref(), "blog.categories.form.name", "Name")}
                                </span>
                                <input
                                    type="text"
                                    class=form_field_classes.text_input
                                    prop:value=name
                                    on:input=move |ev| set_name.set(event_target_value(&ev))
                                />
                            </label>

                            <label class=shell_classes.locale_filter_label>
                                <span class=form_field_classes.label_text>
                                    {t(ui_locale.as_deref(), "blog.categories.form.slug", "Slug (optional)")}
                                </span>
                                <input
                                    type="text"
                                    class=form_field_classes.text_input
                                    prop:value=slug
                                    on:input=move |ev| set_slug.set(event_target_value(&ev))
                                />
                            </label>

                            <label class=shell_classes.locale_filter_label>
                                <span class=form_field_classes.label_text>
                                    {t(ui_locale.as_deref(), "blog.categories.form.description", "Description")}
                                </span>
                                <textarea
                                    class=form_field_classes.textarea_short
                                    prop:value=description
                                    on:input=move |ev| set_description.set(event_target_value(&ev))
                                />
                            </label>

                            {move || {
                                let editing = editing_category_id.get().is_some();
                                let selected_parent = parent_id.get();
                                let editing_id = editing_category_id.get();
                                let options = categories_rows.get();
                                let parent_label = parent_label.clone();
                                let root_label = root_label.clone();
                                let position_label = position_label.clone();
                                view! {
                                    <label class=shell_classes.locale_filter_label>
                                        <span class=form_field_classes.label_text>
                                            {parent_label}
                                        </span>
                                        <select
                                            class=form_field_classes.text_input
                                            disabled=editing
                                            prop:value=selected_parent
                                            on:change=move |ev| set_parent_id.set(event_target_value(&ev))
                                        >
                                            <option value="">
                                                {root_label}
                                            </option>
                                            {options
                                                .into_iter()
                                                .filter(|category| {
                                                    Some(category.id.clone()) != editing_id.clone()
                                                })
                                                .map(|category| view! {
                                                    <option value=category.id.clone()>
                                                        {category.name.clone()}
                                                    </option>
                                                })
                                                .collect_view()}
                                        </select>
                                    </label>
                                    {(!editing).then(move || view! {
                                        <label class=shell_classes.locale_filter_label>
                                            <span class=form_field_classes.label_text>
                                                {position_label}
                                            </span>
                                            <input
                                                type="number"
                                                class=form_field_classes.text_input
                                                prop:value=position
                                                on:input=move |ev| set_position.set(event_target_value(&ev))
                                            />
                                        </label>
                                    })}
                                }
                            }}

                            <div class="flex items-center justify-end space-x-2 pt-2">
                                <button
                                    type="button"
                                    class="rounded-lg border border-border px-3 py-1.5 text-xs text-foreground hover:bg-muted"
                                    disabled=move || busy_key.get().is_some()
                                    on:click=move |_| reset_form.run(())
                                >
                                    {t(ui_locale.as_deref(), "blog.categories.reset", "Reset")}
                                </button>
                                <button
                                    type="submit"
                                    class=form_field_classes.submit_button
                                    disabled=move || busy_key.get().is_some()
                                >
                                    {move || {
                                        if editing_category_id.get().is_some() {
                                            update_category_label.clone()
                                        } else {
                                            create_category_label.clone()
                                        }
                                    }}
                                </button>
                            </div>
                        </form>
                    </section>
                </div>
            </section>
        </div>
    }
}
