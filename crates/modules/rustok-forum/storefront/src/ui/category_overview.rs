use std::collections::HashMap;

use leptos::prelude::*;
use rustok_api::normalize_locale_tag;
use rustok_ui_core::css_hex_accent_class;

use crate::model::ForumCategoryListItem;

fn forum_storefront_content_lang(locale: &str) -> String {
    normalize_locale_tag(locale).unwrap_or_else(|| "und".to_string())
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CategoryHierarchyNode {
    pub category: ForumCategoryListItem,
    pub subcategories: Vec<ForumCategoryListItem>,
    pub total_topic_count: i32,
    pub total_reply_count: i32,
}

#[component]
pub fn CategoryOverview(
    items: Vec<ForumCategoryListItem>,
    #[allow(unused_variables)] total: u64,
    on_select_category: Callback<String>,
) -> impl IntoView {
    let (search_query, set_search_query) = signal(String::new());

    // Build hierarchy
    let mut by_id = HashMap::with_capacity(items.len());
    for item in &items {
        by_id.insert(item.id.clone(), item.clone());
    }

    let mut roots = Vec::new();
    let mut children_map: HashMap<String, Vec<ForumCategoryListItem>> = HashMap::new();

    for item in &items {
        if let Some(parent_id) = &item.parent_id {
            if by_id.contains_key(parent_id) {
                children_map
                    .entry(parent_id.clone())
                    .or_default()
                    .push(item.clone());
                continue;
            }
        }
        roots.push(item.clone());
    }

    let hierarchy: Vec<CategoryHierarchyNode> = roots
        .into_iter()
        .map(|root| {
            let subcategories = children_map.remove(&root.id).unwrap_or_default();
            let total_topic_count = root.topic_count
                + subcategories
                    .iter()
                    .map(|sub| sub.topic_count)
                    .sum::<i32>();
            let total_reply_count = root.reply_count
                + subcategories
                    .iter()
                    .map(|sub| sub.reply_count)
                    .sum::<i32>();

            CategoryHierarchyNode {
                category: root,
                subcategories,
                total_topic_count,
                total_reply_count,
            }
        })
        .collect();

    let total_subcategories: usize = hierarchy.iter().map(|h| h.subcategories.len()).sum();
    let aggregate_topics: i32 = hierarchy.iter().map(|h| h.total_topic_count).sum();
    let root_count = hierarchy.len();

    let filtered_hierarchy = move || {
        let q = search_query.get().trim().to_lowercase();
        if q.is_empty() {
            return hierarchy.clone();
        }

        hierarchy
            .iter()
            .filter_map(|node| {
                let matches_root = node.category.name.to_lowercase().contains(&q)
                    || node.category.slug.to_lowercase().contains(&q)
                    || node
                        .category
                        .description
                        .as_deref()
                        .map(|d| d.to_lowercase().contains(&q))
                        .unwrap_or(false);

                let matching_subs: Vec<_> = node
                    .subcategories
                    .iter()
                    .filter(|sub| {
                        sub.name.to_lowercase().contains(&q)
                            || sub.slug.to_lowercase().contains(&q)
                            || sub
                                .description
                                .as_deref()
                                .map(|d| d.to_lowercase().contains(&q))
                                .unwrap_or(false)
                    })
                    .cloned()
                    .collect();

                if matches_root || !matching_subs.is_empty() {
                    Some(CategoryHierarchyNode {
                        category: node.category.clone(),
                        subcategories: if !matching_subs.is_empty() {
                            matching_subs
                        } else {
                            node.subcategories.clone()
                        },
                        total_topic_count: node.total_topic_count,
                        total_reply_count: node.total_reply_count,
                    })
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
    };

    view! {
        <div class="space-y-6">
            // Header stats banner and search input
            <div class="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between rounded-2xl border border-border bg-card p-4 shadow-sm">
                <div class="flex flex-wrap items-center gap-3 text-xs font-medium text-muted-foreground">
                    <span class="inline-flex items-center gap-1.5 rounded-full bg-primary/10 px-3 py-1 font-semibold text-primary">
                        <span class="h-2 w-2 rounded-full bg-primary"></span>
                        {format!("{root_count} Root Categories")}
                    </span>
                    {if total_subcategories > 0 {
                        Some(view! {
                            <span class="inline-flex items-center gap-1.5 rounded-full bg-muted px-3 py-1 text-muted-foreground">
                                {format!("{total_subcategories} Subcategories")}
                            </span>
                        })
                    } else {
                        None
                    }}
                    <span class="inline-flex items-center gap-1.5 rounded-full bg-muted px-3 py-1 text-muted-foreground">
                        {format!("{aggregate_topics} Total Topics")}
                    </span>
                </div>

                <div class="relative min-w-64 max-w-sm">
                    <input
                        type="search"
                        placeholder="Filter categories or subcategories…"
                        prop:value=move || search_query.get()
                        on:input=move |ev| set_search_query.set(event_target_value(&ev))
                        class="w-full rounded-xl border border-border bg-background px-3.5 py-2 text-sm outline-none transition placeholder:text-muted-foreground focus:border-primary focus:ring-1 focus:ring-primary"
                    />
                </div>
            </div>

            // Category cards grid
            {move || {
                let nodes = filtered_hierarchy();
                if nodes.is_empty() {
                    view! {
                        <div class="rounded-[1.75rem] border border-dashed border-border p-12 text-center">
                            <div class="mx-auto flex h-12 w-12 items-center justify-center rounded-2xl bg-muted text-muted-foreground">
                                "📁"
                            </div>
                            <h3 class="mt-4 text-base font-semibold text-foreground">
                                "No matching categories found"
                            </h3>
                            <p class="mt-1 text-sm text-muted-foreground">
                                "Try searching with a different term or clear the filter."
                            </p>
                        </div>
                    }.into_any()
                } else {
                    view! {
                        <div class="grid gap-6 md:grid-cols-2">
                            {nodes.into_iter().map(|node| {
                                let category = node.category;
                                let subcategories = node.subcategories;
                                let total_topics = node.total_topic_count;
                                let total_replies = node.total_reply_count;
                                let cat_id = category.id.clone();
                                let browse_id = category.id.clone();
                                let on_select_title = on_select_category;
                                let on_select_browse = on_select_category;
                                let accent_class = css_hex_accent_class(category.color.as_deref());
                                let icon_display = category.icon.clone().unwrap_or_else(|| {
                                    category.name.chars().take(2).collect::<String>().to_uppercase()
                                });
                                let content_lang = forum_storefront_content_lang(category.effective_locale.as_str());

                                view! {
                                    <div class="group relative flex flex-col justify-between overflow-hidden rounded-[1.75rem] border border-border bg-card p-6 shadow-sm transition hover:border-primary/40 hover:shadow-md">
                                        <span class=format!("absolute inset-y-0 left-0 w-1.5 transition-all group-hover:w-2 {accent_class}")></span>

                                        <div class="pl-3">
                                            <div class="flex items-start justify-between gap-4">
                                                <div class="flex items-start gap-3">
                                                    <div class="flex h-11 w-11 shrink-0 items-center justify-center rounded-2xl bg-primary/10 text-lg font-bold text-primary shadow-sm">
                                                        {icon_display}
                                                    </div>
                                                    <div>
                                                        <button
                                                            type="button"
                                                            on:click={
                                                                let cat_id = cat_id.clone();
                                                                move |_| on_select_title.run(cat_id.clone())
                                                            }
                                                            class="text-left text-lg font-semibold text-card-foreground transition hover:text-primary"
                                                        >
                                                            {category.name.clone()}
                                                        </button>
                                                        <div class="mt-0.5 flex items-center gap-2">
                                                            <span class="font-mono text-xs text-muted-foreground">
                                                                {format!("/{}", category.slug)}
                                                            </span>
                                                        </div>
                                                    </div>
                                                </div>

                                                <button
                                                    type="button"
                                                    on:click={
                                                        let browse_id = browse_id.clone();
                                                        move |_| on_select_browse.run(browse_id.clone())
                                                    }
                                                    class="shrink-0 rounded-full border border-border bg-background px-3 py-1.5 text-xs font-medium text-foreground transition hover:border-primary hover:bg-muted"
                                                >
                                                    "Browse →"
                                                </button>
                                            </div>

                                            <p class="mt-3 text-sm leading-6 text-muted-foreground line-clamp-2">
                                                {category.description.clone().unwrap_or_else(|| "No description provided for this section.".to_string())}
                                            </p>

                                            {if !subcategories.is_empty() {
                                                Some(view! {
                                                    <div class="mt-5 space-y-2 border-t border-border/60 pt-4">
                                                        <p class="text-[11px] font-semibold uppercase tracking-[0.2em] text-muted-foreground">
                                                            "Subcategories"
                                                        </p>
                                                        <div class="flex flex-wrap gap-2">
                                                            {subcategories.into_iter().map(|sub| {
                                                                let sub_id = sub.id.clone();
                                                                let sub_accent = css_hex_accent_class(sub.color.as_deref());
                                                                let on_select_sub = on_select_category;

                                                                view! {
                                                                    <button
                                                                        type="button"
                                                                        on:click={
                                                                            let sub_id = sub_id.clone();
                                                                            move |_| on_select_sub.run(sub_id.clone())
                                                                        }
                                                                        class="group/sub inline-flex items-center gap-1.5 rounded-full border border-border/80 bg-background/80 px-3 py-1 text-xs font-medium text-foreground transition hover:border-primary/50 hover:bg-muted hover:shadow-sm"
                                                                    >
                                                                        <span class=format!("h-2 w-2 rounded-full transition-transform group-hover/sub:scale-125 {sub_accent}")></span>
                                                                        <span>{sub.name}</span>
                                                                        <span class="text-[10px] text-muted-foreground">
                                                                            {sub.topic_count}
                                                                        </span>
                                                                    </button>
                                                                }
                                                            }).collect_view()}
                                                        </div>
                                                    </div>
                                                })
                                            } else {
                                                None
                                            }}
                                        </div>

                                        <div class="mt-6 flex items-center justify-between border-t border-border/40 pl-3 pt-4 text-xs font-medium text-muted-foreground">
                                            <div class="flex items-center gap-4">
                                                <span class="flex items-center gap-1">
                                                    <span class="font-semibold text-foreground">{total_topics}</span>
                                                    " topics"
                                                </span>
                                                <span class="flex items-center gap-1">
                                                    <span class="font-semibold text-foreground">{total_replies}</span>
                                                    " replies"
                                                </span>
                                            </div>
                                            <span class="text-[11px] text-muted-foreground">
                                                {format!("lang: {content_lang}")}
                                            </span>
                                        </div>
                                    </div>
                                }
                            }).collect_view()}
                        </div>
                    }.into_any()
                }
            }}
        </div>
    }
}
