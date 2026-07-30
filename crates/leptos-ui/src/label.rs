use leptos::children::Children;
use leptos::prelude::*;

#[component]
pub fn Label(
    #[prop(default = false)] required: bool,
    #[prop(optional)] r#for: Option<&'static str>,
    #[prop(optional, into)] class: String,
    children: Children,
) -> impl IntoView {
    view! {
        <label
            for=r#for
            class=format!(
                "text-sm font-medium leading-none \
                 peer-disabled:cursor-not-allowed peer-disabled:opacity-70 {}",
                class
            )
        >
            {children()}
            {move || required.then(|| view! { <span class="text-destructive ml-1">"*"</span> })}
        </label>
    }
}
