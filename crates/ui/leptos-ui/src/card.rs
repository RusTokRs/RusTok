use leptos::children::Children;
use leptos::prelude::*;

#[component]
pub fn Card(#[prop(optional, into)] class: String, children: Children) -> impl IntoView {
    view! {
        <div class=format!(
            "flex flex-col gap-6 rounded-xl border bg-card py-6 text-card-foreground shadow-sm {}",
            class
        )>
            {children()}
        </div>
    }
}

#[component]
pub fn CardHeader(#[prop(optional, into)] class: String, children: Children) -> impl IntoView {
    view! {
        <div class=format!(
            "grid auto-rows-min grid-rows-[auto_auto] items-start gap-1.5 px-6 {}",
            class
        )>
            {children()}
        </div>
    }
}

#[component]
pub fn CardTitle(#[prop(optional, into)] class: String, children: Children) -> impl IntoView {
    view! {
        <div class=format!("font-semibold leading-none {}", class)>
            {children()}
        </div>
    }
}

#[component]
pub fn CardDescription(#[prop(optional, into)] class: String, children: Children) -> impl IntoView {
    view! {
        <div class=format!("text-sm text-muted-foreground {}", class)>
            {children()}
        </div>
    }
}

#[component]
pub fn CardAction(#[prop(optional, into)] class: String, children: Children) -> impl IntoView {
    view! {
        <div class=format!(
            "col-start-2 row-span-2 row-start-1 self-start justify-self-end {}",
            class
        )>
            {children()}
        </div>
    }
}

#[component]
pub fn CardContent(#[prop(optional, into)] class: String, children: Children) -> impl IntoView {
    view! {
        <div class=format!("px-6 {}", class)>
            {children()}
        </div>
    }
}

#[component]
pub fn CardFooter(#[prop(optional, into)] class: String, children: Children) -> impl IntoView {
    view! {
        <div class=format!("flex items-center px-6 {}", class)>
            {children()}
        </div>
    }
}
