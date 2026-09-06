use leptos::prelude::*;

#[component]
pub fn Separator(
    #[prop(default = "horizontal")] orientation: &'static str,
    #[prop(optional, into)] class: String,
) -> impl IntoView {
    let orientation_classes = match orientation {
        "vertical" => "h-full w-px",
        _ => "w-full h-px",
    };

    view! {
        <div
            class=format!(
                "shrink-0 bg-border {} {}",
                orientation_classes, class
            )
            role="separator"
        />
    }
}
