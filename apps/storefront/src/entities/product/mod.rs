use crate::shared::ui::UiButton;
use leptos::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProductCardData {
    pub title: String,
    pub description: String,
    pub price: String,
    pub badge: Option<String>,
}

#[component]
pub fn ProductCard(product: ProductCardData, badge_new: String, cta_view: String) -> impl IntoView {
    let badge = product.badge.unwrap_or(badge_new);
    view! {
        <div class="rounded-xl border border-border bg-card shadow">
            <div class="h-40 w-full rounded-t-xl bg-gradient-to-br from-primary/10 to-secondary" />
            <div class="p-5 space-y-3">
                <div class="flex items-start justify-between gap-2">
                    <h3 class="text-base font-semibold text-card-foreground">{product.title}</h3>
                    <span class="inline-flex items-center rounded-full bg-secondary px-2.5 py-0.5 text-xs font-medium text-secondary-foreground">{badge}</span>
                </div>
                <p class="text-sm text-muted-foreground">{product.description}</p>
                <div class="flex items-center justify-between pt-1">
                    <span class="text-lg font-semibold text-foreground">{product.price}</span>
                    <UiButton class="px-3 py-1.5 text-sm">
                        {cta_view}
                    </UiButton>
                </div>
            </div>
        </div>
    }
}
