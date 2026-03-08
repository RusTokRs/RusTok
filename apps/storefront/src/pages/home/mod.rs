use crate::entities::product::{ProductCard, ProductCardData};
use crate::shared::ui::{UiButton, UiInput};
use leptos::prelude::AnyView;
use leptos::prelude::*;
use leptos_ui::ButtonVariant;

#[component]
pub fn HomePage(
    hero_title: &'static str,
    hero_subtitle: &'static str,
    cta_primary: &'static str,
    cta_secondary: &'static str,
    featured_title: &'static str,
    featured_subtitle: &'static str,
    products: Vec<ProductCardData>,
    badge_new: &'static str,
    cta_view: &'static str,
    story_title: &'static str,
    story_body: &'static str,
    newsletter_title: &'static str,
    newsletter_body: &'static str,
    newsletter_placeholder: &'static str,
    newsletter_cta: &'static str,
    newsletter_note: &'static str,
    module_views: Vec<AnyView>,
) -> impl IntoView {
    view! {
        <main class="space-y-20">
            <section id="home" class="bg-background">
                <div class="container-app flex flex-col gap-10 py-16 lg:flex-row lg:items-center lg:justify-between px-4">
                    <div class="max-w-xl space-y-6">
                        <h1 class="text-4xl font-bold text-foreground lg:text-5xl">
                            {hero_title}
                        </h1>
                        <p class="text-lg text-muted-foreground">{hero_subtitle}</p>
                        <div class="flex flex-wrap gap-3">
                            <a href="#catalog">
                                <UiButton class="px-5 py-2.5 text-sm">
                                    {cta_primary}
                                </UiButton>
                            </a>
                            <a href="#about">
                                <UiButton variant=ButtonVariant::Outline class="px-5 py-2.5 text-sm">
                                    {cta_secondary}
                                </UiButton>
                            </a>
                        </div>
                    </div>
                </div>
            </section>

            {module_views.into_iter().collect_view()}

            <section id="catalog" class="container-app px-4">
                <div class="mx-auto max-w-6xl space-y-6">
                    <div>
                        <h2 class="text-3xl font-bold text-foreground">{featured_title}</h2>
                        <p class="mt-1 text-muted-foreground">{featured_subtitle}</p>
                    </div>
                    <div class="grid gap-6 md:grid-cols-2 lg:grid-cols-3">
                        {products
                            .into_iter()
                            .map(|product| {
                                view! {
                                    <ProductCard
                                        product=product
                                        badge_new=badge_new
                                        cta_view=cta_view
                                    />
                                }
                            })
                            .collect_view()}
                    </div>
                </div>
            </section>

            <section id="about" class="container-app px-4">
                <div class="mx-auto grid max-w-6xl gap-6 lg:grid-cols-2">
                    <div class="rounded-xl border border-border bg-card p-6 shadow space-y-3">
                        <h3 class="text-lg font-semibold text-card-foreground">{story_title}</h3>
                        <p class="text-sm text-muted-foreground">{story_body}</p>
                        <div class="flex gap-2 flex-wrap">
                            <span class="inline-flex items-center rounded-full border border-border px-2.5 py-0.5 text-xs font-medium text-foreground">"GraphQL"</span>
                            <span class="inline-flex items-center rounded-full border border-border px-2.5 py-0.5 text-xs font-medium text-foreground">"Events"</span>
                            <span class="inline-flex items-center rounded-full border border-border px-2.5 py-0.5 text-xs font-medium text-foreground">"Search"</span>
                        </div>
                    </div>
                    <div class="rounded-xl border border-border bg-card p-6 shadow space-y-4">
                        <h3 class="text-lg font-semibold text-card-foreground">{newsletter_title}</h3>
                        <p class="text-sm text-muted-foreground">{newsletter_body}</p>
                        <div class="flex gap-2">
                            <UiInput
                                placeholder=newsletter_placeholder
                            />
                            <UiButton variant=ButtonVariant::Secondary class="px-4 py-2 text-sm">
                                {newsletter_cta}
                            </UiButton>
                        </div>
                        <span class="text-xs text-muted-foreground">{newsletter_note}</span>
                    </div>
                </div>
            </section>
        </main>
    }
}
