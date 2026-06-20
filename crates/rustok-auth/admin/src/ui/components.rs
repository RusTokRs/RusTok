use leptos::prelude::*;

#[component]
pub fn Button(
    #[prop(into)] on_click: Callback<web_sys::MouseEvent>,
    #[prop(optional)] children: Option<Children>,
    #[prop(optional, into)] class: String,
    #[prop(default = Signal::derive(|| false))] disabled: Signal<bool>,
) -> impl IntoView {
    let base_class = "inline-flex h-9 shrink-0 items-center justify-center gap-2 whitespace-nowrap rounded-md bg-primary px-4 py-2 text-sm font-medium text-primary-foreground shadow-xs outline-none transition-all hover:bg-primary/90 focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50 disabled:pointer-events-none disabled:opacity-50";
    let merged_class = move || {
        if class.is_empty() {
            base_class.to_string()
        } else {
            format!("{base_class} {class}")
        }
    };

    view! {
        <button
            class=merged_class
            on:click=move |ev| on_click.run(ev)
            disabled=move || disabled.get()
        >
            {children.map(|c| c())}
        </button>
    }
}

#[component]
pub fn Input(
    #[prop(into)] value: Signal<String>,
    #[prop(into)] set_value: WriteSignal<String>,
    #[prop(into)] placeholder: TextProp,
    #[prop(default = "text")] type_: &'static str,
    #[prop(default = String::new().into(), into)] label: TextProp,
) -> impl IntoView {
    view! {
        <div class="flex flex-col gap-2">
            {move || {
                let label_value = label.get();
                (!label_value.is_empty()).then(|| {
                    view! {
                        <label class="text-sm font-medium leading-none peer-disabled:cursor-not-allowed peer-disabled:opacity-70">
                            {label_value}
                        </label>
                    }
                })
            }}
            <input
                type=type_
                placeholder=placeholder
                prop:value=value
                on:input=move |ev| set_value.set(event_target_value(&ev))
                class="flex h-9 w-full min-w-0 rounded-md border border-input bg-background px-3 py-1 text-sm shadow-xs outline-none transition-[color,box-shadow] placeholder:text-muted-foreground focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50 disabled:cursor-not-allowed disabled:opacity-50"
            />
        </div>
    }
}

#[component]
pub fn PageHeader(
    #[prop(into)] title: TextProp,
    #[prop(optional, into)] subtitle: Option<TextProp>,
    #[prop(optional, into)] eyebrow: Option<TextProp>,
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
        <header class="mb-4 flex flex-col gap-4 md:flex-row md:items-start md:justify-between">
            <div>
                {eyebrow.map(|text| {
                    view! {
                        <span class="mb-2 inline-flex items-center text-xs font-medium uppercase tracking-[0.12em] text-muted-foreground">
                            {move || text.get()}
                        </span>
                    }
                })}

                <h1 class="text-3xl font-bold tracking-tight text-foreground">{move || title.get()}</h1>

                {subtitle.map(|text| {
                    view! { <p class="text-sm text-muted-foreground">{move || text.get()}</p> }
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
