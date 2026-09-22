use crate::entities::oauth_app::model::OAuthApp;
use crate::features::oauth_apps::transport::{self, UpdateOAuthAppInput};
use crate::shared::ui::{Button, Input};
use leptos::prelude::*;
use leptos::task::spawn_local;

fn lines_to_vec(value: &str) -> Vec<String> {
    value
        .lines()
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(ToString::to_string)
        .collect()
}

#[component]
pub fn EditAppForm(
    token: Option<String>,
    tenant: Option<String>,
    app: OAuthApp,
    on_success: impl Fn(OAuthApp) + Send + Sync + 'static + Clone,
    on_cancel: impl Fn() + Send + Sync + 'static + Clone,
) -> impl IntoView {
    let (name, set_name) = signal(app.name.clone());
    let (description, set_description) = signal(app.description.clone().unwrap_or_default());
    let (icon_url, set_icon_url) = signal(app.icon_url.clone().unwrap_or_default());
    let (redirect_uris, set_redirect_uris) = signal(app.redirect_uris.join("\n"));
    let (scopes, set_scopes) = signal(app.scopes.join("\n"));
    let (grant_types, set_grant_types) = signal(app.grant_types.join("\n"));
    let (submitting, set_submitting) = signal(false);
    let (error, set_error) = signal(None::<String>);

    let submit = move || {
        let Some(token_value) = token.clone() else {
            set_error.set(Some("Sign in again to manage app connections.".to_string()));
            return;
        };

        let tenant_value = tenant.clone();
        let on_success = on_success.clone();
        let input = UpdateOAuthAppInput {
            name: name.get_untracked().trim().to_string(),
            description: (!description.get_untracked().trim().is_empty())
                .then(|| description.get_untracked().trim().to_string()),
            icon_url: (!icon_url.get_untracked().trim().is_empty())
                .then(|| icon_url.get_untracked().trim().to_string()),
            redirect_uris: lines_to_vec(&redirect_uris.get_untracked()),
            scopes: lines_to_vec(&scopes.get_untracked()),
            grant_types: lines_to_vec(&grant_types.get_untracked()),
        };
        let app_id = app.id;

        set_submitting.set(true);
        set_error.set(None);

        spawn_local(async move {
            let result =
                transport::update_oauth_app(Some(token_value), tenant_value, app_id, input).await;

            set_submitting.set(false);
            match result {
                Ok(app) => on_success(app),
                Err(err) => set_error.set(Some(err.to_string())),
            }
        });
    };

    view! {
        <div class="space-y-4">
            <h3 class="text-lg font-medium">"Edit App Connection"</h3>

            <Input value=name set_value=set_name placeholder="My Integration" label="App Name" />
            <Input value=icon_url set_value=set_icon_url placeholder="https://example.com/icon.png" label="Icon URL" />

            <div class="flex flex-col gap-2">
                <label class="text-sm font-medium">"Description"</label>
                <textarea
                    class="min-h-24 rounded-md border border-input bg-background px-3 py-2 text-sm"
                    prop:value=description
                    on:input=move |ev| set_description.set(event_target_value(&ev))
                />
            </div>

            <div class="flex flex-col gap-2">
                <label class="text-sm font-medium">"Redirect URIs"</label>
                <textarea
                    class="min-h-24 rounded-md border border-input bg-background px-3 py-2 text-sm font-mono"
                    prop:value=redirect_uris
                    on:input=move |ev| set_redirect_uris.set(event_target_value(&ev))
                />
            </div>

            <div class="flex flex-col gap-2">
                <label class="text-sm font-medium">"Scopes"</label>
                <textarea
                    class="min-h-24 rounded-md border border-input bg-background px-3 py-2 text-sm font-mono"
                    prop:value=scopes
                    on:input=move |ev| set_scopes.set(event_target_value(&ev))
                />
            </div>

            <div class="flex flex-col gap-2">
                <label class="text-sm font-medium">"Grant Types"</label>
                <textarea
                    class="min-h-20 rounded-md border border-input bg-background px-3 py-2 text-sm font-mono"
                    prop:value=grant_types
                    on:input=move |ev| set_grant_types.set(event_target_value(&ev))
                />
            </div>

            <Show when=move || error.get().is_some()>
                <div class="rounded-md border border-destructive/40 bg-destructive/5 px-3 py-2 text-sm text-destructive">
                    {move || error.get().unwrap_or_default()}
                </div>
            </Show>

            <div class="flex items-center gap-2 pt-2">
                <Button
                    disabled=Signal::derive(move || submitting.get())
                    on_click=Callback::new(move |_| submit())
                >
                    {move || if submitting.get() { "Saving..." } else { "Save Changes" }}
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
