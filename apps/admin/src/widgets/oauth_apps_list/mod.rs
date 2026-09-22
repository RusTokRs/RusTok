use crate::entities::oauth_app::model::OAuthApp;
use crate::entities::oauth_app::ui::badge::AppTypeBadge;
use crate::shared::ui::Button;
use leptos::prelude::*;

fn format_timestamp(value: Option<chrono::DateTime<chrono::Utc>>) -> String {
    value
        .map(|timestamp| timestamp.format("%Y-%m-%d %H:%M UTC").to_string())
        .unwrap_or_else(|| "Never".to_string())
}

#[component]
pub fn OAuthAppsList(
    apps: Vec<OAuthApp>,
    loading: bool,
    on_edit_app: Callback<OAuthApp>,
    on_rotate_secret: Callback<OAuthApp>,
    on_revoke_app: Callback<OAuthApp>,
) -> impl IntoView {
    let rows_apps = apps.clone();
    let is_empty = apps.is_empty();

    view! {
        <div class="overflow-x-auto rounded-md border">
            <table class="w-full min-w-[960px] text-left text-sm">
                <thead class="bg-muted/50 text-xs uppercase text-muted-foreground">
                    <tr>
                        <th class="px-4 py-3 font-medium">"App"</th>
                        <th class="px-4 py-3 font-medium">"Type"</th>
                        <th class="px-4 py-3 font-medium">"Scopes / Grants"</th>
                        <th class="px-4 py-3 font-medium">"Client ID"</th>
                        <th class="px-4 py-3 font-medium">"Tokens"</th>
                        <th class="px-4 py-3 font-medium">"Last Used"</th>
                        <th class="px-4 py-3 text-right font-medium">"Actions"</th>
                    </tr>
                </thead>
                <tbody class="divide-y">
                    <Show when=move || loading>
                        <tr>
                            <td colspan="7" class="h-24 text-center text-muted-foreground">
                                "Loading app connections..."
                            </td>
                        </tr>
                    </Show>

                    {rows_apps
                        .into_iter()
                        .map(|app| {
                            let app_for_edit = app.clone();
                            let app_for_rotate = app.clone();
                            let app_for_revoke = app.clone();
                            let description = app.description.clone().unwrap_or_default();
                            let has_description = !description.is_empty();
                            let scopes = app.scopes.join(", ");
                            let grants = app.grant_types.join(", ");
                            let capability_label = if app.managed_by_manifest {
                                "Managed by config/manifest"
                            } else {
                                "Manual app"
                            };

                            view! {
                                <tr class="transition-colors hover:bg-muted/40">
                                    <td class="px-4 py-3 align-top">
                                        <div class="font-medium text-slate-900">{app.name.clone()}</div>
                                        <div class="text-xs text-muted-foreground">{app.slug.clone()}</div>
                                        <Show when=move || has_description>
                                            <div class="mt-1 max-w-xs text-xs text-muted-foreground">
                                                {description.clone()}
                                            </div>
                                        </Show>
                                        <div class="mt-2 inline-flex rounded-full border px-2 py-1 text-xs text-muted-foreground">
                                            {capability_label}
                                        </div>
                                    </td>
                                    <td class="px-4 py-3 align-top">
                                        <AppTypeBadge app_type=app.app_type.clone() />
                                    </td>
                                    <td class="px-4 py-3 align-top text-xs text-slate-600">
                                        <div>
                                            <span class="font-medium text-slate-900">"Scopes: "</span>
                                            {if scopes.is_empty() { "None".to_string() } else { scopes }}
                                        </div>
                                        <div class="mt-1">
                                            <span class="font-medium text-slate-900">"Grants: "</span>
                                            {if grants.is_empty() { "None".to_string() } else { grants }}
                                        </div>
                                    </td>
                                    <td class="px-4 py-3 align-top font-mono text-xs text-slate-500">
                                        {app.client_id.to_string()}
                                    </td>
                                    <td class="px-4 py-3 align-top text-slate-500">
                                        {app.active_token_count}
                                    </td>
                                    <td class="px-4 py-3 align-top text-xs text-slate-500">
                                        {format_timestamp(app.last_used_at)}
                                    </td>
                                    <td class="px-4 py-3 align-top">
                                        <div class="flex justify-end gap-2">
                                            <Button
                                                class="h-8 bg-transparent px-3 py-1 text-xs text-foreground shadow-none ring-1 ring-border hover:bg-accent"
                                                disabled=Signal::derive(move || !app.can_edit)
                                                on_click=Callback::new(move |_| on_edit_app.run(app_for_edit.clone()))
                                            >
                                                "Edit"
                                            </Button>
                                            <Button
                                                class="h-8 bg-transparent px-3 py-1 text-xs text-foreground shadow-none ring-1 ring-border hover:bg-accent"
                                                disabled=Signal::derive(move || !app.can_rotate_secret)
                                                on_click=Callback::new(move |_| on_rotate_secret.run(app_for_rotate.clone()))
                                            >
                                                "Rotate Secret"
                                            </Button>
                                            <Button
                                                class="h-8 bg-destructive px-3 py-1 text-xs text-destructive-foreground hover:bg-destructive/90"
                                                disabled=Signal::derive(move || !app.can_revoke)
                                                on_click=Callback::new(move |_| on_revoke_app.run(app_for_revoke.clone()))
                                            >
                                                "Revoke"
                                            </Button>
                                        </div>
                                    </td>
                                </tr>
                            }
                        })
                        .collect_view()}

                    <Show when=move || !loading && is_empty>
                        <tr>
                            <td colspan="7" class="h-24 text-center text-muted-foreground">
                                "No app connections found."
                            </td>
                        </tr>
                    </Show>
                </tbody>
            </table>
        </div>
    }
}
