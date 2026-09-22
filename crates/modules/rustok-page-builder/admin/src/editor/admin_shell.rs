use leptos::prelude::*;

#[component]
pub fn AdminShell(
    title: String,
    #[prop(optional)] subtitle: Option<String>,
    children: Children,
) -> impl IntoView {
    view! {
        <section class="rustok-page-builder-admin" data-fly-presentation="full">
            <header class="rustok-page-builder-admin__header">
                <div>
                    <p class="rustok-page-builder-admin__eyebrow">"Page Builder"</p>
                    <h1>{title}</h1>
                    {subtitle.map(|subtitle| view! {
                        <p class="rustok-page-builder-admin__subtitle">{subtitle}</p>
                    })}
                </div>
            </header>
            <div class="rustok-page-builder-admin__body">
                {children()}
            </div>
        </section>
    }
}
