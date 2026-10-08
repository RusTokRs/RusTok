use crate::entities::product::{ProductCard, ProductCardData};
use crate::shared::ui::{UiButton, UiInput};
use leptos::prelude::AnyView;
use leptos::prelude::*;
use leptos_ui::ButtonVariant;

#[component]
pub fn HomePage(
    hero_title: String,
    hero_subtitle: String,
    cta_primary: String,
    cta_secondary: String,
    featured_title: String,
    featured_subtitle: String,
    products: Vec<ProductCardData>,
    badge_new: String,
    cta_view: String,
    story_title: String,
    story_body: String,
    newsletter_title: String,
    newsletter_body: String,
    newsletter_placeholder: String,
    newsletter_cta: String,
    newsletter_note: String,
    after_hero_views: Vec<AnyView>,
    after_catalog_views: Vec<AnyView>,
    before_footer_views: Vec<AnyView>,
) -> impl IntoView {
    view! {
        <main class="space-y-24 overflow-hidden">
            // Hero Section with Ambient Luxury Aura and Live Spotlight Card
            <section id="home" class="relative pt-12 pb-20 lg:pt-20 lg:pb-32">
                // Background Ambient Glows
                <div class="pointer-events-none absolute inset-0 -z-10 overflow-hidden">
                    <div class="absolute -top-40 left-1/2 -translate-x-1/2 w-[700px] h-[500px] rounded-full bg-gradient-to-br from-primary/20 via-orange-500/10 to-transparent blur-3xl opacity-70 dark:opacity-40" />
                    <div class="absolute top-20 -left-20 w-96 h-96 rounded-full bg-accent/20 blur-3xl opacity-50" />
                    <div class="absolute top-40 -right-20 w-96 h-96 rounded-full bg-primary/15 blur-3xl opacity-50" />
                </div>

                <div class="container-app px-4 sm:px-6">
                    <div class="grid grid-cols-1 lg:grid-cols-12 gap-12 lg:gap-8 items-center">
                        // Left Column: Hero Content & Metrics
                        <div class="lg:col-span-7 space-y-8">
                            // Top Announcement Badge
                            <div class="inline-flex items-center gap-2 rounded-full border border-primary/25 bg-primary/10 px-4 py-1.5 backdrop-blur-md transition-all hover:bg-primary/15">
                                <span class="flex h-2 w-2 rounded-full bg-primary animate-pulse" />
                                <span class="text-xs font-semibold text-primary tracking-wide">
                                    "⚡ RusToK Native Platform • Zero-GC Memory Safety • Sub-50ms SSR"
                                </span>
                            </div>

                            // Main Headline
                            <h1 class="text-4xl sm:text-5xl lg:text-6xl font-extrabold tracking-tight text-foreground leading-[1.12]">
                                {hero_title}
                            </h1>

                            // Subtitle
                            <p class="text-lg sm:text-xl text-muted-foreground font-normal leading-relaxed max-w-xl">
                                {hero_subtitle}
                            </p>

                            // Call to Action Buttons
                            <div class="flex flex-wrap items-center gap-4 pt-2">
                                <a href="#catalog" class="inline-flex">
                                    <UiButton class="px-7 py-3.5 text-sm font-semibold rounded-xl bg-primary text-primary-foreground shadow-lg shadow-primary/25 hover:shadow-primary/40 hover:-translate-y-0.5 transition-all flex items-center gap-2">
                                        <span>{cta_primary}</span>
                                        <svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                                            <path d="M5 12h14M12 5l7 7-7 7" />
                                        </svg>
                                    </UiButton>
                                </a>
                                <a href="#about" class="inline-flex">
                                    <UiButton variant=ButtonVariant::Outline class="px-7 py-3.5 text-sm font-semibold rounded-xl border-border/80 hover:bg-secondary/60 hover:-translate-y-0.5 transition-all">
                                        {cta_secondary}
                                    </UiButton>
                                </a>
                            </div>

                            // Live Platform Metrics Strip
                            <div class="grid grid-cols-2 sm:grid-cols-4 gap-4 pt-6 border-t border-border/60">
                                <div class="space-y-1">
                                    <div class="text-2xl font-black tracking-tight text-foreground">"< 50ms"</div>
                                    <div class="text-xs text-muted-foreground font-medium">"Cold First Paint"</div>
                                </div>
                                <div class="space-y-1">
                                    <div class="text-2xl font-black tracking-tight text-primary">"0ms"</div>
                                    <div class="text-xs text-muted-foreground font-medium">"GC Stutter (Rust)"</div>
                                </div>
                                <div class="space-y-1">
                                    <div class="text-2xl font-black tracking-tight text-foreground">"50k+"</div>
                                    <div class="text-xs text-muted-foreground font-medium">"RPS on $5 VPS"</div>
                                </div>
                                <div class="space-y-1">
                                    <div class="text-2xl font-black tracking-tight text-emerald-500">"100%"</div>
                                    <div class="text-xs text-muted-foreground font-medium">"Zero-Trust RLS"</div>
                                </div>
                            </div>
                        </div>

                        // Right Column: Glassmorphic Hardware & Architecture Showcase
                        <div class="lg:col-span-5 relative">
                            <div class="relative mx-auto max-w-md rounded-3xl border border-border/80 bg-card/75 p-6 backdrop-blur-2xl shadow-2xl shadow-primary/10 transition-all duration-500 hover:shadow-primary/20">
                                // Showcase Header
                                <div class="flex items-center justify-between pb-4 border-b border-border/60">
                                    <div class="flex items-center gap-2">
                                        <span class="h-3 w-3 rounded-full bg-rose-500/80" />
                                        <span class="h-3 w-3 rounded-full bg-amber-500/80" />
                                        <span class="h-3 w-3 rounded-full bg-emerald-500/80" />
                                        <span class="ml-2 text-xs font-mono font-medium text-muted-foreground">
                                            "rustok-engine-v1.0"
                                        </span>
                                    </div>
                                    <span class="inline-flex items-center gap-1.5 rounded-full bg-emerald-500/15 border border-emerald-500/30 px-2 py-0.5 text-[11px] font-semibold text-emerald-600 dark:text-emerald-400">
                                        <span class="h-1.5 w-1.5 rounded-full bg-emerald-500 animate-pulse" />
                                        "LIVE SSR"
                                    </span>
                                </div>

                                // Spotlight Featured Product Preview
                                <div class="mt-4 space-y-4">
                                    <div class="relative h-48 w-full overflow-hidden rounded-2xl bg-gradient-to-tr from-secondary/80 to-accent/30 flex items-center justify-center group">
                                        <img
                                            src="https://images.unsplash.com/photo-1505740420928-5e560c06d30e?w=600&auto=format&fit=crop&q=80"
                                            alt="RusTok Pro Sound"
                                            class="h-full w-full object-cover transition-transform duration-700 group-hover:scale-105"
                                        />
                                        <div class="absolute top-3 left-3">
                                            <span class="rounded-full bg-background/85 backdrop-blur-md px-2.5 py-0.5 text-[11px] font-semibold text-primary border border-primary/20 shadow-xs">
                                                "FLAGSHIP"
                                            </span>
                                        </div>
                                        <div class="absolute bottom-3 right-3">
                                            <span class="rounded-xl bg-foreground/90 backdrop-blur-md px-3 py-1 text-xs font-bold text-background shadow-md">
                                                "$139"
                                            </span>
                                        </div>
                                    </div>

                                    <div class="space-y-1.5">
                                        <div class="flex items-center justify-between">
                                            <h4 class="text-sm font-bold text-foreground">"RusTok Pro Sound Hi-Fi"</h4>
                                            <div class="flex items-center text-amber-500 text-xs">
                                                "★★★★★"
                                                <span class="ml-1 text-[11px] font-semibold text-foreground">"4.9"</span>
                                            </div>
                                        </div>
                                        <p class="text-xs text-muted-foreground line-clamp-1">
                                            "Studio-grade wireless headphones with adaptive ANC and ultra-low latency."
                                        </p>
                                    </div>

                                    // Architectural Telemetry Stream
                                    <div class="rounded-xl bg-muted/60 p-3 font-mono text-[11px] space-y-1 border border-border/50 text-muted-foreground">
                                        <div class="flex items-center justify-between">
                                            <span class="text-emerald-500">"> axum::serve()"</span>
                                            <span class="text-[10px] text-foreground font-semibold">"127.0.0.1:3101"</span>
                                        </div>
                                        <div class="flex items-center justify-between">
                                            <span>"> sea_orm::pool"</span>
                                            <span class="text-[10px] text-emerald-500">"PostgreSQL 18"</span>
                                        </div>
                                        <div class="flex items-center justify-between">
                                            <span>"> outbox_events"</span>
                                            <span class="text-[10px] text-emerald-500">"0 pending / synced"</span>
                                        </div>
                                    </div>
                                </div>
                            </div>
                        </div>
                    </div>
                </div>
            </section>

            {after_hero_views.into_iter().collect_view()}

            // Catalog Showcase Section
            <section id="catalog" class="container-app px-4 sm:px-6">
                <div class="mx-auto max-w-7xl space-y-8">
                    // Section Header
                    <div class="flex flex-col md:flex-row md:items-end md:justify-between gap-4 border-b border-border/60 pb-6">
                        <div class="space-y-2">
                            <div class="inline-flex items-center gap-1.5 rounded-full bg-secondary px-3 py-1 text-xs font-semibold text-secondary-foreground">
                                <svg class="h-3.5 w-3.5 text-primary" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                                    <path d="M6 2L3 6v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2V6l-3-4z" />
                                    <line x1="3" y1="6" x2="21" y2="6" />
                                </svg>
                                <span>"CURATED HARDWARE & ACCESSORIES"</span>
                            </div>
                            <h2 class="text-3xl sm:text-4xl font-extrabold tracking-tight text-foreground">
                                {featured_title}
                            </h2>
                            <p class="text-sm sm:text-base text-muted-foreground max-w-2xl">
                                {featured_subtitle}
                            </p>
                        </div>

                        // Category Chips
                        <div class="flex items-center gap-2 overflow-x-auto pb-1 text-xs">
                            <span class="rounded-xl bg-primary text-primary-foreground px-3.5 py-1.5 font-semibold cursor-pointer shadow-xs">
                                "All Products"
                            </span>
                            <span class="rounded-xl border border-border bg-card/60 text-muted-foreground hover:text-foreground hover:bg-secondary px-3.5 py-1.5 font-medium cursor-pointer transition-colors">
                                "Smart Tech"
                            </span>
                            <span class="rounded-xl border border-border bg-card/60 text-muted-foreground hover:text-foreground hover:bg-secondary px-3.5 py-1.5 font-medium cursor-pointer transition-colors">
                                "Hi-Fi Audio"
                            </span>
                            <span class="rounded-xl border border-border bg-card/60 text-muted-foreground hover:text-foreground hover:bg-secondary px-3.5 py-1.5 font-medium cursor-pointer transition-colors">
                                "Power & Desk"
                            </span>
                        </div>
                    </div>

                    // Products 3-Column Responsive Grid
                    <div class="grid gap-6 sm:grid-cols-2 lg:grid-cols-3">
                        {products
                            .into_iter()
                            .map(|product| {
                                let badge_new = badge_new.clone();
                                let cta_view = cta_view.clone();
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

            {after_catalog_views.into_iter().collect_view()}

            // Bento Grid: Architecture, Invariants & Newsletter
            <section id="about" class="container-app px-4 sm:px-6">
                <div class="mx-auto max-w-7xl space-y-8">
                    // Section Header
                    <div class="space-y-2 text-center max-w-3xl mx-auto">
                        <div class="inline-flex items-center gap-1.5 rounded-full bg-secondary px-3 py-1 text-xs font-semibold text-secondary-foreground">
                            <span>"PLATFORM ARCHITECTURE"</span>
                        </div>
                        <h2 class="text-3xl sm:text-4xl font-extrabold tracking-tight text-foreground">
                            "Engineered for Hyper-Growth & Zero Downtime"
                        </h2>
                        <p class="text-sm sm:text-base text-muted-foreground">
                            "A single unified Rust platform delivering high-performance commerce, full-text search, and event outbox without runtime bloat."
                        </p>
                    </div>

                    // Bento Grid
                    <div class="grid gap-6 md:grid-cols-3">
                        // Bento Card 1: Story & Core Stack (Span 2 cols on md+)
                        <div class="md:col-span-2 relative overflow-hidden rounded-3xl border border-border/80 bg-card/70 p-8 backdrop-blur-xl shadow-xs space-y-6 flex flex-col justify-between">
                            <div class="space-y-4">
                                <div class="flex items-center gap-3">
                                    <div class="flex h-10 w-10 items-center justify-center rounded-xl bg-primary/10 text-primary border border-primary/20">
                                        <svg class="h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                                            <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z" />
                                        </svg>
                                    </div>
                                    <div>
                                        <h3 class="text-xl font-bold text-card-foreground">{story_title}</h3>
                                        <p class="text-xs text-muted-foreground">"Compiled native code • Zero runtime legacy"</p>
                                    </div>
                                </div>
                                <p class="text-sm text-muted-foreground leading-relaxed">
                                    {story_body}
                                </p>
                            </div>

                            // Technology Tags
                            <div class="pt-4 border-t border-border/50">
                                <div class="text-xs font-semibold uppercase tracking-wider text-muted-foreground mb-3">
                                    "Native Technology Stack"
                                </div>
                                <div class="flex flex-wrap gap-2">
                                    <span class="inline-flex items-center gap-1 rounded-lg border border-border bg-secondary/60 px-3 py-1 text-xs font-semibold text-foreground">
                                        <span>"🦀 Rust 2024"</span>
                                    </span>
                                    <span class="inline-flex items-center gap-1 rounded-lg border border-border bg-secondary/60 px-3 py-1 text-xs font-semibold text-foreground">
                                        <span>"⚡ Axum SSR"</span>
                                    </span>
                                    <span class="inline-flex items-center gap-1 rounded-lg border border-border bg-secondary/60 px-3 py-1 text-xs font-semibold text-foreground">
                                        <span>"🐘 PostgreSQL 18"</span>
                                    </span>
                                    <span class="inline-flex items-center gap-1 rounded-lg border border-border bg-secondary/60 px-3 py-1 text-xs font-semibold text-foreground">
                                        <span>"🌐 Leptos WASM"</span>
                                    </span>
                                    <span class="inline-flex items-center gap-1 rounded-lg border border-border bg-secondary/60 px-3 py-1 text-xs font-semibold text-foreground">
                                        <span>"📦 Transactional Outbox"</span>
                                    </span>
                                    <span class="inline-flex items-center gap-1 rounded-lg border border-border bg-secondary/60 px-3 py-1 text-xs font-semibold text-foreground">
                                        <span>"🔍 pgvector & RAG"</span>
                                    </span>
                                </div>
                            </div>
                        </div>

                        // Bento Card 2: Performance & Zero-Trust
                        <div class="relative overflow-hidden rounded-3xl border border-border/80 bg-gradient-to-br from-card/80 to-secondary/30 p-8 backdrop-blur-xl shadow-xs space-y-6 flex flex-col justify-between">
                            <div class="space-y-4">
                                <div class="flex h-10 w-10 items-center justify-center rounded-xl bg-emerald-500/10 text-emerald-500 border border-emerald-500/20">
                                    <svg class="h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                                        <polyline points="22 12 18 12 15 21 9 3 6 12 2 12" />
                                    </svg>
                                </div>
                                <h3 class="text-xl font-bold text-card-foreground">"Extreme Scale & RLS"</h3>
                                <p class="text-xs text-muted-foreground leading-relaxed">
                                    "Row-Level Security isolates tenants directly inside PostgreSQL. Atomic mutations guarantee complete zero-data-loss consistency."
                                </p>
                            </div>

                            <div class="space-y-2.5 pt-4 border-t border-border/50 text-xs">
                                <div class="flex items-center gap-2 text-foreground font-medium">
                                    <svg class="h-4 w-4 text-emerald-500 shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                                        <polyline points="20 6 9 17 4 12" />
                                    </svg>
                                    <span>"Multi-Tenant Database Isolation"</span>
                                </div>
                                <div class="flex items-center gap-2 text-foreground font-medium">
                                    <svg class="h-4 w-4 text-emerald-500 shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                                        <polyline points="20 6 9 17 4 12" />
                                    </svg>
                                    <span>"Distributed Event Streaming"</span>
                                </div>
                                <div class="flex items-center gap-2 text-foreground font-medium">
                                    <svg class="h-4 w-4 text-emerald-500 shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                                        <polyline points="20 6 9 17 4 12" />
                                    </svg>
                                    <span>"Microsecond Query Execution"</span>
                                </div>
                            </div>
                        </div>

                        // Bento Card 3: Newsletter & VIP Updates (Span 3 cols full width)
                        <div class="md:col-span-3 relative overflow-hidden rounded-3xl border border-primary/20 bg-gradient-to-r from-card/80 via-primary/5 to-card/80 p-8 backdrop-blur-xl shadow-xs">
                            <div class="grid grid-cols-1 lg:grid-cols-12 gap-6 items-center">
                                <div class="lg:col-span-7 space-y-2">
                                    <div class="flex items-center gap-2">
                                        <span class="flex h-2 w-2 rounded-full bg-primary" />
                                        <span class="text-xs font-semibold uppercase tracking-wider text-primary">"STAY CONNECTED"</span>
                                    </div>
                                    <h3 class="text-2xl font-bold text-card-foreground">{newsletter_title}</h3>
                                    <p class="text-sm text-muted-foreground">{newsletter_body}</p>
                                </div>

                                <div class="lg:col-span-5 space-y-2">
                                    <div class="flex gap-2">
                                        <div class="flex-1">
                                            <UiInput
                                                placeholder=newsletter_placeholder
                                                class="rounded-xl border-border/80 bg-background/80"
                                            />
                                        </div>
                                        <UiButton class="px-5 py-2.5 text-xs font-semibold rounded-xl bg-primary text-primary-foreground shadow-sm hover:shadow-md transition-all shrink-0">
                                            {newsletter_cta}
                                        </UiButton>
                                    </div>
                                    <p class="text-[11px] text-muted-foreground">{newsletter_note}</p>
                                </div>
                            </div>
                        </div>
                    </div>
                </div>
            </section>

            {before_footer_views.into_iter().collect_view()}
        </main>
    }
}
