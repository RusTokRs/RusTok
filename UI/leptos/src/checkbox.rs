use leptos::prelude::*;

fn checkbox_checked(ev: &leptos::ev::Event) -> bool {
    use leptos::wasm_bindgen::JsCast;
    ev.target()
        .and_then(|t| t.dyn_into::<leptos::web_sys::HtmlInputElement>().ok())
        .map(|el| el.checked())
        .unwrap_or(false)
}

#[component]
pub fn Checkbox(
    #[prop(optional)] checked: Option<ReadSignal<bool>>,
    #[prop(optional)] set_checked: Option<WriteSignal<bool>>,
    #[prop(default = false)] indeterminate: bool,
    #[prop(default = false)] disabled: bool,
    #[prop(optional, into)] class: String,
    #[prop(optional, into)] id: String,
    #[prop(optional, into)] name: String,
) -> impl IntoView {
    view! {
        <input
            type="checkbox"
            id=id
            name=name
            disabled=disabled
            class=format!(
                "h-4 w-4 rounded border border-primary text-primary shadow \
                 focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring \
                 disabled:cursor-not-allowed disabled:opacity-50 {}",
                class
            )
            prop:checked=move || checked.map(|c| c.get()).unwrap_or(false)
            prop:indeterminate=indeterminate
            on:change=move |ev| {
                if let Some(set) = set_checked {
                    set.set(checkbox_checked(&ev));
                }
            }
        />
    }
}
