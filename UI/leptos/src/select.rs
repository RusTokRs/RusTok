use leptos::prelude::*;

use crate::types::Size;

#[derive(Clone, Debug)]
pub struct SelectOption {
    pub value: String,
    pub label: String,
    pub disabled: bool,
}

impl SelectOption {
    pub fn new(value: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
            disabled: false,
        }
    }
}

#[component]
pub fn Select(
    #[prop(default = Size::Md)] size: Size,
    #[prop(default = false)] disabled: bool,
    #[prop(default = false)] invalid: bool,
    options: Vec<SelectOption>,
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
        "border-destructive focus:ring-destructive"
    } else {
        "border-input focus:ring-ring"
    };

    view! {
        <select
            id=id
            class=format!(
                "flex w-full rounded-md border bg-background text-foreground shadow-sm \
                 focus:outline-none focus:ring-1 \
                 disabled:cursor-not-allowed disabled:opacity-50 {} {} {}",
                size_cls, state_cls, class
            )
            disabled=disabled
            aria-invalid=invalid
            name=name
            on:change=move |ev| {
                if let Some(set) = set_value {
                    set.set(event_target_value(&ev));
                }
            }
        >
            {(!placeholder.is_empty()).then(|| view! {
                <option value="" disabled=true selected=move || value.map(|v| v.get()).unwrap_or_default().is_empty()>
                    {placeholder.clone()}
                </option>
            })}
            {options.into_iter().map(|opt| {
                let opt_value = opt.value;
                let opt_val_cmp = opt_value.clone();
                view! {
                    <option
                        value=opt_value
                        disabled=opt.disabled
                        selected=move || value.map(|v| v.get()).unwrap_or_default() == opt_val_cmp
                    >
                        {opt.label}
                    </option>
                }
            }).collect_view()}
        </select>
    }
}
