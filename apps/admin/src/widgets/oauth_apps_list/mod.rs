use crate::entities::oauth_app::model::OAuthApp;
use crate::entities::oauth_app::ui::badge::AppTypeBadge;
use crate::shared::ui::ui_button;
use leptos::*;

#[component]
pub fn OAuthAppsList(
    apps: Vec<OAuthApp>,
    on_rotate_secret: impl Fn(OAuthApp) + 'static + Clone,
    on_revoke_app: impl Fn(OAuthApp) + 'static + Clone,
) -> impl IntoView {
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
                    <For
                        each=move || apps.clone()
                        key=|app| app.id
                        children=move |app| {
                            let app_clone1 = app.clone();
                            let app_clone2 = app.clone();
                            let on_rotate_secret = on_rotate_secret.clone();
                            let on_revoke_app = on_revoke_app.clone();

                            view! {
                                <tr class="hover:bg-muted/50 transition-colors">
                                    <td class="px-4 py-3 font-medium text-slate-900">
                                        {app.name.clone()}
                                    </td>
                                    <td class="px-4 py-3">
                                        <AppTypeBadge app_type=app.app_type.clone() />
                                    </td>
                                    <td class="px-4 py-3 text-slate-500 font-mono text-xs">
                                        {app.client_id.to_string()}
                                    </td>
                                    <td class="px-4 py-3 text-slate-500">
                                        {app.active_token_count}
                                    </td>
                                    <td class="px-4 py-3 text-right space-x-2">
                                        <Button
                                            variant=crate::shared::ui::ButtonVariant::Outline
                                            size=crate::shared::ui::Size::Sm
                                            on:click=move |_| on_rotate_secret(app_clone1.clone())
                                        >
                                            "Rotate Secret"
                                        </Button>
                                        <Button
                                            variant=crate::shared::ui::ButtonVariant::Destructive
                                            size=crate::shared::ui::Size::Sm
                                            on:click=move |_| on_revoke_app(app_clone2.clone())
                                        >
                                            "Revoke"
                                        </Button>
                                    </td>
                                </tr>
                            }
                        }
                    />
                    <Show when=move || apps.is_empty()>
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
