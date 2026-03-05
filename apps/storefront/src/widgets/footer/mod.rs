use leptos::prelude::*;

#[component]
pub fn Footer(tagline: &'static str) -> impl IntoView {
    view! {
        <footer id="contact" class="mt-20 border-t border-border bg-muted/40 px-4 py-10">
            <div class="container-app space-y-3 text-center">
                <p class="text-sm text-muted-foreground">{tagline}</p>
                <div class="flex justify-center gap-3">
                    <span class="inline-flex items-center rounded-full bg-primary px-2.5 py-0.5 text-xs font-medium text-primary-foreground">"SSR"</span>
                    <span class="inline-flex items-center rounded-full bg-secondary px-2.5 py-0.5 text-xs font-medium text-secondary-foreground">"Tailwind"</span>
                    <span class="inline-flex items-center rounded-full border border-border px-2.5 py-0.5 text-xs font-medium text-foreground">"shadcn"</span>
                </div>
            </div>
        </footer>
    }
}
