use crate::entities::oauth_app::model::OAuthApp;
use crate::entities::oauth_app::ui::badge::AppTypeBadge;
use crate::shared::ui::ui_button as UiButton;
use leptos::prelude::*;

#[component]
pub fn OAuthAppsList(
    apps: Vec<OAuthApp>,
    on_rotate_secret: Callback<OAuthApp>,
    on_revoke_app: Callback<OAuthApp>,
) -> impl IntoView {
    let rows_apps = apps.clone();
    let is_empty = apps.is_empty();

    view! {
        <div class="overflow-x-auto rounded-md border">
            <table class="w-full text-left text-sm whitespace-nowrap">
                <thead class="bg-muted/50 text-muted-foreground uppercase text-xs">
                    <tr>
                        <th class="px-4 py-3 font-medium">"Name"</th>
                        <th class="px-4 py-3 font-medium">"Type"</th>
                        <th class="px-4 py-3 font-medium">"Client ID"</th>
                        <th class="px-4 py-3 font-medium">"Active Tokens"</th>
                        <th class="px-4 py-3 font-medium text-right">"Actions"</th>
                    </tr>
                </thead>
                <tbody class="divide-y">
                    {rows_apps
                        .into_iter()
                        .map(|app| {
                            let app_clone1 = app.clone();
                            let app_clone2 = app.clone();

                            view! {
                                <tr class="hover:bg-muted/50 transition-colors">
                                    <td class="px-4 py-3 font-medium text-slate-900">{app.name.clone()}</td>
                                    <td class="px-4 py-3"><AppTypeBadge app_type=app.app_type.clone() /></td>
                                    <td class="px-4 py-3 text-slate-500 font-mono text-xs">{app.client_id.to_string()}</td>
                                    <td class="px-4 py-3 text-slate-500">{app.active_token_count}</td>
                                    <td class="px-4 py-3 text-right space-x-2">
                                        <UiButton
                                            variant=crate::shared::ui::ButtonVariant::Outline
                                            size=crate::shared::ui::Size::Sm
                                            on_click=Box::new(move || on_rotate_secret.run(app_clone1.clone()))
                                        >
                                            "Rotate Secret"
                                        </UiButton>
                                        <UiButton
                                            variant=crate::shared::ui::ButtonVariant::Destructive
                                            size=crate::shared::ui::Size::Sm
                                            on_click=Box::new(move || on_revoke_app.run(app_clone2.clone()))
                                        >
                                            "Revoke"
                                        </UiButton>
                                    </td>
                                </tr>
                            }
                        })
                        .collect_view()}
                    <Show when=move || is_empty>
                        <tr>
                            <td colspan="5" class="h-24 text-center text-slate-500">
                                "No connections found. Connect an app to get started."
                            </td>
                        </tr>
                    </Show>
                </tbody>
            </table>
        </div>
    }
}
