use crate::entities::oauth_app::model::OAuthApp;
use crate::features::oauth_apps::transport;
use crate::shared::ui::Button;
use leptos::prelude::*;
use leptos::task::spawn_local;

#[component]
pub fn RevokeAppDialog(
    token: Option<String>,
    tenant: Option<String>,
    app: OAuthApp,
    on_success: impl Fn() + Send + Sync + 'static + Clone,
    on_cancel: impl Fn() + Send + Sync + 'static + Clone,
) -> impl IntoView {
    let name = app.name.clone();
    let (submitting, set_submitting) = signal(false);
    let (error, set_error) = signal(None::<String>);

    let revoke = move || {
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
            let result = transport::revoke_oauth_app(Some(token_value), tenant_value, app_id).await;

            set_submitting.set(false);
            match result {
                Ok(_) => on_success(),
                Err(err) => set_error.set(Some(err.to_string())),
            }
        });
    };

    view! {
        <div class="space-y-4">
            <h3 class="text-lg font-medium text-red-600">"Revoke OAuth Application"</h3>
            <p class="text-sm text-slate-500">
                "Revoke access for "<span class="font-semibold">{name}</span>" and invalidate all active tokens."
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
                    on_click=Callback::new(move |_| revoke())
                >
                    {move || if submitting.get() { "Revoking..." } else { "Revoke Application" }}
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
