use crate::entities::oauth_app::model::OAuthApp;
use crate::features::oauth_apps::transport;
use crate::shared::ui::Button;
use leptos::prelude::*;
use leptos::task::spawn_local;

#[component]
pub fn RotateSecretDialog(
    token: Option<String>,
    tenant: Option<String>,
    app: OAuthApp,
    on_success: impl Fn(String, OAuthApp) + Send + Sync + 'static + Clone,
    on_cancel: impl Fn() + Send + Sync + 'static + Clone,
) -> impl IntoView {
    let name = app.name.clone();
    let (submitting, set_submitting) = signal(false);
    let (error, set_error) = signal(None::<String>);

    let rotate = move || {
        let Some(token_value) = token.clone() else {
            set_error.set(Some("Sign in again to manage app connections.".to_string()));
            return;
        };

        let tenant_value = tenant.clone();
        let on_success = on_success.clone();
        let app_id = app.id;

        set_submitting.set(true);
        set_error.set(None);

        spawn_local(async move {
            let result =
                transport::rotate_oauth_app_secret(Some(token_value), tenant_value, app_id).await;

            set_submitting.set(false);
            match result {
                Ok(result) => on_success(result.client_secret.clone(), result.app),
                Err(err) => set_error.set(Some(err.to_string())),
            }
        });
    };

    view! {
        <div class="space-y-4">
            <h3 class="text-lg font-medium">"Rotate Client Secret"</h3>
            <p class="text-sm text-slate-500">
                "Rotate the secret for "<span class="font-semibold">{name}</span>"."
                <br/>
                "The old secret stops working immediately."
            </p>

            <Show when=move || error.get().is_some()>
                <div class="rounded-md border border-destructive/40 bg-destructive/5 px-3 py-2 text-sm text-destructive">
                    {move || error.get().unwrap_or_default()}
                </div>
            </Show>

            <div class="flex items-center gap-2 pt-2">
                <Button
                    class="bg-destructive text-destructive-foreground hover:bg-destructive/90"
                    disabled=Signal::derive(move || submitting.get())
                    on_click=Callback::new(move |_| rotate())
                >
                    {move || if submitting.get() { "Rotating..." } else { "Rotate Secret" }}
                </Button>
                <Button
                    class="bg-transparent text-foreground shadow-none ring-1 ring-border hover:bg-accent"
                    on_click=Callback::new(move |_| on_cancel())
                >
                    "Cancel"
                </Button>
            </div>
        </div>
    }
}
