use crate::i18n::t;
use crate::ui::leptos::{Card, TextField, direct_transport_summary, session_transport_summary};
use leptos::ev::SubmitEvent;
use leptos::prelude::*;

#[component]
pub fn AiJobsPanel(
    ui_locale: Option<String>,

    // Blog Draft
    blog_title: RwSignal<String>,
    blog_locale: RwSignal<String>,
    blog_post_id: RwSignal<String>,
    blog_source_locale: RwSignal<String>,
    blog_source_title: RwSignal<String>,
    blog_source_body: RwSignal<String>,
    blog_source_excerpt: RwSignal<String>,
    blog_source_seo_title: RwSignal<String>,
    blog_source_seo_description: RwSignal<String>,
    blog_tags: RwSignal<String>,
    blog_category_id: RwSignal<String>,
    blog_featured_image_url: RwSignal<String>,
    blog_copy_instructions: RwSignal<String>,
    blog_assistant_prompt: RwSignal<String>,
    on_run_blog_job: Callback<SubmitEvent>,

    // Product Copy
    product_title: RwSignal<String>,
    product_locale: RwSignal<String>,
    product_id: RwSignal<String>,
    product_source_locale: RwSignal<String>,
    product_source_title: RwSignal<String>,
    product_source_description: RwSignal<String>,
    product_source_meta_title: RwSignal<String>,
    product_source_meta_description: RwSignal<String>,
    product_copy_instructions: RwSignal<String>,
    product_assistant_prompt: RwSignal<String>,
    on_run_product_job: Callback<SubmitEvent>,

    // Product Attributes
    product_attributes_title: RwSignal<String>,
    product_attributes_locale: RwSignal<String>,
    product_attributes_product_id: RwSignal<String>,
    product_attributes_category_slug: RwSignal<String>,
    product_attributes_source_locale: RwSignal<String>,
    product_attributes_source_title: RwSignal<String>,
    product_attributes_source_description: RwSignal<String>,
    product_attributes_image_urls: RwSignal<String>,
    product_attributes_copy_instructions: RwSignal<String>,
    product_attributes_assistant_prompt: RwSignal<String>,
    on_run_product_attributes_job: Callback<SubmitEvent>,
    can_submit_product_attributes: Signal<bool>,

    // Order Analytics
    order_analytics_title: RwSignal<String>,
    order_analytics_locale: RwSignal<String>,
    order_analytics_order_ids: RwSignal<String>,
    order_analytics_date_from: RwSignal<String>,
    order_analytics_date_to: RwSignal<String>,
    order_analytics_focus: RwSignal<String>,
    order_analytics_assistant_prompt: RwSignal<String>,
    on_run_order_analytics_job: Callback<SubmitEvent>,
    can_submit_order_analytics: Signal<bool>,

    // Order Operations Assistant
    order_ops_title: RwSignal<String>,
    order_ops_locale: RwSignal<String>,
    order_ops_order_id: RwSignal<String>,
    order_ops_recommended_action: RwSignal<String>,
    order_ops_context: RwSignal<String>,
    order_ops_assistant_prompt: RwSignal<String>,
    on_run_order_ops_job: Callback<SubmitEvent>,
    can_submit_order_ops: Signal<bool>,

    // Media Image
    image_title: RwSignal<String>,
    image_locale: RwSignal<String>,
    image_prompt: RwSignal<String>,
    image_negative_prompt: RwSignal<String>,
    image_file_name: RwSignal<String>,
    image_asset_title: RwSignal<String>,
    image_alt_text: RwSignal<String>,
    image_caption: RwSignal<String>,
    image_size: RwSignal<String>,
    image_assistant_prompt: RwSignal<String>,
    on_run_image_job: Callback<SubmitEvent>,

    // Alloy Assist
    alloy_title: RwSignal<String>,
    alloy_locale: RwSignal<String>,
    alloy_operation: RwSignal<String>,
    alloy_script_id: RwSignal<String>,
    alloy_script_name: RwSignal<String>,
    alloy_script_source: RwSignal<String>,
    alloy_runtime_payload: RwSignal<String>,
    alloy_prompt: RwSignal<String>,
    on_run_alloy_job: Callback<SubmitEvent>,

    // New Session
    session_title: RwSignal<String>,
    session_locale: RwSignal<String>,
    session_message: RwSignal<String>,
    selected_provider: RwSignal<String>,
    selected_task_profile: RwSignal<String>,
    selected_tool_profile: RwSignal<String>,
    on_start_session: Callback<SubmitEvent>,
) -> impl IntoView {
    let ui_locale_blog = ui_locale.clone();
    let ui_locale_product = ui_locale.clone();
    let ui_locale_product_attributes = ui_locale.clone();
    let ui_locale_product_attributes_hint = ui_locale.clone();
    let ui_locale_order_analytics = ui_locale.clone();
    let ui_locale_order_analytics_hint = ui_locale.clone();
    let ui_locale_order_ops = ui_locale.clone();
    let ui_locale_order_ops_hint = ui_locale.clone();
    let ui_locale_image = ui_locale.clone();
    let ui_locale_alloy = ui_locale.clone();
    let ui_locale_new_session = ui_locale.clone();

    let blog_transport_locale = ui_locale.clone();
    let product_transport_locale = ui_locale.clone();
    let product_attributes_transport_locale = ui_locale.clone();
    let order_analytics_transport_locale = ui_locale.clone();
    let order_ops_transport_locale = ui_locale.clone();
    let image_transport_locale = ui_locale.clone();
    let alloy_transport_locale = ui_locale.clone();
    let session_transport_locale = ui_locale.clone();

    view! {
        <Card title=t(ui_locale_blog.as_deref(), "ai.card.blogDraft", "Blog Draft")>
                                    <form class="space-y-3" on:submit=move |ev| on_run_blog_job.run(ev)>
                                        <TextField label=t(ui_locale_blog.as_deref(), "ai.field.jobTitle", "Job title") value=blog_title />
                                        <TextField
                                            label=t(ui_locale_blog.as_deref(), "ai.field.locale", "Locale")
                                            value=blog_locale
                                            placeholder=t(ui_locale_blog.as_deref(), "ai.field.localeAutoPlaceholder", "auto (request locale -> tenant default -> en)")
                                        />
                                        <TextField label=t(ui_locale_blog.as_deref(), "ai.field.existingPostId", "Existing post id") value=blog_post_id />
                                        <TextField label=t(ui_locale_blog.as_deref(), "ai.field.sourceLocale", "Source locale") value=blog_source_locale />
                                        <TextField label=t(ui_locale_blog.as_deref(), "ai.field.sourceTitleOverride", "Source title override") value=blog_source_title />
                                        <label class="block space-y-1">
                                            <span class="text-sm text-muted-foreground">{t(ui_locale_blog.as_deref(), "ai.field.sourceBodyOverride", "Source body override")}</span>
                                            <textarea
                                                class="min-h-28 w-full rounded-lg border border-input bg-background px-3 py-2 text-sm"
                                                prop:value=blog_source_body
                                                on:input=move |ev| blog_source_body.set(event_target_value(&ev))
                                            />
                                        </label>
                                        <TextField label=t(ui_locale_blog.as_deref(), "ai.field.sourceExcerptOverride", "Source excerpt override") value=blog_source_excerpt />
                                        <TextField label=t(ui_locale_blog.as_deref(), "ai.field.sourceSeoTitleOverride", "Source SEO title override") value=blog_source_seo_title />
                                        <label class="block space-y-1">
                                            <span class="text-sm text-muted-foreground">{t(ui_locale_blog.as_deref(), "ai.field.sourceSeoDescriptionOverride", "Source SEO description override")}</span>
                                            <textarea
                                                class="min-h-20 w-full rounded-lg border border-input bg-background px-3 py-2 text-sm"
                                                prop:value=blog_source_seo_description
                                                on:input=move |ev| blog_source_seo_description.set(event_target_value(&ev))
                                            />
                                        </label>
                                        <TextField label=t(ui_locale_blog.as_deref(), "ai.field.tagsCsv", "Tags (csv)") value=blog_tags />
                                        <TextField label=t(ui_locale_blog.as_deref(), "ai.field.categoryId", "Category id") value=blog_category_id />
                                        <TextField label=t(ui_locale_blog.as_deref(), "ai.field.featuredImageUrl", "Featured image URL") value=blog_featured_image_url />
                                        <label class="block space-y-1">
                                            <span class="text-sm text-muted-foreground">{t(ui_locale_blog.as_deref(), "ai.field.copyInstructions", "Copy instructions")}</span>
                                            <textarea
                                                class="min-h-20 w-full rounded-lg border border-input bg-background px-3 py-2 text-sm"
                                                prop:value=blog_copy_instructions
                                                on:input=move |ev| blog_copy_instructions.set(event_target_value(&ev))
                                            />
                                        </label>
                                        <TextField label=t(ui_locale_blog.as_deref(), "ai.field.assistantPrompt", "Assistant prompt") value=blog_assistant_prompt />
                                        <div class="rounded-lg border border-border px-3 py-2 text-sm text-muted-foreground">
                                            {move || direct_transport_summary(
                                                blog_transport_locale.as_deref(),
                                                selected_provider.get().as_str(),
                                                selected_task_profile.get().as_str(),
                                            )}
                                        </div>
                                        <button type="submit" class="rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground">{t(ui_locale_blog.as_deref(), "ai.action.generateBlogDraft", "Generate blog draft")}</button>
                                    </form>
                                </Card>

                                <Card title=t(ui_locale_product.as_deref(), "ai.card.productCopy", "Product Copy")>
                                    <form class="space-y-3" on:submit=move |ev| on_run_product_job.run(ev)>
                                        <TextField label=t(ui_locale_product.as_deref(), "ai.field.jobTitle", "Job title") value=product_title />
                                        <TextField
                                            label=t(ui_locale_product.as_deref(), "ai.field.locale", "Locale")
                                            value=product_locale
                                            placeholder=t(ui_locale_product.as_deref(), "ai.field.localeAutoPlaceholder", "auto (request locale -> tenant default -> en)")
                                        />
                                        <TextField label=t(ui_locale_product.as_deref(), "ai.field.productId", "Product id") value=product_id />
                                        <TextField label=t(ui_locale_product.as_deref(), "ai.field.sourceLocale", "Source locale") value=product_source_locale />
                                        <TextField label=t(ui_locale_product.as_deref(), "ai.field.sourceTitleOverride", "Source title override") value=product_source_title />
                                        <label class="block space-y-1">
                                            <span class="text-sm text-muted-foreground">{t(ui_locale_product.as_deref(), "ai.field.sourceDescriptionOverride", "Source description override")}</span>
                                            <textarea
                                                class="min-h-24 w-full rounded-lg border border-input bg-background px-3 py-2 text-sm"
                                                prop:value=product_source_description
                                                on:input=move |ev| product_source_description.set(event_target_value(&ev))
                                            />
                                        </label>
                                        <TextField label=t(ui_locale_product.as_deref(), "ai.field.sourceMetaTitleOverride", "Source meta title override") value=product_source_meta_title />
                                        <label class="block space-y-1">
                                            <span class="text-sm text-muted-foreground">{t(ui_locale_product.as_deref(), "ai.field.sourceMetaDescriptionOverride", "Source meta description override")}</span>
                                            <textarea
                                                class="min-h-20 w-full rounded-lg border border-input bg-background px-3 py-2 text-sm"
                                                prop:value=product_source_meta_description
                                                on:input=move |ev| product_source_meta_description.set(event_target_value(&ev))
                                            />
                                        </label>
                                        <label class="block space-y-1">
                                            <span class="text-sm text-muted-foreground">{t(ui_locale_product.as_deref(), "ai.field.copyInstructions", "Copy instructions")}</span>
                                            <textarea
                                                class="min-h-20 w-full rounded-lg border border-input bg-background px-3 py-2 text-sm"
                                                prop:value=product_copy_instructions
                                                on:input=move |ev| product_copy_instructions.set(event_target_value(&ev))
                                            />
                                        </label>
                                        <TextField label=t(ui_locale_product.as_deref(), "ai.field.assistantPrompt", "Assistant prompt") value=product_assistant_prompt />
                                        <div class="rounded-lg border border-border px-3 py-2 text-sm text-muted-foreground">
                                            {move || direct_transport_summary(
                                                product_transport_locale.as_deref(),
                                                selected_provider.get().as_str(),
                                                selected_task_profile.get().as_str(),
                                            )}
                                        </div>
                                        <button type="submit" class="rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground">{t(ui_locale_product.as_deref(), "ai.action.generateProductCopy", "Generate product copy")}</button>
                                    </form>
                                </Card>


                                <Card title=t(ui_locale_product_attributes.as_deref(), "ai.card.productAttributes", "Product Attributes")>
                                    <form class="space-y-3" on:submit=move |ev| on_run_product_attributes_job.run(ev)>
                                        <TextField label=t(ui_locale_product_attributes.as_deref(), "ai.field.jobTitle", "Job title") value=product_attributes_title />
                                        <TextField
                                            label=t(ui_locale_product_attributes.as_deref(), "ai.field.locale", "Locale")
                                            value=product_attributes_locale
                                            placeholder=t(ui_locale_product_attributes.as_deref(), "ai.field.localeAutoPlaceholder", "auto (request locale -> tenant default -> en)")
                                        />
                                        <TextField label=t(ui_locale_product_attributes.as_deref(), "ai.field.productId", "Product id") value=product_attributes_product_id />
                                        <TextField label=t(ui_locale_product_attributes.as_deref(), "ai.field.categorySlug", "Category slug") value=product_attributes_category_slug />
                                        <TextField label=t(ui_locale_product_attributes.as_deref(), "ai.field.sourceLocale", "Source locale") value=product_attributes_source_locale />
                                        <TextField label=t(ui_locale_product_attributes.as_deref(), "ai.field.sourceTitleOverride", "Source title override") value=product_attributes_source_title />
                                        <label class="block space-y-1">
                                            <span class="text-sm text-muted-foreground">{t(ui_locale_product_attributes.as_deref(), "ai.field.sourceDescriptionOverride", "Source description override")}</span>
                                            <textarea
                                                class="min-h-24 w-full rounded-lg border border-input bg-background px-3 py-2 text-sm"
                                                prop:value=product_attributes_source_description
                                                on:input=move |ev| product_attributes_source_description.set(event_target_value(&ev))
                                            />
                                        </label>
                                        <TextField label=t(ui_locale_product_attributes.as_deref(), "ai.field.imageUrlsCsv", "Image URLs (csv)") value=product_attributes_image_urls />
                                        <label class="block space-y-1">
                                            <span class="text-sm text-muted-foreground">{t(ui_locale_product_attributes.as_deref(), "ai.field.copyInstructions", "Copy instructions")}</span>
                                            <textarea
                                                class="min-h-20 w-full rounded-lg border border-input bg-background px-3 py-2 text-sm"
                                                prop:value=product_attributes_copy_instructions
                                                on:input=move |ev| product_attributes_copy_instructions.set(event_target_value(&ev))
                                            />
                                        </label>
                                        <TextField label=t(ui_locale_product_attributes.as_deref(), "ai.field.assistantPrompt", "Assistant prompt") value=product_attributes_assistant_prompt />
                                        <div class="rounded-lg border border-border px-3 py-2 text-sm text-muted-foreground">
                                            {move || direct_transport_summary(
                                                product_attributes_transport_locale.as_deref(),
                                                selected_provider.get().as_str(),
                                                selected_task_profile.get().as_str(),
                                            )}
                                        </div>
                                        <Show when=move || !can_submit_product_attributes.get()>
                                            <p class="text-xs text-muted-foreground">{t(ui_locale_product_attributes_hint.as_deref(), "ai.hint.productAttributesRequirements", "Select task profile and product id to enable generation.")}</p>
                                        </Show>
                                        <button
                                            type="submit"
                                            class="rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground disabled:cursor-not-allowed disabled:opacity-60"
                                            disabled=move || !can_submit_product_attributes.get()
                                        >
                                            {t(ui_locale_product_attributes.as_deref(), "ai.action.generateProductAttributes", "Generate product attributes")}
                                        </button>
                                    </form>
                                </Card>

                                <Card title=t(ui_locale_order_analytics.as_deref(), "ai.card.orderAnalytics", "Order Analytics")>
                                    <form class="space-y-3" on:submit=move |ev| on_run_order_analytics_job.run(ev)>
                                        <TextField label=t(ui_locale_order_analytics.as_deref(), "ai.field.jobTitle", "Job title") value=order_analytics_title />
                                        <TextField
                                            label=t(ui_locale_order_analytics.as_deref(), "ai.field.locale", "Locale")
                                            value=order_analytics_locale
                                            placeholder=t(ui_locale_order_analytics.as_deref(), "ai.field.localeAutoPlaceholder", "auto (request locale -> tenant default -> en)")
                                        />
                                        <TextField label=t(ui_locale_order_analytics.as_deref(), "ai.field.orderIdsCsv", "Order ids (csv)") value=order_analytics_order_ids />
                                        <TextField
                                            label=t(ui_locale_order_analytics.as_deref(), "ai.field.dateFrom", "Date from (RFC 3339, optional)")
                                            value=order_analytics_date_from
                                        />
                                        <TextField
                                            label=t(ui_locale_order_analytics.as_deref(), "ai.field.dateTo", "Date to (RFC 3339, optional)")
                                            value=order_analytics_date_to
                                        />
                                        <TextField label=t(ui_locale_order_analytics.as_deref(), "ai.field.focus", "Analysis focus") value=order_analytics_focus />
                                        <TextField label=t(ui_locale_order_analytics.as_deref(), "ai.field.assistantPrompt", "Assistant prompt") value=order_analytics_assistant_prompt />
                                        <div class="rounded-lg border border-border px-3 py-2 text-sm text-muted-foreground">
                                            {move || direct_transport_summary(
                                                order_analytics_transport_locale.as_deref(),
                                                selected_provider.get().as_str(),
                                                selected_task_profile.get().as_str(),
                                            )}
                                        </div>
                                        <p class="text-xs text-muted-foreground">
                                            {t(ui_locale_order_analytics_hint.as_deref(), "ai.hint.orderAdvisory", "Advisory output only. Review generated findings before taking any order action.")}
                                        </p>
                                        <Show when=move || !can_submit_order_analytics.get()>
                                            <p class="text-xs text-muted-foreground">{t(ui_locale_order_analytics_hint.as_deref(), "ai.hint.orderAnalyticsRequirements", "Select the active `order_analytics` task profile and provide at least one order id.")}</p>
                                        </Show>
                                        <button
                                            type="submit"
                                            class="rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground disabled:cursor-not-allowed disabled:opacity-60"
                                            disabled=move || !can_submit_order_analytics.get()
                                        >
                                            {t(ui_locale_order_analytics.as_deref(), "ai.action.generateOrderAnalytics", "Generate order analytics")}
                                        </button>
                                    </form>
                                </Card>

                                <Card title=t(ui_locale_order_ops.as_deref(), "ai.card.orderOpsAssistant", "Order Operations Assistant")>
                                    <form class="space-y-3" on:submit=move |ev| on_run_order_ops_job.run(ev)>
                                        <TextField label=t(ui_locale_order_ops.as_deref(), "ai.field.jobTitle", "Job title") value=order_ops_title />
                                        <TextField
                                            label=t(ui_locale_order_ops.as_deref(), "ai.field.locale", "Locale")
                                            value=order_ops_locale
                                            placeholder=t(ui_locale_order_ops.as_deref(), "ai.field.localeAutoPlaceholder", "auto (request locale -> tenant default -> en)")
                                        />
                                        <TextField label=t(ui_locale_order_ops.as_deref(), "ai.field.orderId", "Order id") value=order_ops_order_id />
                                        <TextField label=t(ui_locale_order_ops.as_deref(), "ai.field.recommendedAction", "Requested action (optional)") value=order_ops_recommended_action />
                                        <label class="block space-y-1">
                                            <span class="text-sm text-muted-foreground">{t(ui_locale_order_ops.as_deref(), "ai.field.orderContext", "Operator context")}</span>
                                            <textarea
                                                class="min-h-24 w-full rounded-lg border border-input bg-background px-3 py-2 text-sm"
                                                prop:value=order_ops_context
                                                on:input=move |ev| order_ops_context.set(event_target_value(&ev))
                                            />
                                        </label>
                                        <TextField label=t(ui_locale_order_ops.as_deref(), "ai.field.assistantPrompt", "Assistant prompt") value=order_ops_assistant_prompt />
                                        <div class="rounded-lg border border-border px-3 py-2 text-sm text-muted-foreground">
                                            {move || direct_transport_summary(
                                                order_ops_transport_locale.as_deref(),
                                                selected_provider.get().as_str(),
                                                selected_task_profile.get().as_str(),
                                            )}
                                        </div>
                                        <p class="text-xs text-muted-foreground">
                                            {t(ui_locale_order_ops_hint.as_deref(), "ai.hint.orderOpsAdvisory", "Sensitive advisory output only. It cannot modify an order and requires operator review.")}
                                        </p>
                                        <Show when=move || !can_submit_order_ops.get()>
                                            <p class="text-xs text-muted-foreground">{t(ui_locale_order_ops_hint.as_deref(), "ai.hint.orderOpsRequirements", "Select the active `order_ops_assistant` task profile and provide an order id.")}</p>
                                        </Show>
                                        <button
                                            type="submit"
                                            class="rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground disabled:cursor-not-allowed disabled:opacity-60"
                                            disabled=move || !can_submit_order_ops.get()
                                        >
                                            {t(ui_locale_order_ops.as_deref(), "ai.action.runOrderOpsAssistant", "Run order operations assistant")}
                                        </button>
                                    </form>
                                </Card>

                                <Card title=t(ui_locale_image.as_deref(), "ai.card.mediaImage", "Media Image")>
                                    <form class="space-y-3" on:submit=move |ev| on_run_image_job.run(ev)>
                                        <TextField label=t(ui_locale_image.as_deref(), "ai.field.jobTitle", "Job title") value=image_title />
                                        <TextField
                                            label=t(ui_locale_image.as_deref(), "ai.field.locale", "Locale")
                                            value=image_locale
                                            placeholder=t(ui_locale_image.as_deref(), "ai.field.localeAutoPlaceholder", "auto (request locale -> tenant default -> en)")
                                        />
                                        <TextField label=t(ui_locale_image.as_deref(), "ai.field.prompt", "Prompt") value=image_prompt />
                                        <TextField label=t(ui_locale_image.as_deref(), "ai.field.negativePrompt", "Negative prompt") value=image_negative_prompt />
                                        <TextField label=t(ui_locale_image.as_deref(), "ai.field.fileName", "File name") value=image_file_name />
                                        <TextField label=t(ui_locale_image.as_deref(), "ai.field.mediaTitle", "Media title") value=image_asset_title />
                                        <TextField label=t(ui_locale_image.as_deref(), "ai.field.altText", "Alt text") value=image_alt_text />
                                        <TextField label=t(ui_locale_image.as_deref(), "ai.field.caption", "Caption") value=image_caption />
                                        <TextField label=t(ui_locale_image.as_deref(), "ai.field.size", "Size") value=image_size />
                                        <TextField label=t(ui_locale_image.as_deref(), "ai.field.assistantPrompt", "Assistant prompt") value=image_assistant_prompt />
                                        <div class="rounded-lg border border-border px-3 py-2 text-sm text-muted-foreground">
                                            {move || direct_transport_summary(
                                                image_transport_locale.as_deref(),
                                                selected_provider.get().as_str(),
                                                selected_task_profile.get().as_str(),
                                            )}
                                        </div>
                                        <button type="submit" class="rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground">{t(ui_locale_image.as_deref(), "ai.action.generateMediaImage", "Generate media image")}</button>
                                    </form>
                                </Card>

                                <Card title=t(ui_locale_alloy.as_deref(), "ai.card.alloyAssist", "Alloy Assist")>
                                    <form class="space-y-3" on:submit=move |ev| on_run_alloy_job.run(ev)>
                                        <TextField label=t(ui_locale_alloy.as_deref(), "ai.field.jobTitle", "Job title") value=alloy_title />
                                        <TextField
                                            label=t(ui_locale_alloy.as_deref(), "ai.field.locale", "Locale")
                                            value=alloy_locale
                                            placeholder=t(ui_locale_alloy.as_deref(), "ai.field.localeAutoPlaceholder", "auto (request locale -> tenant default -> en)")
                                        />
                                        <TextField label=t(ui_locale_alloy.as_deref(), "ai.field.operation", "Operation") value=alloy_operation />
                                        <TextField label=t(ui_locale_alloy.as_deref(), "ai.field.scriptId", "Script id") value=alloy_script_id />
                                        <TextField label=t(ui_locale_alloy.as_deref(), "ai.field.scriptName", "Script name") value=alloy_script_name />
                                        <TextField label=t(ui_locale_alloy.as_deref(), "ai.field.assistantPrompt", "Assistant prompt") value=alloy_prompt />
                                        <label class="block space-y-1">
                                            <span class="text-sm text-muted-foreground">{t(ui_locale_alloy.as_deref(), "ai.field.scriptSource", "Script source")}</span>
                                            <textarea
                                                class="min-h-28 w-full rounded-lg border border-input bg-background px-3 py-2 text-sm"
                                                prop:value=alloy_script_source
                                                on:input=move |ev| alloy_script_source.set(event_target_value(&ev))
                                            />
                                        </label>
                                        <label class="block space-y-1">
                                            <span class="text-sm text-muted-foreground">{t(ui_locale_alloy.as_deref(), "ai.field.runtimePayloadJson", "Runtime payload JSON")}</span>
                                            <textarea
                                                class="min-h-24 w-full rounded-lg border border-input bg-background px-3 py-2 text-sm"
                                                prop:value=alloy_runtime_payload
                                                on:input=move |ev| alloy_runtime_payload.set(event_target_value(&ev))
                                            />
                                        </label>
                                        <div class="rounded-lg border border-border px-3 py-2 text-sm text-muted-foreground">
                                            {move || direct_transport_summary(
                                                alloy_transport_locale.as_deref(),
                                                selected_provider.get().as_str(),
                                                selected_task_profile.get().as_str(),
                                            )}
                                        </div>
                                        <button type="submit" class="rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground">{t(ui_locale_alloy.as_deref(), "ai.action.runAlloyJob", "Run Alloy job")}</button>
                                    </form>
                                </Card>

                                <Card title=t(ui_locale_new_session.as_deref(), "ai.card.newSession", "New Session")>
                                    <form class="space-y-3" on:submit=move |ev| on_start_session.run(ev)>
                                        <TextField label=t(ui_locale_new_session.as_deref(), "ai.field.title", "Title") value=session_title />
                                        <TextField
                                            label=t(ui_locale_new_session.as_deref(), "ai.field.locale", "Locale")
                                            value=session_locale
                                            placeholder=t(ui_locale_new_session.as_deref(), "ai.field.localeAutoPlaceholder", "auto (request locale -> tenant default -> en)")
                                        />
                                        <TextField label=t(ui_locale_new_session.as_deref(), "ai.field.initialMessage", "Initial message") value=session_message />
                                        <div class="rounded-lg border border-border px-3 py-2 text-sm text-muted-foreground">
                                            {move || session_transport_summary(
                                                session_transport_locale.as_deref(),
                                                selected_provider.get().as_str(),
                                                selected_task_profile.get().as_str(),
                                                selected_tool_profile.get().as_str(),
                                            )}
                                        </div>
                                        <button type="submit" class="rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground">{t(ui_locale_new_session.as_deref(), "ai.action.startSession", "Start session")}</button>
                                    </form>
                                </Card>
    }
}
