use crate::entities::oauth_app::model::AppType;
use leptos::*;
use rustok_ui_leptos::{Badge, BadgeVariant};

#[component]
pub fn AppTypeBadge(app_type: AppType) -> impl IntoView {
    let (variant, label) = match app_type {
        AppType::Embedded => (BadgeVariant::Secondary, "Embedded"),
        AppType::FirstParty => (BadgeVariant::Default, "First Party"),
        AppType::Mobile => (BadgeVariant::Default, "Mobile"),
        AppType::Service => (BadgeVariant::Outline, "Service"),
        AppType::ThirdParty => (BadgeVariant::Warning, "Third Party"),
    };

    view! {
        <Badge variant=variant class="whitespace-nowrap">
            {label}
        </Badge>
    }
}
