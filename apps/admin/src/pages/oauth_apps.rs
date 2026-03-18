use crate::entities::oauth_app::model::OAuthApp;
use crate::features::oauth_apps::create_app::{CreateAppForm, CreateOAuthAppResult};
use crate::features::oauth_apps::revoke_app::RevokeAppDialog;
use crate::features::oauth_apps::rotate_secret::RotateSecretDialog;
use crate::shared::ui::{ui_button as UiButton, ui_success_message as UiSuccessMessage};
use crate::widgets::oauth_apps_list::OAuthAppsList;
use leptos::prelude::*;

#[derive(Clone, PartialEq)]
enum ModalState {
    None,
    CreateApp,
    RotateSecret(OAuthApp),
    RevokeApp(OAuthApp),
    SecretRevealed(String), // The raw secret string
}

#[component]
pub fn OAuthAppsPage() -> impl IntoView {
    // In a real implementation we would fetch these from the API
    let (apps, set_apps) = signal(Vec::<OAuthApp>::new());
    let (modal_state, set_modal_state) = signal(ModalState::None);

    let on_rotate = Callback::new(move |app| set_modal_state.set(ModalState::RotateSecret(app)));
    let on_revoke = Callback::new(move |app| set_modal_state.set(ModalState::RevokeApp(app)));

    let close_modal = move || set_modal_state.set(ModalState::None);

    // MOCK initial fetch
    Effect::new(move |_| {
        set_apps.set(vec![]);
    });

    view! {
        <div class="space-y-6">
            <div class="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4">
                <div>
                    <h2 class="text-2xl font-bold tracking-tight">"OAuth App Connections"</h2>
                    <p class="text-muted-foreground">
                        "Manage third-party applications, API clients, and external integrations."
                    </p>
                </div>
                <UiButton on_click=Box::new(move || set_modal_state.set(ModalState::CreateApp))>
                    "Create New App"
                </UiButton>
            </div>

            <OAuthAppsList
                apps=apps.get()
                on_rotate_secret=on_rotate
                on_revoke_app=on_revoke
            />

            // Simple Dialog Overlay
            <Show when=move || modal_state.get() != ModalState::None>
                <div class="fixed inset-0 z-50 bg-black/50 backdrop-blur-sm flex items-center justify-center">
                    <div class="bg-background rounded-lg border shadow-lg w-full max-w-md p-6">
                        {move || match modal_state.get() {
                            ModalState::CreateApp => {
                                let close = close_modal;
                                view! {
                                    <CreateAppForm
                                        on_success=move |res: CreateOAuthAppResult| {
                                            set_apps.update(|a| a.push(res.app.clone()));
                                            set_modal_state.set(ModalState::SecretRevealed(res.client_secret));
                                        }
                                        on_cancel=move || close()
                                    />
                                }.into_any()
                            },
                            ModalState::RotateSecret(app) => {
                                let close = close_modal;
                                view! {
                                    <RotateSecretDialog
                                        app=app
                                        on_success=move |new_secret| {
                                            set_modal_state.set(ModalState::SecretRevealed(new_secret));
                                            // Real app would invalidate/update list
                                        }
                                        on_cancel=move || close()
                                    />
                                }.into_any()
                            },
                            ModalState::RevokeApp(app) => {
                                let close_for_success = close_modal;
                                let close_for_cancel = close_modal;
                                view! {
                                    <RevokeAppDialog
                                        app=app
                                        on_success=move || {
                                            // Mock remove
                                            // set_apps.update(|a| a.retain(|x| x.id != app.id));
                                            close_for_success();
                                        }
                                        on_cancel=close_for_cancel
                                    />
                                }.into_any()
                            },
                            ModalState::SecretRevealed(secret) => {
                                let close = close_modal;
                                view! {
                                    <div class="space-y-4">
                                        <h3 class="text-lg font-medium text-green-600">"Success!"</h3>
                                        <p class="text-sm">"Your new Client Secret has been generated."</p>

                                        <div class="p-3 bg-slate-100 rounded border font-mono text-sm break-all">
                                            {secret}
                                        </div>
                                        <UiSuccessMessage message="Store this secret safely. You will not be able to see it again." />

                                        <UiButton class="w-full" on_click=Box::new(close)>
                                            "I have saved it"
                                        </UiButton>
                                    </div>
                                }.into_any()
                            },
                            ModalState::None => view! { <div></div> }.into_any(),
                        }}
                    </div>
                </div>
            </Show>
        </div>
    }
}
