use leptos::prelude::*;

use crate::types::Size;

#[component]
pub fn Input(
    #[prop(default = "text")] r#type: &'static str,
    #[prop(default = Size::Md)] size: Size,
    #[prop(default = false)] disabled: bool,
    #[prop(default = false)] invalid: bool,
    #[prop(optional, into)] placeholder: String,
    #[prop(optional)] value: Option<ReadSignal<String>>,
    #[prop(optional)] set_value: Option<WriteSignal<String>>,
    #[prop(optional, into)] class: String,
    #[prop(optional, into)] id: String,
    #[prop(optional, into)] name: String,
) -> impl IntoView {
    let id = (!id.is_empty()).then_some(id);
    let size_cls = match size {
        Size::Sm => "h-8 text-xs px-2",
        Size::Md => "h-9 text-sm px-3 py-1",
        Size::Lg | Size::Icon => "h-10 text-sm px-4 py-2",
    };

    let state_cls = if invalid {
        "border-destructive focus-visible:ring-destructive"
    } else {
        "border-input focus-visible:ring-ring"
    };

    view! {
        <input
            id=id
            type=r#type
            class=format!(
                "flex w-full rounded-md border bg-background text-foreground shadow-sm \
                 transition-colors file:border-0 file:bg-transparent file:text-sm file:font-medium \
                 placeholder:text-muted-foreground \
                 focus-visible:outline-none focus-visible:ring-1 \
                 disabled:cursor-not-allowed disabled:opacity-50 {} {} {}",
                size_cls, state_cls, class
            )
            disabled=disabled
            aria-invalid=invalid
            placeholder=placeholder
            name=name
            prop:value=move || value.map(|v| v.get()).unwrap_or_default()
            on:input=move |ev| {
                if let Some(set) = set_value {
                    set.set(event_target_value(&ev));
                }
            }
        />
    }
}
