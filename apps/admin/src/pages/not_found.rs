use leptos::prelude::*;
use crate::shared::i18n::translate;
use crate::shared::ui::Button;
use leptos_router::components::A;

#[component]
pub fn NotFound() -> impl IntoView {
    view! {
        <section class="flex min-h-screen items-center justify-center bg-background">
            <div class="grid gap-4 rounded-xl border border-border bg-card p-10 text-center shadow-md">
                <h1 class="text-5xl font-semibold text-card-foreground">"404"</h1>
                <p class="text-muted-foreground">{move || translate("app.not_found.text")}</p>
                <div class="flex justify-center">
                    <A href="/dashboard">
                        <Button on_click=move |_| {}>
                            {move || translate("app.not_found.back")}
                        </Button>
                    </A>
                </div>
            </div>
        </section>
    }
}
