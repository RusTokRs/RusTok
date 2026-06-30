use leptos::ev::SubmitEvent;
use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::model::{
    ApplyMcpScaffoldDraftPayload, CreateMcpClientPayload, RotateMcpTokenPayload,
    StageMcpScaffoldDraftPayload, UpdateMcpPolicyPayload,
};
use crate::transport;

#[component]
pub fn McpAdmin() -> impl IntoView {
    let (drafts, set_drafts) = signal(Vec::new());
    let (audit_events, set_audit_events) = signal(Vec::new());
    let (clients, set_clients) = signal(Vec::new());
    let (client_details, set_client_details) = signal(None);
    let (selected_client_id, set_selected_client_id) = signal(String::new());
    let (selected_draft_id, set_selected_draft_id) = signal(String::new());
    let (feedback, set_feedback) = signal(Option::<String>::None);
    let (error, set_error) = signal(Option::<String>::None);
    let (plaintext_token, set_plaintext_token) = signal(Option::<String>::None);

    let slug = RwSignal::new(String::new());
    let name = RwSignal::new(String::new());
    let description = RwSignal::new(String::new());
    let dependencies = RwSignal::new(String::new());
    let with_graphql = RwSignal::new(true);
    let with_rest = RwSignal::new(true);
    let workspace_root = RwSignal::new(String::new());
    let confirm_apply = RwSignal::new(false);
    let client_slug = RwSignal::new(String::new());
    let client_name = RwSignal::new(String::new());
    let client_description = RwSignal::new(String::new());
    let client_actor_type = RwSignal::new("SERVICE_CLIENT".to_string());
    let client_token_name = RwSignal::new("primary".to_string());
    let rotate_token_name = RwSignal::new("rotated".to_string());
    let revoke_existing_tokens = RwSignal::new(true);
    let management_reason = RwSignal::new(String::new());
    let policy_allowed_tools = RwSignal::new(String::new());
    let policy_denied_tools = RwSignal::new(String::new());
    let policy_permissions = RwSignal::new(String::new());
    let policy_scopes = RwSignal::new(String::new());

    let reload = move || {
        spawn_local(async move {
            match transport::fetch_scaffold_drafts().await {
                Ok(items) => {
                    let next_id = selected_draft_id.get_untracked().pipe(|current| {
                        if items.iter().any(|draft| draft.id == current) {
                            current
                        } else {
                            items
                                .first()
                                .map(|draft| draft.id.clone())
                                .unwrap_or_default()
                        }
                    });
                    set_selected_draft_id.set(next_id);
                    set_drafts.set(items);
                    set_error.set(None);
                }
                Err(err) => set_error.set(Some(err.to_string())),
            }
        });
    };

    let reload_audit = move || {
        spawn_local(async move {
            match transport::fetch_audit_events().await {
                Ok(items) => {
                    set_audit_events.set(items);
                    set_error.set(None);
                }
                Err(err) => set_error.set(Some(err.to_string())),
            }
        });
    };

    let reload_clients = move || {
        spawn_local(async move {
            match transport::fetch_clients().await {
                Ok(items) => {
                    let current = selected_client_id.get_untracked();
                    let next_id = if items.iter().any(|client| client.id == current) {
                        current
                    } else {
                        items
                            .first()
                            .map(|client| client.id.clone())
                            .unwrap_or_default()
                    };
                    set_clients.set(items);
                    set_selected_client_id.set(next_id);
                    set_error.set(None);
                }
                Err(err) => set_error.set(Some(err.to_string())),
            }
        });
    };

    Effect::new(move |_| {
        reload();
        reload_audit();
        reload_clients();
    });

    Effect::new(move |_| {
        let client_id = selected_client_id.get();
        if client_id.is_empty() {
            set_client_details.set(None);
            return;
        }
        spawn_local(async move {
            match transport::fetch_client_details(client_id).await {
                Ok(details) => {
                    if let Some(policy) =
                        details.as_ref().and_then(|details| details.policy.as_ref())
                    {
                        policy_allowed_tools.set(policy.allowed_tools.join(", "));
                        policy_denied_tools.set(policy.denied_tools.join(", "));
                        policy_permissions.set(policy.granted_permissions.join(", "));
                        policy_scopes.set(policy.granted_scopes.join(", "));
                    }
                    set_client_details.set(details);
                }
                Err(err) => set_error.set(Some(err.to_string())),
            }
        });
    });

    let selected_draft = move || {
        let id = selected_draft_id.get();
        drafts.get().into_iter().find(|draft| draft.id == id)
    };

    let stage = move |event: SubmitEvent| {
        event.prevent_default();
        let input = StageMcpScaffoldDraftPayload {
            client_id: None,
            slug: slug.get_untracked(),
            name: name.get_untracked(),
            description: description.get_untracked(),
            dependencies: split_csv(&dependencies.get_untracked()),
            with_graphql: with_graphql.get_untracked(),
            with_rest: with_rest.get_untracked(),
        };
        spawn_local(async move {
            match transport::stage_scaffold_draft(input).await {
                Ok(draft) => {
                    set_selected_draft_id.set(draft.id.clone());
                    set_feedback.set(Some(format!("Draft {} staged.", draft.crate_name)));
                    reload();
                    reload_audit();
                }
                Err(err) => set_error.set(Some(err.to_string())),
            }
        });
    };

    let apply = move |_| {
        let Some(draft) = selected_draft() else {
            return;
        };
        let input = ApplyMcpScaffoldDraftPayload {
            draft_id: draft.id,
            workspace_root: workspace_root.get_untracked(),
            confirm: confirm_apply.get_untracked(),
        };
        spawn_local(async move {
            match transport::apply_scaffold_draft(input).await {
                Ok(draft) => {
                    set_feedback.set(Some(format!("Draft {} applied.", draft.crate_name)));
                    reload();
                    reload_audit();
                }
                Err(err) => set_error.set(Some(err.to_string())),
            }
        });
    };

    let create_client = move |event: SubmitEvent| {
        event.prevent_default();
        let input = CreateMcpClientPayload {
            slug: client_slug.get_untracked(),
            display_name: client_name.get_untracked(),
            description: client_description.get_untracked(),
            actor_type: client_actor_type.get_untracked(),
            token_name: client_token_name.get_untracked(),
            token_expires_at: String::new(),
            allowed_tools: Vec::new(),
            denied_tools: Vec::new(),
            granted_permissions: Vec::new(),
            granted_scopes: Vec::new(),
        };
        spawn_local(async move {
            match transport::create_client(input).await {
                Ok(secret) => {
                    set_plaintext_token.set(Some(secret.plaintext_token));
                    set_selected_client_id.set(secret.client_id);
                    set_feedback.set(Some("MCP client created.".to_string()));
                    reload_clients();
                    reload_audit();
                }
                Err(err) => set_error.set(Some(err.to_string())),
            }
        });
    };

    let rotate_token = move |_| {
        let client_id = selected_client_id.get_untracked();
        if client_id.is_empty() {
            return;
        }
        let input = RotateMcpTokenPayload {
            client_id,
            token_name: rotate_token_name.get_untracked(),
            expires_at: String::new(),
            revoke_existing_tokens: revoke_existing_tokens.get_untracked(),
        };
        spawn_local(async move {
            match transport::rotate_token(input).await {
                Ok(secret) => {
                    set_plaintext_token.set(Some(secret.plaintext_token));
                    set_feedback.set(Some("MCP token rotated.".to_string()));
                    reload_clients();
                    reload_audit();
                }
                Err(err) => set_error.set(Some(err.to_string())),
            }
        });
    };

    let update_policy = move |_| {
        let client_id = selected_client_id.get_untracked();
        if client_id.is_empty() {
            return;
        }
        let input = UpdateMcpPolicyPayload {
            client_id,
            allowed_tools: split_csv(&policy_allowed_tools.get_untracked()),
            denied_tools: split_csv(&policy_denied_tools.get_untracked()),
            granted_permissions: split_csv(&policy_permissions.get_untracked()),
            granted_scopes: split_csv(&policy_scopes.get_untracked()),
        };
        spawn_local(async move {
            match transport::update_policy(input).await {
                Ok(()) => {
                    set_feedback.set(Some("MCP policy updated.".to_string()));
                    reload_clients();
                    reload_audit();
                }
                Err(err) => set_error.set(Some(err.to_string())),
            }
        });
    };

    let revoke_token = move |token_id: String| {
        let reason = management_reason.get_untracked();
        spawn_local(async move {
            match transport::revoke_token(token_id, reason).await {
                Ok(()) => {
                    set_feedback.set(Some("MCP token revoked.".to_string()));
                    reload_clients();
                    reload_audit();
                }
                Err(err) => set_error.set(Some(err.to_string())),
            }
        });
    };

    let deactivate_client = move |_| {
        let client_id = selected_client_id.get_untracked();
        if client_id.is_empty() {
            return;
        }
        let reason = management_reason.get_untracked();
        spawn_local(async move {
            match transport::deactivate_client(client_id, reason).await {
                Ok(()) => {
                    set_feedback.set(Some("MCP client deactivated.".to_string()));
                    reload_clients();
                    reload_audit();
                }
                Err(err) => set_error.set(Some(err.to_string())),
            }
        });
    };

    view! {
        <section class="mcp-admin">
            <header>
                <p>"MCP control plane"</p>
                <h1>"Alloy scaffold drafts"</h1>
            </header>
            {move || error.get().map(|message| view! { <p class="error">{message}</p> })}
            {move || feedback.get().map(|message| view! { <p class="feedback">{message}</p> })}
            {move || plaintext_token.get().map(|token| view! {
                <aside class="mcp-token-secret">
                    <strong>"New token - shown once"</strong>
                    <code>{token}</code>
                    <button type="button" on:click=move |_| set_plaintext_token.set(None)>"Dismiss"</button>
                </aside>
            })}
            <form on:submit=stage>
                <h2>"Stage draft"</h2>
                <label>"Slug"<input prop:value=slug on:input=move |event| slug.set(event_target_value(&event)) /></label>
                <label>"Name"<input prop:value=name on:input=move |event| name.set(event_target_value(&event)) /></label>
                <label>"Description"<input prop:value=description on:input=move |event| description.set(event_target_value(&event)) /></label>
                <label>"Dependencies"<input prop:value=dependencies on:input=move |event| dependencies.set(event_target_value(&event)) /></label>
                <label><input type="checkbox" prop:checked=with_graphql on:change=move |event| with_graphql.set(event_target_checked(&event)) />"GraphQL"</label>
                <label><input type="checkbox" prop:checked=with_rest on:change=move |event| with_rest.set(event_target_checked(&event)) />"REST"</label>
                <button type="submit">"Stage draft"</button>
            </form>
            <section>
                <h2>"Review and apply"</h2>
                <button type="button" on:click=move |_| reload()>"Refresh"</button>
                <select prop:value=selected_draft_id on:change=move |event| set_selected_draft_id.set(event_target_value(&event))>
                    <option value="">"Select draft"</option>
                    {move || drafts.get().into_iter().map(|draft| {
                        let label = format!("{} - {}", draft.crate_name, draft.status);
                        view! { <option value=draft.id>{label}</option> }
                    }).collect_view()}
                </select>
                {move || selected_draft().map(|draft| view! {
                    <div>
                        <p>{format!("Crate: {}", draft.crate_name)}</p>
                        <p>{format!("Slug: {}", draft.slug)}</p>
                        <p>{format!("Status: {}", draft.status)}</p>
                        <pre>{format_json_for_display(&draft.preview_json)}</pre>
                        <label>"Workspace root"<input prop:value=workspace_root on:input=move |event| workspace_root.set(event_target_value(&event)) /></label>
                        <label><input type="checkbox" prop:checked=confirm_apply on:change=move |event| confirm_apply.set(event_target_checked(&event)) />"Confirm apply"</label>
                        <button type="button" on:click=apply disabled=move || !confirm_apply.get() || workspace_root.get().is_empty() || draft.status == "APPLIED">"Apply draft"</button>
                    </div>
                })}
            </section>
            <section>
                <h2>"MCP clients"</h2>
                <form on:submit=create_client>
                    <h3>"Create client"</h3>
                    <label>"Slug"<input prop:value=client_slug on:input=move |event| client_slug.set(event_target_value(&event)) /></label>
                    <label>"Display name"<input prop:value=client_name on:input=move |event| client_name.set(event_target_value(&event)) /></label>
                    <label>"Description"<input prop:value=client_description on:input=move |event| client_description.set(event_target_value(&event)) /></label>
                    <label>"Actor type"
                        <select prop:value=client_actor_type on:change=move |event| client_actor_type.set(event_target_value(&event))>
                            <option value="SERVICE_CLIENT">"Service client"</option>
                            <option value="MODEL_AGENT">"Model agent"</option>
                            <option value="HUMAN_USER">"Human user"</option>
                        </select>
                    </label>
                    <label>"Initial token name"<input prop:value=client_token_name on:input=move |event| client_token_name.set(event_target_value(&event)) /></label>
                    <button type="submit">"Create client"</button>
                </form>
                <button type="button" on:click=move |_| reload_clients()>"Refresh clients"</button>
                <select prop:value=selected_client_id on:change=move |event| set_selected_client_id.set(event_target_value(&event))>
                    <option value="">"Select client"</option>
                    {move || clients.get().into_iter().map(|client| {
                        let label = format!("{} - {}", client.display_name, if client.is_active { "active" } else { "inactive" });
                        view! { <option value=client.id>{label}</option> }
                    }).collect_view()}
                </select>
                {move || client_details.get().map(|details| view! {
                    <div class="mcp-client-details">
                        <h3>{details.client.display_name}</h3>
                        <p>{format!("Slug: {}", details.client.slug)}</p>
                        <p>{format!("Actor: {}", details.client.actor_type)}</p>
                        <section>
                            <h4>"Policy"</h4>
                            <label>"Allowed tools"<input prop:value=policy_allowed_tools on:input=move |event| policy_allowed_tools.set(event_target_value(&event)) /></label>
                            <label>"Denied tools"<input prop:value=policy_denied_tools on:input=move |event| policy_denied_tools.set(event_target_value(&event)) /></label>
                            <label>"Permissions"<input prop:value=policy_permissions on:input=move |event| policy_permissions.set(event_target_value(&event)) /></label>
                            <label>"Scopes"<input prop:value=policy_scopes on:input=move |event| policy_scopes.set(event_target_value(&event)) /></label>
                            <button type="button" on:click=update_policy>"Update policy"</button>
                        </section>
                        <section>
                            <h4>"Tokens"</h4>
                            <label>"New token name"<input prop:value=rotate_token_name on:input=move |event| rotate_token_name.set(event_target_value(&event)) /></label>
                            <label><input type="checkbox" prop:checked=revoke_existing_tokens on:change=move |event| revoke_existing_tokens.set(event_target_checked(&event)) />"Revoke existing tokens"</label>
                            <button type="button" on:click=rotate_token>"Rotate token"</button>
                            {details.tokens.into_iter().map(|token| {
                                let token_id = token.id.clone();
                                view! {
                                    <article>
                                        <strong>{token.token_name}</strong>
                                        <p>{token.token_preview}</p>
                                        <p>{if token.is_active { "Active" } else { "Inactive" }}</p>
                                        {token.expires_at.map(|expires_at| view! { <time>{format!("Expires: {expires_at}")}</time> })}
                                        <button type="button" disabled=!token.is_active on:click=move |_| revoke_token(token_id.clone())>"Revoke"</button>
                                    </article>
                                }
                            }).collect_view()}
                        </section>
                        <label>"Management reason"<input prop:value=management_reason on:input=move |event| management_reason.set(event_target_value(&event)) /></label>
                        <button type="button" disabled=!details.client.is_active on:click=deactivate_client>"Deactivate client"</button>
                    </div>
                })}
            </section>
            <section>
                <h2>"Audit events"</h2>
                <button type="button" on:click=move |_| reload_audit()>"Refresh audit"</button>
                <div class="mcp-audit-events">
                    {move || audit_events.get().into_iter().map(|event| view! {
                        <article>
                            <strong>{format!("{} - {}", event.action, event.outcome)}</strong>
                            <p>{event.tool_name.unwrap_or_else(|| "Control plane".to_string())}</p>
                            <p>{event.actor_type.unwrap_or_else(|| "unknown actor".to_string())}</p>
                            <time>{event.created_at}</time>
                            {event.reason.map(|reason| view! { <p>{reason}</p> })}
                        </article>
                    }).collect_view()}
                </div>
            </section>
        </section>
    }
}

fn split_csv(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn format_json_for_display(value: &str) -> String {
    serde_json::from_str::<serde_json::Value>(value)
        .and_then(|json| serde_json::to_string_pretty(&json))
        .unwrap_or_else(|_| value.to_string())
}

trait Pipe: Sized {
    fn pipe<T>(self, f: impl FnOnce(Self) -> T) -> T {
        f(self)
    }
}

impl<T> Pipe for T {}
