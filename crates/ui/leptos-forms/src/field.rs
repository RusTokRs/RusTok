use crate::form::FormContext;
use leptos::ev;
use leptos::prelude::*;

#[component]
pub fn Field(
    form: FormContext,
    name: &'static str,
    #[prop(optional)] label: Option<&'static str>,
    #[prop(default = "text")] r#type: &'static str,
    #[prop(optional)] placeholder: Option<&'static str>,
    #[prop(optional)] class: Option<&'static str>,
) -> impl IntoView {
    // Register field on mount
    {
        let form = form.clone();
        Effect::new(move |_| {
            form.register(name);
        });
    }

    let value = {
        let form = form.clone();
        Memo::new(move |_| form.get_value(name))
    };
    let error = {
        let form = form.clone();
        Memo::new(move |_| form.get_field_error(name))
    };

    let on_input = {
        let form = form.clone();
        move |ev: ev::Event| {
            let value = event_target_value(&ev);
            form.set_value(name, value);
        }
    };

    let on_blur = {
        let form = form.clone();
        move |_| {
            let _ = form.validate_field(name);
        }
    };

    let input_classes = if error.get().is_some() {
        "block w-full rounded-md border border-red-300 px-3 py-2 text-sm focus:border-red-500 focus:ring-red-500"
    } else {
        "block w-full rounded-md border border-gray-300 px-3 py-2 text-sm focus:border-blue-500 focus:ring-blue-500"
    };

    let container_class = class.unwrap_or("space-y-1");

    view! {
        <div class=container_class>
            {move || label.map(|l| view! {
                <label
                    for=name
                    class="block text-sm font-medium text-gray-700"
                >
                    {l}
                </label>
            })}

            <input
                type=r#type
                id=name
                name=name
                placeholder=placeholder.unwrap_or("")
                class=input_classes
                value=move || value.get()
                on:input=on_input
                on:blur=on_blur
            />

            {move || error.get().map(|err| view! {
                <p class="text-xs text-red-600">{err}</p>
            })}
        </div>
    }
}
