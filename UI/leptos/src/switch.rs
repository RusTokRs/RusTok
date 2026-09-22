use leptos::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SwitchSize {
    Sm,
    #[default]
    Md,
}

#[component]
pub fn Switch(
    #[prop(optional)] checked: Option<ReadSignal<bool>>,
    #[prop(optional)] set_checked: Option<WriteSignal<bool>>,
    #[prop(default = false)] disabled: bool,
    #[prop(default = SwitchSize::Md)] size: SwitchSize,
    #[prop(optional, into)] class: String,
    #[prop(optional, into)] id: String,
) -> impl IntoView {
    let (track_w_h, thumb_size, thumb_checked_x) = match size {
        SwitchSize::Sm => ("w-7 h-4", "h-3 w-3", "translate-x-3"),
        SwitchSize::Md => ("w-11 h-6", "h-5 w-5", "translate-x-5"),
    };

    let is_checked = move || checked.map(|c| c.get()).unwrap_or(false);

    view! {
        <button
            id=id
            type="button"
            role="switch"
            aria-checked=move || is_checked().to_string()
            disabled=disabled
            class=move || format!(
                "peer inline-flex shrink-0 cursor-pointer items-center \
                 rounded-full border-2 border-transparent shadow-sm transition-colors \
                 focus-visible:outline-none focus-visible:ring-2 \
                 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-background \
                 disabled:cursor-not-allowed disabled:opacity-50 {} {} {}",
                if is_checked() { "bg-primary" } else { "bg-input" },
                track_w_h,
                class
            )
            on:click=move |_| {
                if !disabled
                    && let Some(set) = set_checked
                {
                    let current = checked.map(|c| c.get()).unwrap_or(false);
                    set.set(!current);
                }
            }
        >
            <span
                class=move || format!(
                    "pointer-events-none block rounded-full bg-background shadow-lg \
                     ring-0 transition-transform {} {}",
                    if is_checked() { thumb_checked_x } else { "translate-x-0" },
                    thumb_size
                )
            />
        </button>
    }
}
