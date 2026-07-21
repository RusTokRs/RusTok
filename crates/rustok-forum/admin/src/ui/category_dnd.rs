use leptos::ev::DragEvent;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_auth::hooks::{use_tenant, use_token};

use crate::core::{
    category_card_view_model, forum_admin_action_button_class, ForumAdminActionButtonKind,
    ForumAdminCategoryRenderLabels,
};
use crate::i18n::t;
use crate::model::{
    category_drop_move_request, CategoryDropPlacement, CategoryListItem, CategoryMoveRequest,
};
use crate::transport;

#[component]
pub(super) fn CategoryDndGrid(
    items: Vec<CategoryListItem>,
    editing_id: Option<String>,
    busy_key: Option<String>,
    on_edit: Callback<String>,
    on_delete: Callback<String>,
    set_refresh_nonce: WriteSignal<u64>,
    locale: Option<String>,
) -> impl IntoView {
    let token = use_token();
    let tenant = use_tenant();
    let (dragged_id, set_dragged_id) = signal(Option::<String>::None);
    let (move_error, set_move_error) = signal(Option::<String>::None);
    let (move_busy, set_move_busy) = signal(false);

    let category_labels = ForumAdminCategoryRenderLabels {
        no_description: t(
            locale.as_deref(),
            "forum.render.noDescription",
            "No description yet.",
        ),
        topics_count_template: t(
            locale.as_deref(),
            "forum.render.topicsCount",
            "topics: {count}",
        ),
        replies_count_template: t(
            locale.as_deref(),
            "forum.render.repliesCount",
            "replies: {count}",
        ),
        icon_template: t(locale.as_deref(), "forum.render.icon", "icon: {value}"),
        editing: t(locale.as_deref(), "forum.render.editing", "Editing"),
        edit: t(locale.as_deref(), "forum.render.edit", "Edit"),
    };
    let delete_label = t(locale.as_deref(), "forum.render.delete", "Delete");
    let drag_label = t(
        locale.as_deref(),
        "forum.categories.dragHandle",
        "Drag category",
    );
    let before_label = t(
        locale.as_deref(),
        "forum.categories.dropBefore",
        "Drop before",
    );
    let inside_label = t(
        locale.as_deref(),
        "forum.categories.dropInside",
        "Drop inside",
    );
    let root_label = t(
        locale.as_deref(),
        "forum.categories.dropRoot",
        "Drop here to move to the end of root categories",
    );
    let archived_label = t(
        locale.as_deref(),
        "forum.categories.archived",
        "Archived",
    );
    let move_error_prefix = t(
        locale.as_deref(),
        "forum.error.moveCategory",
        "Failed to move category",
    );

    let items_for_drop = items.clone();
    let execute_drop = Callback::new(
        move |(target_id, placement): (Option<String>, CategoryDropPlacement)| {
            if move_busy.get_untracked() {
                return;
            }
            let Some(category_id) = dragged_id.get_untracked() else {
                return;
            };
            set_dragged_id.set(None);
            set_move_error.set(None);

            let request = match category_drop_move_request(
                &items_for_drop,
                category_id.as_str(),
                target_id.as_deref(),
                placement,
            ) {
                Ok(Some(request)) => request,
                Ok(None) => return,
                Err(error) => {
                    set_move_error.set(Some(error));
                    return;
                }
            };
            let token_value = token.get_untracked();
            let tenant_value = tenant.get_untracked();
            let move_error_prefix = move_error_prefix.clone();
            set_move_busy.set(true);
            spawn_local(async move {
                let CategoryMoveRequest {
                    category_id,
                    parent_id,
                    position,
                } = request;
                match transport::move_category(
                    token_value,
                    tenant_value,
                    category_id,
                    parent_id,
                    position,
                )
                .await
                {
                    Ok(()) => set_refresh_nonce.update(|value| *value += 1),
                    Err(error) => set_move_error.set(Some(format!(
                        "{}: {}",
                        move_error_prefix, error
                    ))),
                }
                set_move_busy.set(false);
            });
        },
    );

    let root_drop = execute_drop;
    view! {
        <div class="mt-6 space-y-3">
            {move || move_error.get().map(|error| view! {
                <div class="rounded-xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">
                    {error}
                </div>
            })}
            <div
                class="rounded-2xl border border-dashed border-border bg-muted/30 px-4 py-3 text-center text-xs font-medium text-muted-foreground transition hover:border-primary/40 hover:text-foreground"
                on:dragover=move |event: DragEvent| event.prevent_default()
                on:drop=move |event: DragEvent| {
                    event.prevent_default();
                    root_drop.run((None, CategoryDropPlacement::RootEnd));
                }
            >
                {root_label}
            </div>
            {items.into_iter().map(|item| {
                let vm = category_card_view_model(
                    &item,
                    editing_id.as_deref(),
                    busy_key.as_deref(),
                    &category_labels,
                );
                let item_is_busy = vm.is_busy;
                let item_is_busy = vm.is_busy;
                let item_id = item.id.clone();
                let before_target = item.id.clone();
                let inside_target = item.id.clone();
                let drag_item_id = item.id.clone();
                let depth_style = format!("margin-left: {}rem", f32::from(item.depth) * 1.25);
                let before_drop = execute_drop;
                let inside_drop = execute_drop;
                view! {
                    <div style=depth_style class="space-y-2">
                        <div
                            class="h-2 rounded-full border border-dashed border-transparent transition hover:border-primary/50 hover:bg-primary/10"
                            title=before_label.clone()
                            on:dragover=move |event: DragEvent| event.prevent_default()
                            on:drop=move |event: DragEvent| {
                                event.prevent_default();
                                before_drop.run((Some(before_target.clone()), CategoryDropPlacement::Before));
                            }
                        ></div>
                        <article
                            class="relative overflow-hidden rounded-[1.5rem] border border-border bg-background p-5 shadow-sm transition hover:border-primary/30 hover:shadow-md"
                            attr:draggable="true"
                            on:dragstart=move |_event: DragEvent| {
                                set_move_error.set(None);
                                set_dragged_id.set(Some(drag_item_id.clone()));
                            }
                            on:dragend=move |_event: DragEvent| set_dragged_id.set(None)
                        >
                            <span class=format!("absolute inset-y-0 left-0 w-1.5 {}", rustok_ui_core::css_background_accent_class(vm.accent_style.as_str()))></span>
                            <div class="pl-3">
                                <div class="flex items-start justify-between gap-4">
                                    <div>
                                        <div class="flex flex-wrap items-center gap-2 text-[11px] font-semibold uppercase tracking-[0.22em] text-muted-foreground">
                                            <span>{vm.effective_locale.clone()}</span>
                                            <span>{format!("depth {} · position {}", item.depth, item.position)}</span>
                                            {item.is_archived.then(|| view! {
                                                <span class="rounded-full bg-destructive/10 px-2 py-0.5 text-destructive">{archived_label.clone()}</span>
                                            })}
                                        </div>
                                        <h3 class="mt-2 text-lg font-semibold text-foreground">{vm.name.clone()}</h3>
                                    </div>
                                    <span class="rounded-full border border-border px-3 py-1 text-xs font-medium text-muted-foreground">
                                        {vm.slug_badge.clone()}
                                    </span>
                                </div>
                                <p class="mt-3 text-sm leading-6 text-muted-foreground">
                                    {vm.description.clone()}
                                </p>
                                <div class="mt-4 flex flex-wrap gap-2">
                                    <span class="rounded-full bg-muted px-2.5 py-1 text-xs font-medium text-muted-foreground">{vm.topics_count_label.clone()}</span>
                                    <span class="rounded-full bg-muted px-2.5 py-1 text-xs font-medium text-muted-foreground">{vm.replies_count_label.clone()}</span>
                                    {vm.icon_label.clone().map(|label| view! {
                                        <span class="rounded-full bg-muted px-2.5 py-1 text-xs font-medium text-muted-foreground">{label}</span>
                                    })}
                                </div>
                                <div class="mt-5 flex flex-wrap items-center gap-2">
                                    <span class="cursor-grab rounded-full border border-dashed border-border px-3 py-2 text-xs font-medium text-muted-foreground active:cursor-grabbing">
                                        {drag_label.clone()}
                                    </span>
                                    <button
                                        type="button"
                                        class=forum_admin_action_button_class(ForumAdminActionButtonKind::Action)
                                        on:click={ let item_id = item_id.clone(); move |_| on_edit.run(item_id.clone()) }
                                        disabled=move || item_is_busy || move_busy.get()
                                    >
                                        {vm.action_label.clone()}
                                    </button>
                                    <button
                                        type="button"
                                        class=forum_admin_action_button_class(ForumAdminActionButtonKind::Delete)
                                        on:click={ let item_id = item_id.clone(); move |_| on_delete.run(item_id.clone()) }
                                        disabled=move || item_is_busy || move_busy.get()
                                    >
                                        {delete_label.clone()}
                                    </button>
                                </div>
                                <div
                                    class="mt-4 rounded-xl border border-dashed border-border bg-muted/30 px-3 py-2 text-center text-xs font-medium text-muted-foreground transition hover:border-primary/50 hover:bg-primary/10 hover:text-foreground"
                                    title=inside_label.clone()
                                    on:dragover=move |event: DragEvent| event.prevent_default()
                                    on:drop=move |event: DragEvent| {
                                        event.prevent_default();
                                        inside_drop.run((Some(inside_target.clone()), CategoryDropPlacement::Inside));
                                    }
                                >
                                    {inside_label.clone()}
                                </div>
                            </div>
                        </article>
                    </div>
                }
            }).collect_view()}
        </div>
    }
}
