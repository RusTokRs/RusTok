use leptos::prelude::*;

use crate::types::{BadgeVariant, Size};

#[component]
pub fn Badge(
    #[prop(default = BadgeVariant::Default)] variant: BadgeVariant,
    #[prop(default = Size::Md)] size: Size,
    #[prop(default = false)] dismissible: bool,
    #[prop(optional)] on_dismiss: Option<Callback<()>>,
    #[prop(optional, into)] class: String,
    children: Children,
) -> impl IntoView {
    let size_cls = match size {
        Size::Sm => "px-1.5 py-0 text-[10px]",
        _ => "px-2.5 py-0.5 text-xs",
    };

    let variant_cls = match variant {
        BadgeVariant::Default => {
            "border-transparent bg-primary text-primary-foreground shadow hover:bg-primary/80"
        }
        BadgeVariant::Secondary => {
            "border-transparent bg-secondary text-secondary-foreground hover:bg-secondary/80"
        }
        BadgeVariant::Destructive => {
            "border-transparent bg-destructive text-destructive-foreground shadow hover:bg-destructive/80"
        }
        BadgeVariant::Outline => "text-foreground",
        BadgeVariant::Success => {
            "border-transparent bg-emerald-100 text-emerald-700 dark:bg-emerald-900/30 dark:text-emerald-400"
        }
        BadgeVariant::Warning => {
            "border-transparent bg-amber-100 text-amber-700 dark:bg-amber-900/30 dark:text-amber-400"
        }
    };

    view! {
        <span
            class=format!(
                "inline-flex items-center gap-1 rounded-full border font-semibold \
                 transition-colors focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-2 \
                 {} {} {}",
                size_cls, variant_cls, class
            )
        >
            {children()}
            {move || dismissible.then(|| {
                view! {
                    <button
                        type="button"
                        class="ml-0.5 rounded-full opacity-70 hover:opacity-100 focus:outline-none"
                        aria-label="Dismiss"
                        on:click=move |_| {
                            if let Some(cb) = on_dismiss {
                                cb.run(());
                            }
                        }
                    >
                        "×"
                    </button>
                }
            })}
        </span>
    }
}
