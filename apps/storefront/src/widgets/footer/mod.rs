use leptos::prelude::*;

#[component]
pub fn Footer(tagline: String, navigation_views: Vec<AnyView>) -> impl IntoView {
    view! {
        <footer id="contact" class="mt-28 border-t border-border/70 bg-card/40 backdrop-blur-md pt-16 pb-12">
            <div class="container-app px-4 sm:px-6 space-y-12">
                <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-12 gap-8 lg:gap-12">
                    // Brand & Status Column (Span 4)
                    <div class="lg:col-span-4 space-y-4">
                        <div class="flex items-center gap-2.5">
                            <div class="flex h-8 w-8 items-center justify-center rounded-xl bg-gradient-to-tr from-primary to-orange-500 text-white shadow-sm">
                                <svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2">
                                    <polygon points="12 2 2 7 12 12 22 7 12 2" />
                                    <polyline points="2 17 12 22 22 17" />
                                    <polyline points="2 12 12 17 22 12" />
                                </svg>
                            </div>
                            <span class="text-lg font-black tracking-tight text-foreground">
                                "Rus"<span class="text-primary">"ToK"</span>
                            </span>
                        </div>
                        <p class="text-sm text-muted-foreground leading-relaxed max-w-sm">
                            {tagline}
                        </p>
                        <div class="pt-2">
                            <span class="inline-flex items-center gap-2 rounded-full bg-emerald-500/10 border border-emerald-500/25 px-3 py-1 text-xs font-semibold text-emerald-600 dark:text-emerald-400">
                                <span class="h-2 w-2 rounded-full bg-emerald-500 animate-pulse" />
                                "All Systems Operational • 0ms GC"
                            </span>
                        </div>
                    </div>

                    // Quick Links Column (Span 2)
                    <div class="lg:col-span-2 space-y-3">
                        <h4 class="text-xs font-bold uppercase tracking-wider text-foreground">"Navigation"</h4>
                        <ul class="space-y-2 text-sm text-muted-foreground">
                            <li><a href="#home" class="hover:text-primary transition-colors">"Home"</a></li>
                            <li><a href="#catalog" class="hover:text-primary transition-colors">"Catalog"</a></li>
                            <li><a href="#about" class="hover:text-primary transition-colors">"Architecture"</a></li>
                            <li><a href="#contact" class="hover:text-primary transition-colors">"Contact"</a></li>
                        </ul>
                    </div>

                    // Platform Technology (Span 3)
                    <div class="lg:col-span-3 space-y-3">
                        <h4 class="text-xs font-bold uppercase tracking-wider text-foreground">"Engine Specs"</h4>
                        <ul class="space-y-2 text-sm text-muted-foreground">
                            <li class="flex items-center gap-1.5">
                                <span class="text-primary">"⚡"</span>
                                <span>"Axum SSR & Streaming"</span>
                            </li>
                            <li class="flex items-center gap-1.5">
                                <span class="text-primary">"🦀"</span>
                                <span>"Leptos WASM Islands"</span>
                            </li>
                            <li class="flex items-center gap-1.5">
                                <span class="text-primary">"🐘"</span>
                                <span>"PostgreSQL 18 RLS"</span>
                            </li>
                            <li class="flex items-center gap-1.5">
                                <span class="text-primary">"📦"</span>
                                <span>"Transactional Outbox"</span>
                            </li>
                        </ul>
                    </div>

                    // Platform Invariants & Slot Navigation (Span 3)
                    <div class="lg:col-span-3 space-y-3">
                        <h4 class="text-xs font-bold uppercase tracking-wider text-foreground">"Quality Standard"</h4>
                        <p class="text-xs text-muted-foreground leading-relaxed">
                            "Compiled natively with zero legacy adapters. Single canonical source of truth with PostgreSQL database-level isolation."
                        </p>
                        {(!navigation_views.is_empty()).then(|| view! {
                            <div class="pt-2">{navigation_views}</div>
                        })}
                    </div>
                </div>

                // Bottom Legal Bar
                <div class="pt-8 border-t border-border/60 flex flex-col sm:flex-row items-center justify-between gap-4 text-xs text-muted-foreground">
                    <p>"© 2026 RusTokRs. The modular platform for modern high-load commerce."</p>
                    <div class="flex items-center gap-2">
                        <span class="inline-flex items-center rounded-md bg-secondary px-2 py-0.5 font-mono text-[11px] font-medium text-secondary-foreground">"SSR"</span>
                        <span class="inline-flex items-center rounded-md bg-secondary px-2 py-0.5 font-mono text-[11px] font-medium text-secondary-foreground">"Tailwind v4"</span>
                        <span class="inline-flex items-center rounded-md bg-secondary px-2 py-0.5 font-mono text-[11px] font-medium text-secondary-foreground">"Leptos 0.8"</span>
                        <span class="inline-flex items-center rounded-md bg-secondary px-2 py-0.5 font-mono text-[11px] font-medium text-secondary-foreground">"PostgreSQL 18"</span>
                    </div>
                </div>
            </div>
        </footer>
    }
}
