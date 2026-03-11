use crate::entities::oauth_app::model::OAuthApp;
use crate::features::oauth_apps::create_app::{CreateAppForm, CreateOAuthAppResult};
use crate::features::oauth_apps::revoke_app::RevokeAppDialog;
use crate::features::oauth_apps::rotate_secret::RotateSecretDialog;
use crate::shared::ui::{ui_button, ui_success_message};
use crate::widgets::oauth_apps_list::OAuthAppsList;
use leptos::*;

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
    let (apps, set_apps) = create_signal(Vec::<OAuthApp>::new());
    let (modal_state, set_modal_state) = create_signal(ModalState::None);

    let on_rotate = move |app| set_modal_state.set(ModalState::RotateSecret(app));
    let on_revoke = move |app| set_modal_state.set(ModalState::RevokeApp(app));

    let close_modal = move || set_modal_state.set(ModalState::None);

    // MOCK initial fetch
    create_effect(move |_| {
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
                <Button on:click=move |_| set_modal_state.set(ModalState::CreateApp)>
                    "Create New App"
                </Button>
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
                                let close = close_modal.clone();
                                view! {
                                    <CreateAppForm
                                        on_success=move |res: CreateOAuthAppResult| {
                                            set_apps.update(|a| a.push(res.app.clone()));
                                            set_modal_state.set(ModalState::SecretRevealed(res.client_secret));
                                        }
                                        on_cancel=move || close()
                                    />
                                }.into_view()
                            },
                            ModalState::RotateSecret(app) => {
                                let close = close_modal.clone();
                                view! {
                                    <RotateSecretDialog
                                        app=app
                                        on_success=move |new_secret| {
                                            set_modal_state.set(ModalState::SecretRevealed(new_secret));
                                            // Real app would invalidate/update list
                                        }
                                        on_cancel=move || close()
                                    />
                                }.into_view()
                            },
                            ModalState::RevokeApp(app) => {
                                let close = close_modal.clone();
                                view! {
                                    <RevokeAppDialog
                                        app=app
                                        on_success=move || {
                                            // Mock remove
                                            // set_apps.update(|a| a.retain(|x| x.id != app.id));
                                            close();
                                        }
                                        on_cancel=move || close()
                                    />
                                }.into_view()
                            },
                            ModalState::SecretRevealed(secret) => {
                                let close = close_modal.clone();
                                view! {
                                    <div class="space-y-4">
                                        <h3 class="text-lg font-medium text-green-600">"Success!"</h3>
                                        <p class="text-sm">"Your new Client Secret has been generated."</p>

                                        <div class="p-3 bg-slate-100 rounded border font-mono text-sm break-all">
                                            {secret}
                                        </div>
                                        <SuccessMessage message="Store this secret safely. You will not be able to see it again." />

                                        <Button class="w-full" on:click=move |_| close()>
                                            "I have saved it"
                                        </Button>
                                    </div>
                                }.into_view()
                            },
                            ModalState::None => view! { <div></div> }.into_view(),
                        }}
                    </div>
                </div>
            </Show>
        </div>
    }
}
