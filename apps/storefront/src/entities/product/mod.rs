use crate::shared::ui::UiButton;
use leptos::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProductCardData {
    pub title: String,
    pub description: String,
    pub price: String,
    pub badge: Option<String>,
    #[serde(default)]
    pub image_url: Option<String>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub rating: Option<f32>,
    #[serde(default)]
    pub review_count: Option<u32>,
    #[serde(default)]
    pub original_price: Option<String>,
}

#[component]
pub fn ProductCard(product: ProductCardData, badge_new: String, cta_view: String) -> impl IntoView {
    let badge = product.badge.clone().unwrap_or(badge_new);
    let category = product.category.clone().unwrap_or_else(|| "Hardware".to_string());
    let rating = product.rating.unwrap_or(4.9);
    let review_count = product.review_count.unwrap_or(36);
    let image_url = product.image_url.clone();
    let original_price = product.original_price.clone();

    view! {
        <div class="group relative flex flex-col justify-between overflow-hidden rounded-2xl border border-border/80 bg-card/70 backdrop-blur-xl shadow-xs transition-all duration-300 hover:-translate-y-1.5 hover:shadow-2xl hover:shadow-primary/10 hover:border-primary/40">
            // Image Frame with Floating Badges
            <div class="relative h-52 w-full overflow-hidden bg-gradient-to-br from-primary/10 via-secondary/60 to-accent/20 flex items-center justify-center">
                {if let Some(url) = image_url {
                    view! {
                        <img
                            src=url
                            alt=product.title.clone()
                            class="h-full w-full object-cover transition-transform duration-700 group-hover:scale-105"
                            loading="lazy"
                        />
                    }.into_any()
                } else {
                    view! {
                        <div class="relative flex h-full w-full items-center justify-center">
                            <div class="absolute inset-0 bg-gradient-to-tr from-primary/10 via-transparent to-accent/20" />
                            <svg class="h-20 w-20 text-primary/30 transition-transform duration-500 group-hover:scale-110 group-hover:text-primary/50" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.2">
                                <rect x="2" y="3" width="20" height="14" rx="2" />
                                <line x1="8" y1="21" x2="16" y2="21" />
                                <line x1="12" y1="17" x2="12" y2="21" />
                            </svg>
                        </div>
                    }.into_any()
                }}

                // Floating Top Badges
                <div class="absolute top-3 left-3 right-3 flex items-center justify-between pointer-events-none">
                    <span class="inline-flex items-center rounded-full border border-primary/20 bg-background/85 px-2.5 py-0.5 text-[11px] font-semibold text-primary backdrop-blur-md shadow-xs">
                        {category}
                    </span>
                    <span class="inline-flex items-center rounded-full bg-emerald-500/15 border border-emerald-500/25 px-2.5 py-0.5 text-[11px] font-semibold text-emerald-600 dark:text-emerald-400 backdrop-blur-md">
                        {badge}
                    </span>
                </div>
            </div>

            // Card Body
            <div class="flex flex-col flex-1 justify-between p-5 space-y-4">
                <div class="space-y-2">
                    // Star Rating
                    <div class="flex items-center gap-1.5 text-xs text-amber-500 font-medium">
                        <span class="tracking-widest">"★★★★★"</span>
                        <span class="text-xs text-foreground font-semibold">{format!("{:.1}", rating)}</span>
                        <span class="text-[11px] text-muted-foreground">{format!("({review_count})")}</span>
                    </div>

                    // Title
                    <h3 class="text-base font-bold text-card-foreground group-hover:text-primary transition-colors line-clamp-1">
                        {product.title}
                    </h3>

                    // Description
                    <p class="text-xs text-muted-foreground line-clamp-2 leading-relaxed">
                        {product.description}
                    </p>
                </div>

                // Price and Action Button
                <div class="flex items-center justify-between pt-3 border-t border-border/50">
                    <div class="flex flex-col">
                        {original_price.map(|orig| view! {
                            <span class="text-[11px] text-muted-foreground line-through">{orig}</span>
                        })}
                        <span class="text-lg font-bold tracking-tight text-foreground">{product.price}</span>
                    </div>

                    <a href="#catalog" class="inline-flex">
                        <UiButton class="px-3.5 py-2 text-xs font-semibold rounded-xl shadow-xs hover:shadow-md transition-all flex items-center gap-1.5">
                            <svg class="h-3.5 w-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                                <path d="M6 2L3 6v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2V6l-3-4z" />
                                <line x1="3" y1="6" x2="21" y2="6" />
                                <path d="M16 10a4 4 0 0 1-8 0" />
                            </svg>
                            {cta_view}
                        </UiButton>
                    </a>
                </div>
            </div>
        </div>
    }
}
