use leptos::prelude::*;

#[component]
pub fn PageHeader(
    #[prop(into)] title: String,
    #[prop(optional)] subtitle: Option<String>,
    #[prop(optional)] eyebrow: Option<String>,
    #[prop(optional)] actions: Option<AnyView>,
    #[prop(optional)] breadcrumbs: Option<Vec<(String, String)>>,
) -> impl IntoView {
    let actions_view = actions.map(|actions| {
        view! {
            <div class="flex flex-wrap items-center gap-3">
                {actions}
            </div>
        }
    });

    view! {
        <header class="mb-8 flex flex-col gap-4 md:flex-row md:items-start md:justify-between">
            <div>
                {eyebrow.map(|text| {
                    view! {
                        <span class="mb-2 inline-flex items-center rounded-full bg-secondary px-3 py-1 text-xs font-semibold text-secondary-foreground">
                            {text}
                        </span>
                    }
                })}

                <h1 class="text-2xl font-semibold text-foreground">{title}</h1>

                {subtitle.map(|text| {
                    view! { <p class="mt-2 text-sm text-muted-foreground">{text}</p> }
                })}

                {breadcrumbs.map(|crumbs| {
                    view! {
                        <div class="mt-4 flex items-center gap-2 text-sm text-muted-foreground">
                            {crumbs
                                .into_iter()
                                .enumerate()
                                .map(|(index, (label, href))| {
                                    view! {
                                        {(index > 0).then(|| view! {
                                            <span class="text-border">"/"</span>
                                        })}
                                        <a href=href class="transition-colors hover:text-foreground">
                                            {label}
                                        </a>
                                    }
                                })
                                .collect_view()}
                        </div>
                    }
                })}
            </div>
            {actions_view}
        </header>
    }
}
