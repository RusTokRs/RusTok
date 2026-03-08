use crate::entities::oauth_app::model::{AppType, OAuthApp};
use crate::shared::ui::{ui_badge, ui_button, ui_input, ui_success_message, ui_textarea};
use leptos::*;
use log::{error, info};
use serde::{Deserialize, Serialize};

// This simulates the GraphQL result
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateOAuthAppResult {
    pub app: OAuthApp,
    pub client_secret: String,
}

#[component]
pub fn CreateAppForm(
    on_success: impl Fn(CreateOAuthAppResult) + 'static + Clone,
    on_cancel: impl Fn() + 'static + Clone,
) -> impl IntoView {
    let (name, set_name) = create_signal("".to_string());
    let (slug, set_slug) = create_signal("".to_string());
    let (description, set_description) = create_signal("".to_string());
    let (app_type, set_app_type) = create_signal("ThirdParty".to_string());

    // In a real app with leptos-graphql, this would be a use_mutation Call
    let create_action = create_action(move |_: &()| {
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
                <ui_input::Input
                    type_="text"
                    prop:value=name
                    on:input=move |ev| set_name.set(event_target_value(&ev))
                />
            </div>
            <div class="space-y-2">
                <label>"Slug/Identifier"</label>
                <ui_input::Input
                    type_="text"
                    prop:value=slug
                    on:input=move |ev| set_slug.set(event_target_value(&ev))
                />
            </div>
            <div class="space-y-2">
                <label>"Description"</label>
                <ui_textarea::Textarea
                    prop:value=description
                    on:input=move |ev| set_description.set(event_target_value(&ev))
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
                <ui_button::Button
                    on:click=move |_| create_action.dispatch(())
                >
                    "Create App"
                </ui_button::Button>
                <ui_button::Button
                    variant=crate::shared::ui::ButtonVariant::Outline
                    on:click=move |_| on_cancel()
                >
                    "Cancel"
                </ui_button::Button>
            </div>
        </div>
    }
}
