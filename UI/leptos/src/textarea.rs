use leptos::prelude::*;

use crate::types::Size;

#[component]
pub fn Textarea(
    #[prop(default = Size::Md)] size: Size,
    #[prop(default = false)] disabled: bool,
    #[prop(default = false)] invalid: bool,
    #[prop(default = 3u32)] rows: u32,
    #[prop(optional, into)] placeholder: String,
    #[prop(optional)] value: Option<ReadSignal<String>>,
    #[prop(optional)] set_value: Option<WriteSignal<String>>,
    #[prop(optional, into)] class: String,
    #[prop(optional, into)] id: String,
    #[prop(optional, into)] name: String,
) -> impl IntoView {
    let id = (!id.is_empty()).then_some(id);
    let size_cls = match size {
        Size::Sm => "text-xs px-2 py-1.5",
        Size::Md => "text-sm px-3 py-2",
        Size::Lg | Size::Icon => "text-sm px-4 py-3",
    };

    let state_cls = if invalid {
        "border-destructive focus-visible:ring-destructive"
    } else {
        "border-input focus-visible:ring-ring"
    };

    view! {
        <textarea
            id=id
            rows=rows
            class=format!(
                "flex w-full rounded-md border bg-background text-foreground shadow-sm \
                 placeholder:text-muted-foreground resize-y \
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
