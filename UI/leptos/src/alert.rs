use leptos::prelude::*;

use crate::types::AlertVariant;

/// Alert banner with semantic variants.
///
/// Renders an accessible `role="alert"` container styled to match the
/// platform design system. Accepts an optional `title` for a bold heading
/// and `children` for the description body.
///
/// # Example
/// ```rust
/// use leptos::prelude::*;
/// use iu_leptos::{Alert, AlertVariant};
///
/// let _ = view! {
///     <Alert variant=AlertVariant::Warning>
///         "The Iggy module is not enabled."
///     </Alert>
///
///     <Alert variant=AlertVariant::Destructive title="Error".to_string()>
///         "Failed to save settings."
///     </Alert>
/// };
/// ```
#[component]
pub fn Alert(
    #[prop(default = AlertVariant::Default)] variant: AlertVariant,
    #[prop(optional, into)] title: Option<String>,
    #[prop(optional, into)] class: String,
    children: Children,
) -> impl IntoView {
    let variant_cls = match variant {
        AlertVariant::Default => "border-border bg-card text-card-foreground",
        AlertVariant::Info => {
            "border-blue-200 bg-blue-50 text-blue-800 \
             dark:border-blue-800 dark:bg-blue-950 dark:text-blue-200"
        }
        AlertVariant::Warning => {
            "border-amber-300 bg-amber-50 text-amber-800 \
             dark:border-amber-700 dark:bg-amber-950 dark:text-amber-200"
        }
        AlertVariant::Destructive => "border-destructive/30 bg-destructive/10 text-destructive",
        AlertVariant::Success => {
            "border-emerald-200 bg-emerald-50 text-emerald-800 \
             dark:border-emerald-800 dark:bg-emerald-950 dark:text-emerald-200"
        }
    };

    view! {
        <div
            role="alert"
            class=format!(
                "relative w-full rounded-lg border px-4 py-3 text-sm {} {}",
                variant_cls, class
            )
        >
            {title.map(|t| view! {
                <p class="mb-1 font-medium leading-none tracking-tight">{t}</p>
            })}
            <div class="[&_p]:leading-relaxed">
                {children()}
            </div>
        </div>
    }
}
