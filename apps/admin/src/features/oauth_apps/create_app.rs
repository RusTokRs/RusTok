use crate::features::oauth_apps::transport::{self, CreateOAuthAppInput, CreateOAuthAppResult};
use crate::shared::ui::{Button, Input};
use leptos::prelude::*;
use leptos::task::spawn_local;

pub type CreateResult = CreateOAuthAppResult;

fn lines_to_vec(value: &str) -> Vec<String> {
    value
        .lines()
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn default_redirects(app_type: &str) -> &'static str {
    match app_type {
        "Mobile" => "myapp://auth/callback",
        "Service" => "",
        _ => "http://localhost:3000/auth/callback",
    }
}

fn default_grants(app_type: &str) -> &'static str {
    match app_type {
        "Service" => "client_credentials",
        _ => "authorization_code\nrefresh_token",
    }
}

#[component]
pub fn CreateAppForm(
    token: Option<String>,
    tenant: Option<String>,
    on_success: impl Fn(CreateResult) + Send + Sync + 'static + Clone,
    on_cancel: impl Fn() + Send + Sync + 'static + Clone,
) -> impl IntoView {
    let (name, set_name) = signal(String::new());
    let (slug, set_slug) = signal(String::new());
    let (description, set_description) = signal(String::new());
    let (icon_url, set_icon_url) = signal(String::new());
    let (app_type, set_app_type) = signal("ThirdParty".to_string());
    let (redirect_uris, set_redirect_uris) = signal(default_redirects("ThirdParty").to_string());
    let (scopes, set_scopes) = signal(String::new());
    let (grant_types, set_grant_types) = signal(default_grants("ThirdParty").to_string());
    let (submitting, set_submitting) = signal(false);
    let (error, set_error) = signal(None::<String>);

    let submit = move || {
        let Some(token_value) = token.clone() else {
            set_error.set(Some("Sign in again to manage app connections.".to_string()));
            return;
        };

        let tenant_value = tenant.clone();
        let input = CreateOAuthAppInput {
            name: name.get_untracked().trim().to_string(),
            slug: slug.get_untracked().trim().to_string(),
            description: (!description.get_untracked().trim().is_empty())
                .then(|| description.get_untracked().trim().to_string()),
            icon_url: (!icon_url.get_untracked().trim().is_empty())
                .then(|| icon_url.get_untracked().trim().to_string()),
            app_type: match app_type.get_untracked().as_str() {
                "Mobile" => crate::entities::oauth_app::model::AppType::Mobile,
                "Service" => crate::entities::oauth_app::model::AppType::Service,
                _ => crate::entities::oauth_app::model::AppType::ThirdParty,
            },
            redirect_uris: {
                let values = lines_to_vec(&redirect_uris.get_untracked());
                (!values.is_empty()).then_some(values)
            },
            scopes: lines_to_vec(&scopes.get_untracked()),
            grant_types: lines_to_vec(&grant_types.get_untracked()),
        };
        let on_success = on_success.clone();

        set_submitting.set(true);
        set_error.set(None);

        spawn_local(async move {
            let result = transport::create_oauth_app(Some(token_value), tenant_value, input).await;

            set_submitting.set(false);
            match result {
                Ok(result) => on_success(result),
                Err(err) => set_error.set(Some(err.to_string())),
            }
        });
    };

    view! {
        <div class="space-y-4">
            <h3 class="text-lg font-medium">"Create New Connected App"</h3>

            <Input value=name set_value=set_name placeholder="My Integration" label="App Name" />
            <Input value=slug set_value=set_slug placeholder="com.example.app" label="Slug / Bundle ID" />
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
                <label class="text-sm font-medium">"App Type"</label>
                <select
                    class="h-10 rounded-md border border-input bg-background px-3 py-2 text-sm"
                    prop:value=app_type
                    on:change=move |ev| {
                        let next = event_target_value(&ev);
                        set_app_type.set(next.clone());
                        set_redirect_uris.set(default_redirects(&next).to_string());
                        set_grant_types.set(default_grants(&next).to_string());
                    }
                >
                    <option value="ThirdParty">"Third Party"</option>
                    <option value="Mobile">"Mobile"</option>
                    <option value="Service">"Service"</option>
                </select>
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
                    {move || if submitting.get() { "Creating..." } else { "Create App" }}
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
