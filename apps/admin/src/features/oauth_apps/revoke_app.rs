use crate::entities::oauth_app::model::OAuthApp;
use crate::shared::ui::ui_button as UiButton;
use leptos::prelude::*;

#[component]
pub fn RevokeAppDialog(
    app: OAuthApp,
    on_success: impl Fn() + Send + Sync + 'static + Clone,
    on_cancel: impl Fn() + 'static + Clone,
) -> impl IntoView {
    let name = app.name.clone();

    let revoke_action = Action::new(move |_: &()| {
        let on_success = on_success.clone();
        async move {
            // MOCK: GraphQL revoke logic
            on_success();
        }
    });

    view! {
        <div class="space-y-4">
            <h3 class="text-lg font-medium text-red-600">"Revoke OAuth Application"</h3>
            <p class="text-sm text-slate-500">
                "Are you absolutely sure you want to revoke access for "<span class="font-semibold">{name}</span>"?"
                <br/>
                "This action cannot be undone. All active tokens will be invalidated immediately, and the application will be disconnected from all users."
            </p>

            <div class="flex items-center gap-2 pt-4">
                <UiButton
                    variant=crate::shared::ui::ButtonVariant::Destructive
                    on_click=Box::new(move || { revoke_action.dispatch(()); })
                >
                    "Revoke Application"
                </UiButton>
                <UiButton
                    variant=crate::shared::ui::ButtonVariant::Outline
                    on_click=Box::new(on_cancel)
                >
                    "Cancel"
                </UiButton>
            </div>
        </div>
    }
}
