use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::core::{
    ProductRelationsTransportProfile, build_product_relations_panel_copy, selected_transport_profile,
    validate_target_product_id,
};
use crate::model::{
    CreateProductRelationDraft, ProductRelationItem, ProductRelationsAdminCommand,
    ProductRelationsAdminFilters,
};
use crate::transport::{
    ProductRelationsTransportContext, execute_product_relations_command, load_product_relations,
};

fn local_resource<S, Fut, T>(
    source: impl Fn() -> S + 'static,
    fetcher: impl Fn(S) -> Fut + 'static,
) -> LocalResource<T>
where
    S: 'static,
    Fut: std::future::Future<Output = T> + 'static,
    T: 'static,
{
    LocalResource::new(move || fetcher(source()))
}

#[component]
pub fn ProductRelationsPanel(
    #[prop(optional)] locale: Option<String>,
    product_id: String,
    #[prop(optional)] token: Option<Signal<Option<String>>>,
    #[prop(optional)] tenant: Option<Signal<Option<String>>>,
    busy: ReadSignal<bool>,
    set_busy: WriteSignal<bool>,
    set_error: WriteSignal<Option<String>>,
) -> impl IntoView {
    let copy = std::sync::Arc::new(build_product_relations_panel_copy(locale.as_deref()));
    let (selected_type, set_selected_type) = signal("cross_sell".to_string());
    let (show_add, set_show_add) = signal(false);
    let (target_product_id, set_target_product_id) = signal(String::new());
    let (position, set_position) = signal(0_i32);
    let (reload_seq, set_reload_seq) = signal(0_usize);

    let profile = selected_transport_profile(option_env!("RUSTOK_UI_TRANSPORT_PROFILE"));
    let token_val = token.map(|t| t.get()).unwrap_or_default();
    let tenant_val = tenant.map(|t| t.get()).unwrap_or_default();
    let transport = match profile {
        ProductRelationsTransportProfile::Native => ProductRelationsTransportContext::native(),
        ProductRelationsTransportProfile::Graphql => {
            ProductRelationsTransportContext::graphql(token_val, tenant_val)
        }
    };

    let p_id = product_id.clone();
    let transport_for_loader = transport.clone();
    let relations_resource = local_resource(
        move || (selected_type.get(), reload_seq.get()),
        move |(rel_type, _)| {
            let context = transport_for_loader.clone();
            let prod_id = p_id.clone();
            async move {
                load_product_relations(
                    context,
                    ProductRelationsAdminFilters {
                        product_id: Some(prod_id),
                        relation_type: Some(rel_type),
                    },
                )
                .await
            }
        },
    );

    let on_add_submit = {
        let transport = transport.clone();
        let prod_id = product_id.clone();
        move |ev: leptos::ev::SubmitEvent| {
            ev.prevent_default();
            let target_id_val = target_product_id.get_untracked().trim().to_string();

            if let Err(e) = validate_target_product_id(&target_id_val) {
                set_error.set(Some(e.to_string()));
                return;
            }
            if target_id_val == prod_id {
                set_error.set(Some("A product cannot be related to itself.".to_string()));
                return;
            }

            let draft = CreateProductRelationDraft {
                product_id: prod_id.clone(),
                related_product_id: target_id_val,
                relation_type: selected_type.get_untracked(),
                position: Some(position.get_untracked()),
                metadata: None,
            };

            set_busy.set(true);
            set_error.set(None);
            let transport = transport.clone();
            spawn_local(async move {
                match execute_product_relations_command(
                    transport,
                    uuid::Uuid::new_v4().to_string(),
                    ProductRelationsAdminCommand::Add { draft },
                )
                .await
                {
                    Ok(res) if res.is_success() => {
                        set_target_product_id.set(String::new());
                        set_position.set(0);
                        set_show_add.set(false);
                        set_reload_seq.update(|v| *v += 1);
                    }
                    Ok(_) => set_error.set(Some("Failed to add relation.".to_string())),
                    Err(err) => set_error.set(Some(err.to_string())),
                }
                set_busy.set(false);
            });
        }
    };

    let on_remove = {
        let transport = transport.clone();
        move |rel_id: String| {
            set_busy.set(true);
            set_error.set(None);
            let transport = transport.clone();
            spawn_local(async move {
                match execute_product_relations_command(
                    transport,
                    uuid::Uuid::new_v4().to_string(),
                    ProductRelationsAdminCommand::Remove { id: rel_id },
                )
                .await
                {
                    Ok(res) if res.is_success() => set_reload_seq.update(|v| *v += 1),
                    Ok(_) => set_error.set(Some("Failed to remove relation.".to_string())),
                    Err(err) => set_error.set(Some(err.to_string())),
                }
                set_busy.set(false);
            });
        }
    };

    let on_move = {
        let transport = transport.clone();
        let prod_id = product_id.clone();
        move |items: Vec<ProductRelationItem>, index: usize, delta: isize| {
            let new_index = index as isize + delta;
            if new_index < 0 || new_index >= items.len() as isize {
                return;
            }
            let new_index = new_index as usize;
            let mut ordered = items;
            ordered.swap(index, new_index);
            let ordered_ids: Vec<String> = ordered.iter().map(|item| item.id.clone()).collect();
            let rel_type = selected_type.get_untracked();
            let prod_id = prod_id.clone();

            set_busy.set(true);
            set_error.set(None);
            let transport = transport.clone();
            spawn_local(async move {
                match execute_product_relations_command(
                    transport,
                    uuid::Uuid::new_v4().to_string(),
                    ProductRelationsAdminCommand::Reorder {
                        product_id: prod_id,
                        relation_type: rel_type,
                        ordered_ids,
                    },
                )
                .await
                {
                    Ok(res) if res.is_success() => set_reload_seq.update(|v| *v += 1),
                    Ok(_) => set_error.set(Some("Failed to reorder relations.".to_string())),
                    Err(err) => set_error.set(Some(err.to_string())),
                }
                set_busy.set(false);
            });
        }
    };

    view! {
        <section class="rounded-3xl border border-border bg-card p-6 shadow-sm dark:border-gray-800 dark:bg-gray-900">
            <div class="flex items-center justify-between gap-3">
                <div>
                    <h3 class="text-lg font-semibold text-card-foreground dark:text-white">{copy.title.clone()}</h3>
                    <p class="text-sm text-muted-foreground dark:text-gray-400">{copy.subtitle.clone()}</p>
                </div>
                <button
                    type="button"
                    class="inline-flex rounded-lg border border-border px-3 py-2 text-sm font-medium text-foreground transition hover:bg-accent disabled:opacity-50 dark:border-gray-700 dark:text-gray-200"
                    disabled=move || busy.get()
                    on:click=move |_| set_show_add.update(|v| *v = !*v)
                >
                    {copy.add.clone()}
                </button>
            </div>

            <div class="mt-4 flex flex-wrap gap-2 border-b border-border pb-3 dark:border-gray-800">
                {
                    let types = [
                        ("cross_sell", copy.tab_cross_sell.clone()),
                        ("up_sell", copy.tab_up_sell.clone()),
                        ("related", copy.tab_related.clone()),
                        ("accessory", copy.tab_accessory.clone()),
                        ("alternative", copy.tab_alternative.clone()),
                    ];
                    types.into_iter().map(|(rel_key, label)| {
                        let is_active = move || selected_type.get() == rel_key;
                        let rel_key_str = rel_key.to_string();
                        view! {
                            <button
                                type="button"
                                class=move || {
                                    if is_active() {
                                        "rounded-lg bg-blue-600 px-3 py-1.5 text-xs font-medium text-white shadow-sm transition"
                                    } else {
                                        "rounded-lg border border-border px-3 py-1.5 text-xs font-medium text-muted-foreground transition hover:bg-gray-100 dark:hover:bg-gray-800 hover:text-foreground dark:border-gray-700"
                                    }
                                }
                                on:click=move |_| {
                                    set_selected_type.set(rel_key_str.clone());
                                }
                            >
                                {label}
                            </button>
                        }
                    }).collect_view()
                }
            </div>

            {
                let copy_for_form = copy.clone();
                view! {
                    <Show when=move || show_add.get()>
                        <form class="mt-4 rounded-2xl border border-border/70 bg-background/50 p-4 space-y-3 dark:border-gray-700 dark:bg-gray-800/40" on:submit={
                            let on_submit = on_add_submit.clone();
                            move |ev| on_submit(ev)
                        }>
                            <div class="grid gap-3 md:grid-cols-2">
                                <div>
                                    <label class="block text-xs font-medium text-muted-foreground mb-1 dark:text-gray-300">{copy_for_form.target_product_id.clone()}</label>
                                    <input
                                        class="w-full rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-blue-500 font-mono text-xs dark:bg-gray-800 dark:border-gray-700 dark:text-white"
                                        placeholder="Product UUID"
                                        prop:value=move || target_product_id.get()
                                        on:input=move |ev| set_target_product_id.set(event_target_value(&ev))
                                    />
                                </div>
                                <div>
                                    <label class="block text-xs font-medium text-muted-foreground mb-1 dark:text-gray-300">{copy_for_form.position.clone()}</label>
                                    <input
                                        type="number"
                                        class="w-full rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-blue-500 text-xs dark:bg-gray-800 dark:border-gray-700 dark:text-white"
                                        prop:value=move || position.get().to_string()
                                        on:input=move |ev| {
                                            if let Ok(val) = event_target_value(&ev).parse::<i32>() {
                                                set_position.set(val);
                                            }
                                        }
                                    />
                                </div>
                            </div>
                            <div class="flex justify-end gap-2">
                                <button
                                    type="button"
                                    class="rounded-lg px-3 py-1.5 text-xs text-muted-foreground hover:bg-accent transition dark:text-gray-400"
                                    on:click=move |_| set_show_add.set(false)
                                >
                                    "Cancel"
                                </button>
                                <button
                                    type="submit"
                                    class="rounded-lg bg-blue-600 px-3 py-1.5 text-xs font-medium text-white hover:bg-blue-700 transition disabled:opacity-50"
                                    disabled=move || busy.get()
                                >
                                    {copy_for_form.add.clone()}
                                </button>
                            </div>
                        </form>
                    </Show>
                }
            }

            <div class="mt-4">
                {
                    let copy_for_list = copy.clone();
                    move || {
                        let copy = copy_for_list.clone();
                        let relations = relations_resource.get().and_then(Result::ok).unwrap_or_default();
                        if relations.is_empty() {
                            view! {
                                <div class="rounded-2xl border border-dashed border-border/70 p-6 text-center text-sm text-muted-foreground dark:border-gray-800 dark:text-gray-400">
                                    {copy.empty.clone()}
                                </div>
                            }.into_any()
                        } else {
                            let total = relations.len();
                            let all_items = relations.clone();
                            let on_move = on_move.clone();
                            let on_remove = on_remove.clone();
                            let copy_move_up = copy.move_up.clone();
                            let copy_move_down = copy.move_down.clone();
                            let copy_remove = copy.remove.clone();
                            let copy_pos = copy.position.clone();
                            let copy_target = copy.target_product_id.clone();
                            view! {
                                <div class="overflow-hidden rounded-2xl border border-border dark:border-gray-800">
                                    <table class="w-full text-left text-sm">
                                        <thead class="bg-gray-50 dark:bg-gray-800/50 text-xs text-muted-foreground dark:text-gray-400">
                                            <tr>
                                                <th class="px-4 py-3 font-medium">{copy_pos}</th>
                                                <th class="px-4 py-3 font-medium">{copy_target}</th>
                                                <th class="px-4 py-3 text-right font-medium">"Actions"</th>
                                            </tr>
                                        </thead>
                                        <tbody class="divide-y divide-border dark:divide-gray-800">
                                            {relations.into_iter().enumerate().map(|(idx, item)| {
                                                let item_id = item.id.clone();
                                                let is_first = idx == 0;
                                                let is_last = idx + 1 >= total;
                                                let on_move_up = {
                                                    let on_move = on_move.clone();
                                                    let all_items = all_items.clone();
                                                    move |_| on_move(all_items.clone(), idx, -1)
                                                };
                                                let on_move_down = {
                                                    let on_move = on_move.clone();
                                                    let all_items = all_items.clone();
                                                    move |_| on_move(all_items.clone(), idx, 1)
                                                };
                                                let on_del = {
                                                    let on_remove = on_remove.clone();
                                                    let item_id = item_id.clone();
                                                    move |_| on_remove(item_id.clone())
                                                };
                                                view! {
                                                    <tr class="hover:bg-gray-50/50 dark:hover:bg-gray-800/40 transition">
                                                        <td class="px-4 py-3 font-mono text-xs text-muted-foreground dark:text-gray-400">{item.position}</td>
                                                        <td class="px-4 py-3 font-mono text-xs text-foreground dark:text-gray-200">{item.related_product_id}</td>
                                                        <td class="px-4 py-3 text-right">
                                                            <div class="flex items-center justify-end gap-1">
                                                                <button
                                                                    type="button"
                                                                    class="rounded p-1 text-xs text-muted-foreground hover:bg-gray-100 dark:hover:bg-gray-800 hover:text-foreground disabled:opacity-30"
                                                                    disabled=move || is_first || busy.get()
                                                                    title=copy_move_up.clone()
                                                                    on:click=on_move_up
                                                                >
                                                                    "↑"
                                                                </button>
                                                                <button
                                                                    type="button"
                                                                    class="rounded p-1 text-xs text-muted-foreground hover:bg-gray-100 dark:hover:bg-gray-800 hover:text-foreground disabled:opacity-30"
                                                                    disabled=move || is_last || busy.get()
                                                                    title=copy_move_down.clone()
                                                                    on:click=on_move_down
                                                                >
                                                                    "↓"
                                                                </button>
                                                                <button
                                                                    type="button"
                                                                    class="rounded px-2 py-0.5 text-xs text-rose-600 hover:bg-rose-50 border border-rose-200 dark:border-rose-900/40 transition disabled:opacity-50 ml-2"
                                                                    disabled=move || busy.get()
                                                                    on:click=on_del
                                                                >
                                                                    {copy_remove.clone()}
                                                                </button>
                                                            </div>
                                                        </td>
                                                    </tr>
                                                }
                                            }).collect_view()}
                                        </tbody>
                                    </table>
                                </div>
                            }.into_any()
                        }
                    }
                }
            </div>
        </section>
    }.into_any()
}

#[component]
pub fn ProductRelationsAdmin() -> impl IntoView {
    let (product_id, set_product_id) = signal(String::new());
    let (busy, set_busy) = signal(false);
    let (error, set_error) = signal(Option::<String>::None);

    view! {
        <div class="product-relations-admin p-6 max-w-7xl mx-auto space-y-6">
            <div class="border-b border-gray-200 pb-5 dark:border-gray-800">
                <h1 class="text-2xl font-bold tracking-tight text-gray-900 dark:text-white">
                    "Product Relations"
                </h1>
                <p class="text-sm text-gray-500 dark:text-gray-400 mt-1">
                    "Manage cross-sells, up-sells, accessories, and product merchandising associations."
                </p>
            </div>

            {move || error.get().map(|err| view! {
                <div class="rounded-xl border border-red-200 bg-red-50 p-4 text-sm text-red-700 dark:border-red-900/50 dark:bg-red-950/50 dark:text-red-300">
                    {err}
                </div>
            })}

            <div class="bg-white dark:bg-gray-900 p-4 rounded-xl border border-gray-200 dark:border-gray-800">
                <label class="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-1">
                    "Product ID (UUID)"
                </label>
                <input
                    type="text"
                    class="w-full max-w-md px-3 py-2 border rounded-lg font-mono text-sm dark:bg-gray-800 dark:border-gray-700 dark:text-white"
                    placeholder="Enter Product UUID to manage relations"
                    prop:value=move || product_id.get()
                    on:input=move |ev| set_product_id.set(event_target_value(&ev))
                />
            </div>

            {move || {
                let pid = product_id.get();
                if pid.trim().is_empty() {
                    view! {
                        <div class="p-8 text-center text-gray-500 dark:text-gray-400 text-sm">
                            "Please specify a Product UUID above to manage its relations."
                        </div>
                    }.into_any()
                } else {
                    view! {
                        <ProductRelationsPanel
                            product_id=pid
                            busy=busy
                            set_busy=set_busy
                            set_error=set_error
                        />
                    }.into_any()
                }
            }}
        </div>
    }
}
