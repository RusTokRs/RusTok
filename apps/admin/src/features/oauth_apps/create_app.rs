use crate::entities::oauth_app::model::{AppType, OAuthApp};
use crate::shared::ui::{ui_button as UiButton, ui_input as UiInput, ui_textarea as UiTextarea};
use leptos::prelude::*;
use log::info;
use serde::{Deserialize, Serialize};

// This simulates the GraphQL result
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateOAuthAppResult {
    pub app: OAuthApp,
    pub client_secret: String,
}

#[component]
pub fn CreateAppForm(
    on_success: impl Fn(CreateOAuthAppResult) + Send + Sync + 'static + Clone,
    on_cancel: impl Fn() + 'static + Clone,
) -> impl IntoView {
    let (name, set_name) = signal("".to_string());
    let (slug, set_slug) = signal("".to_string());
    let (description, set_description) = signal("".to_string());
    let (app_type, set_app_type) = signal("ThirdParty".to_string());

    // In a real app with leptos-graphql, this would be a use_mutation Call
    let create_action = Action::new(move |_: &()| {
        let name_val = name.get();
        let slug_val = slug.get();
        let desc_val = description.get();
        let type_val = app_type.get();
        let on_success = on_success.clone();

        async move {
            info!(
                "MOCK: Creating app {} ({}) of type {}",
                name_val, slug_val, type_val
            );

            // Mock GraphQL request logic here
            // let client = reqwest::Client::new();
            // let res = client.post("...").send().await...

            /* Mock Response */
            let mock_app = OAuthApp {
                id: uuid::Uuid::new_v4(),
                name: name_val,
                slug: slug_val,
                description: Some(desc_val),
                app_type: AppType::ThirdParty, // Parse type
                client_id: uuid::Uuid::new_v4(),
                redirect_uris: vec![],
                scopes: vec![],
                grant_types: vec!["authorization_code".into()],
                manifest_ref: None,
                auto_created: false,
                is_active: true,
                active_token_count: 0,
                last_used_at: None,
                created_at: chrono::Utc::now(),
            };

            on_success(CreateOAuthAppResult {
                app: mock_app,
                client_secret: "sk_live_mock_secret_12345".into(),
            });
        }
    });

    view! {
        <div class="space-y-4">
            <h3 class="text-lg font-medium">"Create New Connected App"</h3>
            <div class="space-y-2">
                <label>"App Name"</label>
                <UiInput
                    r#type="text"
                    value=name
                    set_value=set_name
                />
            </div>
            <div class="space-y-2">
                <label>"Slug/Identifier"</label>
                <UiInput
                    r#type="text"
                    value=slug
                    set_value=set_slug
                />
            </div>
            <div class="space-y-2">
                <label>"Description"</label>
                <UiTextarea
                    value=description
                    set_value=set_description
                />
            </div>
            <div class="space-y-2">
                <label>"App Type"</label>
                // Need a Select, but native select or simple input for now
                <select
                    class="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background"
                    on:change=move |ev| set_app_type.set(event_target_value(&ev))
                >
                    <option value="ThirdParty">"Third Party (Integration)"</option>
                    <option value="FirstParty">"First Party (Storefront/Admin)"</option>
                    <option value="Mobile">"Mobile"</option>
                    <option value="Service">"Service (M2M)"</option>
                </select>
            </div>
            <div class="flex items-center gap-2 pt-4">
                <UiButton
                    on_click=Box::new(move || { create_action.dispatch(()); })
                >
                    "Create App"
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
