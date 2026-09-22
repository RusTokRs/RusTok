use leptos::prelude::*;

use super::model::{UserRole, UserStatus};

#[component]
pub fn user_avatar(
    #[prop(optional, into)] name: Option<String>,
    #[prop(optional, into)] class: String,
) -> impl IntoView {
    let initials = name
        .as_deref()
        .and_then(|n| n.chars().next())
        .map(|c| c.to_uppercase().to_string())
        .unwrap_or_else(|| "U".to_string());

    view! {
        <div class=format!(
            "flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-primary/10 {}",
            class
        )>
            <span class="text-primary text-sm font-semibold">{initials}</span>
        </div>
    }
}

#[component]
pub fn user_role_badge(role: UserRole) -> impl IntoView {
    let (label, class) = match role {
        UserRole::SuperAdmin => (
            "Super Admin",
            "bg-violet-100 text-violet-700 dark:bg-violet-900/30 dark:text-violet-400",
        ),
        UserRole::Admin => ("Admin", "bg-primary/10 text-primary"),
        UserRole::Manager => ("Manager", "bg-secondary text-secondary-foreground"),
        UserRole::Customer => ("Customer", "bg-muted text-muted-foreground"),
        UserRole::Unknown => ("Unknown", "bg-muted text-muted-foreground"),
    };

    view! {
        <span class=format!(
            "inline-flex items-center rounded-full border px-2 py-0.5 text-xs font-semibold {}",
            class
        )>
            {label}
        </span>
    }
}

#[component]
pub fn user_status_badge(status: UserStatus) -> impl IntoView {
    let (label, class) = match status {
        UserStatus::Active => (
            "Active",
            "bg-emerald-100 text-emerald-700 dark:bg-emerald-900/30 dark:text-emerald-400",
        ),
        UserStatus::Inactive => ("Inactive", "bg-muted text-muted-foreground"),
        UserStatus::Suspended => ("Suspended", "bg-destructive/10 text-destructive"),
        UserStatus::Unknown => ("Unknown", "bg-muted text-muted-foreground"),
    };

    view! {
        <span class=format!(
            "inline-flex items-center rounded-full border px-2 py-0.5 text-xs font-semibold {}",
            class
        )>
            {label}
        </span>
    }
}
