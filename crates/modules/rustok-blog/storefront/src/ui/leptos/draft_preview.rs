use super::*;
use leptos::prelude::*;
use rustok_ui_leptos::RichTextHtml;
use rustok_ui_leptos::{Button, Input};
use rustok_ui_core::UiRouteContext;

#[cfg(target_arch = "wasm32")]
use crate::core::parse_preview_token_from_fragment;
use crate::core::{calculate_reading_time, format_reading_time};
use crate::model::BlogPostDetail;
use crate::transport::fetch_post_by_preview_token;

/// Draft preview component for viewing unpublished posts via a capability bearer token.
///
/// Matches the Next.js `BlogDraftPreview` surface. Automatically parses `#token=вЂ¦` from
/// the browser URL fragment on load, or allows pasting the raw preview token into a form.
#[component]
pub fn BlogDraftPreview() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let locale = route_context.locale;
    let copy = core::build_draft_preview_copy(locale.as_deref());

    let token_input = RwSignal::new(String::new());
    let post = RwSignal::new(Option::<BlogPostDetail>::None);
    let loading = RwSignal::new(false);
    let not_found = RwSignal::new(false);
    let failure = RwSignal::new(Option::<String>::None);

    let locale_stored = StoredValue::new(locale.clone());
    let load_preview = Callback::new(move |token: String| {
        let trimmed = token.trim().to_string();
        if trimmed.is_empty() || loading.get_untracked() {
            return;
        }
        loading.set(true);
        not_found.set(false);
        failure.set(None);
        let locale_str = locale_stored.get_value();
        leptos::task::spawn_local(async move {
            match fetch_post_by_preview_token(trimmed, locale_str).await {
                Ok(Some(loaded_post)) => {
                    post.set(Some(loaded_post));
                    not_found.set(false);
                    failure.set(None);
                }
                Ok(None) => {
                    post.set(None);
                    not_found.set(true);
                    failure.set(None);
                }
                Err(err) => {
                    post.set(None);
                    not_found.set(false);
                    failure.set(Some(err.to_string()));
                }
            }
            loading.set(false);
        });
    });

    // Client-side initialization: read URL fragment and listen for hash changes
    Effect::new(move |_| {
        #[cfg(target_arch = "wasm32")]
        {
            let check_hash = move || {
                if let Some(window) = web_sys::window() {
                    if let Ok(location) = window.location().hash() {
                        if let Some(token) = parse_preview_token_from_fragment(&location) {
                            token_input.set(token.clone());
                            load_preview.run(token);
                        }
                    }
                }
            };
            check_hash();
            let _ = window_event_listener(leptos::ev::hashchange, move |_| check_hash());
        }
    });

    view! {
        <div class="mx-auto max-w-6xl px-4 sm:px-6 py-10 space-y-8">
            {move || {
                if let Some(p) = post.get() {
                    let reading_minutes = p.content_plain_text.as_deref().map(calculate_reading_time).unwrap_or(1);
                    let reading_time_label = format_reading_time(reading_minutes, locale.as_deref());
                    let has_tags = !p.tags.is_empty();
                    let tags = p.tags.clone();
                    let excerpt = p.excerpt.clone();
                    let featured_image = p.featured_image_url.clone();
                    let effective_locale = p.effective_locale.clone();
                    let content = p.content.clone();
                    let author_name = p.author_profile.as_ref().map(|a| a.display_name.clone());
                    let category_name = p.category_name.clone();

                    view! {
                        <article class="space-y-8">
                            // Amber Draft Preview Banner
                            <div class="rounded-lg border border-amber-500/40 bg-amber-500/10 px-4 py-3 text-sm text-amber-700 dark:text-amber-400">
                                <span class="inline-flex items-center gap-2 font-medium">
                                    <span class="inline-block h-2 w-2 rounded-full bg-amber-500"></span>
                                    {copy.badge.clone()}
                                </span>
                                <p class="mt-1 text-muted-foreground">
                                    {copy.banner_body.clone()}
                                </p>
                            </div>

                            <header class="space-y-4 border-b border-border pb-6">
                                <div class="flex flex-wrap items-center gap-4 text-xs text-muted-foreground">
                                    {category_name.map(|name| view! {
                                        <span class="rounded-full bg-secondary px-2.5 py-0.5 font-medium text-secondary-foreground text-xs">
                                            {name}
                                        </span>
                                    })}
                                    {author_name.map(|name| view! {
                                        <span class="font-medium text-foreground">
                                            {name}
                                        </span>
                                    })}
                                    <span class="inline-flex items-center gap-1.5">
                                        <span>{reading_time_label}</span>
                                    </span>
                                    {if has_tags {
                                        view! {
                                            <div class="flex flex-wrap items-center gap-1.5">
                                                {tags.into_iter().map(|tag| view! {
                                                    <span class="rounded-full bg-primary/10 px-2.5 py-0.5 font-medium text-primary text-[11px]">
                                                        {format!("#{tag}")}
                                                    </span>
                                                }).collect_view()}
                                            </div>
                                        }.into_any()
                                    } else {
                                        ().into_any()
                                    }}
                                </div>

                                <h1 class="text-3xl font-extrabold tracking-tight text-foreground sm:text-4xl lg:text-5xl leading-tight">
                                    {p.title}
                                </h1>

                                {excerpt.map(|text| view! {
                                    <p class="text-lg text-muted-foreground leading-relaxed">
                                        {text}
                                    </p>
                                })}
                            </header>

                            <div class="space-y-8">
                                {featured_image.map(|url| view! {
                                    <div class="relative aspect-video w-full overflow-hidden rounded-2xl border border-border shadow-sm">
                                        <img src=url alt="Featured" class="h-full w-full object-cover" />
                                    </div>
                                })}

                                <div class="prose prose-neutral dark:prose-invert max-w-none pt-2">
                                    {match content {
                                        Some(view) => view! {
                                            <RichTextHtml
                                                view=view
                                                content_locale=effective_locale
                                                class="text-base leading-8 text-foreground/90 space-y-4"
                                            />
                                        }.into_any(),
                                        None => ().into_any(),
                                    }}
                                </div>
                            </div>
                        </article>
                    }.into_any()
                } else {
                    let copy = copy.clone();
                    view! {
                        <div class="mx-auto max-w-xl px-4 sm:px-6 py-16 space-y-6">
                            <h1 class="text-2xl font-bold tracking-tight text-foreground">
                                {copy.title.clone()}
                            </h1>
                            <p class="text-sm text-muted-foreground">
                                {copy.subtitle.clone()}
                            </p>

                            <form
                                class="flex items-center gap-2"
                                on:submit=move |ev: leptos::ev::SubmitEvent| {
                                    ev.prevent_default();
                                    let token = token_input.get();
                                    load_preview.run(token);
                                }
                            >
                                <Input placeholder=copy.token_placeholder.clone()
                                    value=token_input.read_only() set_value=token_input.write_only()
                                    disabled=Signal::derive(move || loading.get()) />
                                <Button
                                    r#type="submit"
                                    class="inline-flex h-10 items-center justify-center whitespace-nowrap rounded-md bg-primary px-4 text-sm font-medium text-primary-foreground transition-colors disabled:pointer-events-none disabled:opacity-50"
                                    disabled=Signal::derive(move || loading.get())
                                >
                                    {move || if loading.get() { copy.button_loading.clone() } else { copy.button_open.clone() }}
                                </Button>
                            </form>

                            {move || if not_found.get() {
                                view! {
                                    <p class="text-sm text-destructive">
                                        {copy.error_not_found.clone()}
                                    </p>
                                }.into_any()
                            } else if let Some(err) = failure.get() {
                                view! {
                                    <p class="text-sm text-destructive">
                                        {format!("{}: {err}", copy.error_load)}
                                    </p>
                                }.into_any()
                            } else {
                                ().into_any()
                            }}
                        </div>
                    }.into_any()
                }
            }}
        </div>
    }
}
