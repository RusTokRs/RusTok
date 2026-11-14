use leptos::prelude::*;
use leptos::task::spawn_local;
use rustok_auth_ui::ui::leptos::hooks::{use_current_user, use_tenant, use_token};
use rustok_forms_leptos::{
    Form, FormError, FormField, FormInput, FormLabel, FormMessage, FormSelect, FormTextarea,
    SubmitButton,
};
use rustok_grid_leptos::prelude::*;
use rustok_ui_core::UiRouteContext;
use rustok_ui_forms::{DirtyTracker, FieldOption, FormState};
use rustok_ui_leptos::{Alert, Button};

use super::components::loaded_resource;
use crate::i18n::t;
use crate::model::BlogCategoryDetail;
use crate::{core, transport};

/// Full owner list goes to DataGrid; it owns client pagination and never receives
/// a pre-sliced page. Localized editor values belong only to the target locale.
#[component]
pub fn BlogCategoriesAdmin() -> impl IntoView {
    let route = use_context::<UiRouteContext>().unwrap_or_default();
    let locale = StoredValue::new(route.locale);
    let token = use_token();
    let tenant = use_tenant();
    let current_user = use_current_user();
    let editor_principal = Memo::new(move |_| core::EditorPrincipal {
        actor_id: current_user.get().map(|user| user.id),
        tenant: tenant.get(),
    });
    let refresh = RwSignal::new(0_u64);
    let busy = RwSignal::new(false);
    let active_request = RwSignal::new(None::<core::EditorRequest>);
    let loaded = RwSignal::new(Option::<BlogCategoryDetail>::None);
    let delete_id = RwSignal::new(Option::<String>::None);
    let name = RwSignal::new(String::new());
    let slug = RwSignal::new(String::new());
    let description = RwSignal::new(String::new());
    let parent = RwSignal::new(String::new());
    let position = RwSignal::new("0".to_string());
    let search = RwSignal::new(String::new());
    let state = RwSignal::new(FormState::idle());
    let dirty = RwSignal::new(DirtyTracker::new());
    let copy = move |key: &str, fallback: &str| {
        locale.with_value(|value| t(value.as_deref(), key, fallback))
    };
    let category_locale = locale.get_value();
    let categories = loaded_resource(
        move || {
            (
                token.get(),
                tenant.get(),
                refresh.get(),
                category_locale.clone(),
            )
        },
        move |(token, tenant, _, locale)| async move {
            transport::fetch_category_details(token, tenant, locale).await
        },
    );
    let rows = Signal::derive(move || {
        categories
            .get()
            .and_then(Result::ok)
            .unwrap_or_default()
            .into_iter()
            .filter(|row| {
                core::blog_category_matches_query(
                    &core::blog_category_admin_table_row_view(row),
                    &search.get(),
                )
            })
            .collect::<Vec<_>>()
    });
    let reset = Callback::new(move |()| {
        active_request.set(None);
        busy.set(false);
        loaded.set(None);
        name.set(String::new());
        slug.set(String::new());
        description.set(String::new());
        parent.set(String::new());
        position.set("0".to_string());
        state.set(FormState::idle());
        dirty.update(|value| value.reset());
    });
    Effect::new(move |_| {
        let _principal = editor_principal.get();
        reset.run(());
    });
    let edit = Callback::new(move |category: BlogCategoryDetail| {
        if busy.get_untracked() {
            return;
        }
        active_request.set(None);
        let target = core::category_editor_copy(&category);
        name.set(target.name);
        description.set(target.description);
        slug.set(target.slug);
        parent.set(category.parent_id.clone().unwrap_or_default());
        position.set(category.position.to_string());
        loaded.set(Some(category));
        state.set(FormState::idle());
        dirty.update(|value| value.reset());
    });
    let submit = Callback::new(move |()| {
        if busy.get_untracked() {
            return;
        }
        let category = loaded.get_untracked();
        let command = core::prepare_blog_category_save_command(
            core::BlogCategoryFormInput {
                editing_category_id: category.as_ref().map(|value| value.id.as_str()),
                loaded_category: category.as_ref(),
                locale: locale.get_value().as_deref().unwrap_or_default(),
                name: &name.get_untracked(),
                slug: &slug.get_untracked(),
                description: &description.get_untracked(),
                parent_id: Some(&parent.get_untracked()),
                position: &position.get_untracked(),
            },
            copy(
                "blog.categories.error.invalidName",
                "Enter a category name of 1 to 120 characters.",
            ),
            copy(
                "blog.categories.error.invalidPosition",
                "Category order must be a non-negative number.",
            ),
            copy("blog.editor.slugRequired", "Canonical slug is required"),
        );
        let command = match command {
            Ok(command) => command,
            Err(issue) => {
                state.set(FormState::with_form_error(issue.message));
                return;
            }
        };
        let request = core::EditorRequest::new(
            category.as_ref().map(|row| row.id.clone()),
            locale.get_value().unwrap_or_default(),
        );
        active_request.set(Some(request.clone()));
        let token_value = token.get_untracked();
        let tenant_value = tenant.get_untracked();
        busy.set(true);
        state.set(FormState::submitting());
        spawn_local(async move {
            let result = match command.operation {
                core::BlogCategorySaveOperation::Create(draft) => {
                    transport::create_category(token_value, tenant_value, draft)
                        .await
                        .map(|_| ())
                }
                core::BlogCategorySaveOperation::Update {
                    category_id,
                    update,
                } => transport::update_category(token_value, tenant_value, category_id, update)
                    .await
                    .map(|_| ()),
            };
            if !request.is_current(active_request.get_untracked().as_ref()) {
                return;
            }
            match result {
                Ok(()) => {
                    reset.run(());
                    refresh.update(|value| *value += 1);
                }
                Err(_) => state.set(FormState::with_form_error(copy(
                    "blog.categories.error.save",
                    "Failed to save category",
                ))),
            }
            busy.set(false);
        });
    });
    let remove = Callback::new(move |id: String| {
        if busy.get_untracked() {
            return;
        }
        let request = core::EditorRequest::new(
            loaded.get_untracked().map(|row| row.id),
            locale.get_value().unwrap_or_default(),
        );
        active_request.set(Some(request.clone()));
        let token_value = token.get_untracked();
        let tenant_value = tenant.get_untracked();
        busy.set(true);
        spawn_local(async move {
            match transport::delete_category(token_value, tenant_value, id.clone()).await {
                _ if !request.is_current(active_request.get_untracked().as_ref()) => return,
                Ok(true) => {
                    if loaded.get_untracked().is_some_and(|row| row.id == id) {
                        reset.run(());
                    }
                    delete_id.set(None);
                    refresh.update(|value| *value += 1);
                }
                Ok(false) | Err(_) => state.set(FormState::with_form_error(copy(
                    "blog.categories.error.delete",
                    "Failed to delete category",
                ))),
            }
            busy.set(false);
        });
    });
    let cells = Callback::new(move |(row, column): (BlogCategoryDetail, String)| {
        match column.as_str() {
            "name" => view! { <span>{row.name}</span> }.into_any(),
            "slug" => view! { <span>{row.slug}</span> }.into_any(),
            "locale" => view! { <span>{row.effective_locale}</span> }.into_any(),
            "position" => view! { <span>{row.position}</span> }.into_any(),
            "actions" => {
                let edit_row = row.clone();
                let id = row.id.clone();
                let confirm_id = row.id;
                let visible_id = confirm_id.clone();
                view! { <div class="flex gap-2">
                <Button disabled=busy on_click=Box::new(move || edit.run(edit_row.clone()))>{copy("blog.categories.edit", "Edit")}</Button>
                <Button disabled=busy on_click=Box::new(move || delete_id.set(Some(id.clone())))>{copy("blog.categories.delete", "Delete")}</Button>
                <Show when=move || delete_id.get().as_deref() == Some(visible_id.as_str())>
                    <Button disabled=busy on_click=Box::new({ let id = confirm_id.clone(); move || remove.run(id.clone()) })>{copy("blog.categories.confirmDelete", "Confirm delete")}</Button>
                    <Button on_click=Box::new(move || delete_id.set(None))>{copy("blog.categories.cancel", "Cancel")}</Button>
                </Show>
            </div> }.into_any()
            }
            _ => ().into_any(),
        }
    });
    let columns = core::category_grid_columns([
        &copy("blog.categories.nameHeader", "Name"),
        &copy("blog.categories.slugHeader", "Slug"),
        &copy("blog.categories.localeHeader", "Locale"),
        &copy("blog.categories.orderHeader", "Order"),
        &copy("blog.categories.actionsHeader", "Actions"),
    ]);
    view! {
        <section class="space-y-6">
            <div class="flex items-center justify-between"><h1>{copy("blog.categories.title", "Blog categories")}</h1>
                <Button disabled=busy on_click=Box::new(move || reset.run(()))>{copy("blog.categories.new", "New category")}</Button>
            </div>
            <FormInput value=search on_input=move |value| search.set(value) aria_label=copy("blog.categories.searchPlaceholder", "Search categories") />
            {move || categories.get().and_then(Result::err).map(|_| view! {
                <Alert>{copy("blog.categories.error.load", "Failed to load categories")}
                    <Button on_click=Box::new(move || refresh.update(|value| *value += 1))>{copy("blog.editor.retry", "Retry")}</Button>
                </Alert>
            })}
            <DataGrid columns=columns data=rows key_fn=|row: &BlogCategoryDetail| row.id.clone()
                cell_renderer=cells empty_message=copy("blog.categories.empty", "No categories created yet.") />
            {move || loaded.get().filter(|row| row.locale != row.effective_locale).map(|row| view! {
                <Alert><p>{copy("blog.editor.fallbackNotice", "The requested translation does not exist. The source below is read-only; enter target-language title and content to create it.")}</p>
                    <p lang=row.effective_locale>{row.name}</p><p>{row.description.unwrap_or_default()}</p></Alert>
            })}
            <Form state=state dirty_tracker=dirty auto_submitting=false on_submit=submit class="space-y-4">
                <FormError />
                <FormField name="name"><FormLabel>{copy("blog.categories.form.name", "Name")}</FormLabel><FormInput value=name on_input=move |value| name.set(value) disabled=busy /><FormMessage /></FormField>
                <FormField name="slug"><FormLabel>{copy("blog.categories.form.slug", "Slug")}</FormLabel><FormInput value=slug on_input=move |value| slug.set(value) disabled=busy /><FormMessage /></FormField>
                <FormField name="description"><FormLabel>{copy("blog.categories.form.description", "Description")}</FormLabel><FormTextarea value=description on_input=move |value| description.set(value) disabled=busy /><FormMessage /></FormField>
                <Show when=move || loaded.get().is_none()>
                    <FormField name="parent"><FormLabel>{copy("blog.categories.form.parent", "Parent category")}</FormLabel>
                        {move || { let mut options = vec![FieldOption::new("", copy("blog.categories.form.root", "Root (no parent)"))];
                            if let Some(Ok(rows)) = categories.get() { options.extend(rows.into_iter().map(|row| FieldOption::new(row.id, row.name))); }
                            view! { <FormSelect value=parent options=options on_change=move |value| parent.set(value) disabled=busy /> }
                        }}<FormMessage />
                    </FormField>
                    <FormField name="position"><FormLabel>{copy("blog.categories.form.position", "Order")}</FormLabel><FormInput input_type="number" min="0" value=position on_input=move |value| position.set(value) disabled=busy /><FormMessage /></FormField>
                </Show>
                <SubmitButton disabled=busy>{move || if loaded.get().is_some() { copy("blog.categories.update", "Update category") } else { copy("blog.categories.create", "Create category") }}</SubmitButton>
            </Form>
        </section>
    }
}
