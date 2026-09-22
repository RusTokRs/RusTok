use leptos::prelude::*;

use crate::spinner::Spinner;
use crate::types::{ButtonVariant, Size};

#[component]
pub fn Button(
    #[prop(default = ButtonVariant::Default)] variant: ButtonVariant,
    #[prop(default = Size::Md)] size: Size,
    #[prop(default = false)] disabled: bool,
    #[prop(default = false)] loading: bool,
    #[prop(optional, into)] class: String,
    #[prop(optional)] on_click: Option<Box<dyn Fn() + 'static>>,
    #[prop(default = "button")] r#type: &'static str,
    children: Children,
) -> impl IntoView {
    let size_cls = match size {
        Size::Sm => "h-8 px-3 text-xs gap-1.5",
        Size::Md => "h-9 px-4 py-2 text-sm gap-2",
        Size::Lg => "h-10 px-8 text-sm gap-2",
        Size::Icon => "h-9 w-9",
    };

    let variant_cls = match variant {
        ButtonVariant::Default => "bg-primary text-primary-foreground shadow hover:bg-primary/90",
        ButtonVariant::Destructive => {
            "bg-destructive text-destructive-foreground shadow-sm hover:bg-destructive/90"
        }
        ButtonVariant::Outline => {
            "border border-input bg-background shadow-sm hover:bg-accent hover:text-accent-foreground"
        }
        ButtonVariant::Secondary => {
            "bg-secondary text-secondary-foreground shadow-sm hover:bg-secondary/80"
        }
        ButtonVariant::Ghost => "hover:bg-accent hover:text-accent-foreground",
        ButtonVariant::Link => "text-primary underline-offset-4 hover:underline",
    };

    let full_class = format!(
        "inline-flex items-center justify-center whitespace-nowrap rounded-md font-medium \
         ring-offset-background transition-colors \
         focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 \
         disabled:pointer-events-none disabled:opacity-50 {} {} {}",
        size_cls, variant_cls, class
    );

    let is_disabled = disabled || loading;

    view! {
        <button
            type=r#type
            class=full_class
            disabled=is_disabled
            on:click=move |_| {
                if let Some(ref handler) = on_click {
                    handler();
                }
            }
        >
            {move || {
                if loading {
                    Some(view! { <Spinner size=Size::Sm /> })
                } else {
                    None
                }
            }}
            {children()}
        </button>
    }
}
