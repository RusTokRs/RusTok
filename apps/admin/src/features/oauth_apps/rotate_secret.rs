use crate::entities::oauth_app::model::OAuthApp;
use crate::shared::ui::{ui_button as UiButton, ui_input as UiInput, ui_success_message as UiSuccessMessage};
use leptos::prelude::*;

#[component]
pub fn RotateSecretDialog(
    app: OAuthApp,
    on_success: impl Fn(String) + 'static + Clone,
    on_cancel: impl Fn() + 'static + Clone,
) -> impl IntoView {
    let name = app.name.clone();

    let rotate_action = create_action(move |_: &()| {
        let on_success = on_success.clone();
        async move {
            // MOCK: GraphQL rotation logic
            let new_secret = format!(
                "sk_live_{}",
                uuid::Uuid::new_v4().to_string().replace("-", "")
            );
            on_success(new_secret);
        }
    });

    view! {
        <div class="space-y-4">
            <h3 class="text-lg font-medium">"Rotate Client Secret"</h3>
            <p class="text-sm text-slate-500">
                "Are you sure you want to rotate the secret for "<span class="font-semibold">{name}</span>"?"
                <br/>
                "The old secret will immediately stop working and all active sessions/tokens might be invalidated or require the new secret to refresh."
            </p>

            <div class="flex items-center gap-2 pt-4">
                <UiButton
                    variant=crate::shared::ui::ButtonVariant::Destructive
                    on_click=Some(Box::new(move || rotate_action.dispatch(())))
                >
                    "Yes, Rotate Secret"
                </UiButton>
                <UiButton
                    variant=crate::shared::ui::ButtonVariant::Outline
                    on_click=Some(Box::new(move || on_cancel()))
                >
                    "Cancel"
                </UiButton>
            </div>
        </div>
    }
}
