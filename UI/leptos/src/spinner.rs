use leptos::prelude::*;

use crate::types::Size;

#[component]
pub fn Spinner(
    #[prop(default = Size::Md)] size: Size,
    #[prop(optional, into)] class: String,
) -> impl IntoView {
    let size_cls = match size {
        Size::Sm => "h-4 w-4 border-2",
        Size::Md => "h-6 w-6 border-2",
        Size::Lg | Size::Icon => "h-8 w-8 border-[3px]",
    };

    view! {
        <span
            role="status"
            aria-label="Loading"
            class=format!(
                "inline-block rounded-full border-primary border-t-transparent animate-spin {} {}",
                size_cls, class
            )
        />
    }
}
