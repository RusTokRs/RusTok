use leptos::prelude::*;
use leptos_auth::hooks::{use_tenant, use_token};
use leptos_ui::{
    Alert, AlertVariant, Badge, BadgeVariant, Card, CardContent, CardDescription, CardHeader,
    CardTitle,
};
use rustok_ui_core::UiRouteContext;

use crate::core::{
    ChannelListItemViewModel, E2eeDeviceAuditViewModel, MessengerAdminLabels, MessengerAdminTab,
};
use crate::transport;

fn local_resource<S, Fut, T>(
    source: impl Fn() -> S + 'static,
    fetcher: impl Fn(S) -> Fut + 'static,
) -> LocalResource<T>
where
    S: 'static,
    Fut: std::future::Future<Output = T> + 'static,
    T: 'static,
{
    LocalResource::new(move || fetcher(source()))
}

#[component]
pub fn MessengerAdmin() -> impl IntoView {
    let token = use_token();
    let tenant = use_tenant();
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let labels = StoredValue::new(MessengerAdminLabels::resolve(
        route_context.locale.as_deref(),
    ));

    let (active_tab, set_active_tab) = signal(MessengerAdminTab::Overview);

    let overview_resource = local_resource(
        move || (token.get(), tenant.get()),
        move |(tok, ten)| async move { transport::fetch_overview(tok, ten).await },
    );

    let channels_resource = local_resource(
        move || (token.get(), tenant.get()),
        move |(tok, ten)| async move { transport::fetch_channels(tok, ten).await },
    );

    let devices_resource = local_resource(
        move || (token.get(), tenant.get()),
        move |(tok, ten)| async move { transport::fetch_e2ee_devices(tok, ten).await },
    );

    let l = labels.get_value();

    view! {
        <div class="space-y-6">
            // Page Header
            <div class="flex flex-col gap-1 sm:flex-row sm:items-center sm:justify-between">
                <div>
                    <span class="text-xs font-semibold tracking-wider text-primary uppercase">
                        {l.eyebrow.clone()}
                    </span>
                    <h1 class="text-2xl font-bold tracking-tight text-foreground sm:text-3xl">
                        {l.title.clone()}
                    </h1>
                    <p class="text-sm text-muted-foreground">
                        {l.subtitle.clone()}
                    </p>
                </div>
            </div>

            // Navigation Tabs
            <div class="flex border-b border-border space-x-1">
                <button
                    type="button"
                    class=move || {
                        if active_tab.get() == MessengerAdminTab::Overview {
                            "border-b-2 border-primary px-4 py-2 text-sm font-semibold text-primary transition-colors"
                        } else {
                            "border-b-2 border-transparent px-4 py-2 text-sm font-medium text-muted-foreground hover:text-foreground transition-colors"
                        }
                    }
                    on:click=move |_| set_active_tab.set(MessengerAdminTab::Overview)
                >
                    {labels.get_value().tab_overview}
                </button>
                <button
                    type="button"
                    class=move || {
                        if active_tab.get() == MessengerAdminTab::Channels {
                            "border-b-2 border-primary px-4 py-2 text-sm font-semibold text-primary transition-colors"
                        } else {
                            "border-b-2 border-transparent px-4 py-2 text-sm font-medium text-muted-foreground hover:text-foreground transition-colors"
                        }
                    }
                    on:click=move |_| set_active_tab.set(MessengerAdminTab::Channels)
                >
                    {labels.get_value().tab_channels}
                </button>
                <button
                    type="button"
                    class=move || {
                        if active_tab.get() == MessengerAdminTab::E2ee {
                            "border-b-2 border-primary px-4 py-2 text-sm font-semibold text-primary transition-colors"
                        } else {
                            "border-b-2 border-transparent px-4 py-2 text-sm font-medium text-muted-foreground hover:text-foreground transition-colors"
                        }
                    }
                    on:click=move |_| set_active_tab.set(MessengerAdminTab::E2ee)
                >
                    {labels.get_value().tab_e2ee}
                </button>
                <button
                    type="button"
                    class=move || {
                        if active_tab.get() == MessengerAdminTab::Federation {
                            "border-b-2 border-primary px-4 py-2 text-sm font-semibold text-primary transition-colors"
                        } else {
                            "border-b-2 border-transparent px-4 py-2 text-sm font-medium text-muted-foreground hover:text-foreground transition-colors"
                        }
                    }
                    on:click=move |_| set_active_tab.set(MessengerAdminTab::Federation)
                >
                    {labels.get_value().tab_federation}
                </button>
            </div>

            // Active Tab Content
            {
                move || {
                    let lbl = labels.get_value();
                    match active_tab.get() {
                        MessengerAdminTab::Overview => {
                            view! {
                                <div class="space-y-6">
                                    // Key Metrics Grid
                                    <div class="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-4">
                                        <Card>
                                            <CardHeader>
                                                <CardTitle>{lbl.stat_conversations.clone()}</CardTitle>
                                            </CardHeader>
                                            <CardContent>
                                                <span class="text-3xl font-bold">
                                                    {move || overview_resource.get().and_then(|r| r.ok()).map(|o| o.active_conversations_count).unwrap_or(0)}
                                                </span>
                                            </CardContent>
                                        </Card>

                                        <Card>
                                            <CardHeader>
                                                <CardTitle>{lbl.stat_streaming.clone()}</CardTitle>
                                            </CardHeader>
                                            <CardContent>
                                                <span class="text-3xl font-bold text-primary">
                                                    {move || overview_resource.get().and_then(|r| r.ok()).map(|o| o.partitions_count).unwrap_or(16)}
                                                </span>
                                            </CardContent>
                                        </Card>

                                        <Card>
                                            <CardHeader>
                                                <CardTitle>{lbl.stat_e2ee_devices.clone()}</CardTitle>
                                            </CardHeader>
                                            <CardContent>
                                                <span class="text-3xl font-bold">
                                                    {move || overview_resource.get().and_then(|r| r.ok()).map(|o| o.registered_devices_count).unwrap_or(0)}
                                                </span>
                                            </CardContent>
                                        </Card>

                                        <Card>
                                            <CardHeader>
                                                <CardTitle>{lbl.stat_matrix.clone()}</CardTitle>
                                            </CardHeader>
                                            <CardContent>
                                                <Badge variant=BadgeVariant::Outline>
                                                    {lbl.federation_status.clone()}
                                                </Badge>
                                            </CardContent>
                                        </Card>
                                    </div>

                                    // Streaming Details Card
                                    <Card>
                                        <CardHeader>
                                            <CardTitle>{lbl.streaming_title.clone()}</CardTitle>
                                            <CardDescription>
                                                "Sub-millisecond horizontal message bus partitioned by tenant and conversation"
                                            </CardDescription>
                                        </CardHeader>
                                        <CardContent>
                                            <div class="grid grid-cols-1 md:grid-cols-3 gap-4 text-sm">
                                                <div class="rounded-lg border border-border p-3 bg-muted/40">
                                                    <div class="text-xs text-muted-foreground">{lbl.streaming_topic.clone()}</div>
                                                    <div class="font-mono font-medium mt-1">"rustok-messenger-events"</div>
                                                </div>
                                                <div class="rounded-lg border border-border p-3 bg-muted/40">
                                                    <div class="text-xs text-muted-foreground">{lbl.streaming_group.clone()}</div>
                                                    <div class="font-mono font-medium mt-1">"rustok-messenger-workers"</div>
                                                </div>
                                                <div class="rounded-lg border border-border p-3 bg-muted/40">
                                                    <div class="text-xs text-muted-foreground">{lbl.streaming_partitions.clone()}</div>
                                                    <div class="font-mono font-medium mt-1">"16 Partitions (Monotonic Hashing)"</div>
                                                </div>
                                            </div>
                                        </CardContent>
                                    </Card>
                                </div>
                            }.into_any()
                        }
                        MessengerAdminTab::Channels => {
                            view! {
                                <Card>
                                    <CardHeader>
                                        <div class="flex items-center justify-between">
                                            <div>
                                                <CardTitle>{lbl.channels_title.clone()}</CardTitle>
                                                <CardDescription>"Manage multi-lingual channels and topic hierarchies"</CardDescription>
                                            </div>
                                        </div>
                                    </CardHeader>
                                    <CardContent>
                                        <div class="overflow-x-auto">
                                            <table class="w-full text-left text-sm">
                                                <thead class="border-b border-border bg-muted/50 text-xs uppercase text-muted-foreground">
                                                    <tr>
                                                        <th class="p-3">{lbl.channels_id.clone()}</th>
                                                        <th class="p-3">{lbl.channels_type.clone()}</th>
                                                        <th class="p-3">{lbl.channels_title_col.clone()}</th>
                                                        <th class="p-3">{lbl.channels_topic_col.clone()}</th>
                                                        <th class="p-3">{lbl.channels_created.clone()}</th>
                                                    </tr>
                                                </thead>
                                                <tbody class="divide-y divide-border">
                                                    {
                                                        move || {
                                                            let channels = channels_resource.get().and_then(|r| r.ok()).unwrap_or_default();
                                                            let l = labels.get_value();
                                                            if channels.is_empty() {
                                                                view! {
                                                                    <tr>
                                                                        <td colspan="5" class="p-4 text-center text-muted-foreground">
                                                                            {l.channels_empty}
                                                                        </td>
                                                                    </tr>
                                                                }.into_any()
                                                            } else {
                                                                channels.into_iter().map(|c| {
                                                                    let vm = ChannelListItemViewModel::from_payload(&c);
                                                                    view! {
                                                                        <tr class="hover:bg-muted/30">
                                                                            <td class="p-3 font-mono text-xs">{vm.id_display}</td>
                                                                            <td class="p-3">
                                                                                <Badge variant=BadgeVariant::Outline>{vm.type_label}</Badge>
                                                                            </td>
                                                                            <td class="p-3 font-medium">{vm.title_display}</td>
                                                                            <td class="p-3 text-muted-foreground">{vm.topic_display}</td>
                                                                            <td class="p-3 text-xs">{vm.created_at_display}</td>
                                                                        </tr>
                                                                    }
                                                                }).collect::<Vec<_>>().into_any()
                                                            }
                                                        }
                                                    }
                                                </tbody>
                                            </table>
                                        </div>
                                    </CardContent>
                                </Card>
                            }.into_any()
                        }
                        MessengerAdminTab::E2ee => {
                            view! {
                                <div class="space-y-6">
                                    <Alert variant=AlertVariant::Info>
                                        <div class="flex flex-col gap-1">
                                            <span class="font-semibold">{lbl.e2ee_badge.clone()}</span>
                                            <span class="text-xs">
                                                {lbl.e2ee_description.clone()}
                                            </span>
                                        </div>
                                    </Alert>

                                    <Card>
                                        <CardHeader>
                                            <CardTitle>{lbl.e2ee_title.clone()}</CardTitle>
                                            <CardDescription>"Public identity keys and one-time prekeys (OTK) pool inspection"</CardDescription>
                                        </CardHeader>
                                        <CardContent>
                                            <div class="overflow-x-auto">
                                                <table class="w-full text-left text-sm">
                                                    <thead class="border-b border-border bg-muted/50 text-xs uppercase text-muted-foreground">
                                                        <tr>
                                                            <th class="p-3">{lbl.e2ee_device_id.clone()}</th>
                                                            <th class="p-3">{lbl.e2ee_user_id.clone()}</th>
                                                            <th class="p-3">"Identity Keys"</th>
                                                            <th class="p-3">{lbl.e2ee_otk_pool.clone()}</th>
                                                        </tr>
                                                    </thead>
                                                    <tbody class="divide-y divide-border">
                                                        {
                                                            move || {
                                                                let devices = devices_resource.get().and_then(|r| r.ok()).unwrap_or_default();
                                                                let l = labels.get_value();
                                                                if devices.is_empty() {
                                                                    view! {
                                                                        <tr>
                                                                            <td colspan="4" class="p-4 text-center text-muted-foreground">
                                                                                {l.e2ee_empty}
                                                                            </td>
                                                                        </tr>
                                                                    }.into_any()
                                                                } else {
                                                                    devices.into_iter().map(|d| {
                                                                        let vm = E2eeDeviceAuditViewModel::from_payload(&d);
                                                                        view! {
                                                                            <tr class="hover:bg-muted/30">
                                                                                <td class="p-3 font-mono text-xs">{vm.device_id}</td>
                                                                                <td class="p-3 font-mono text-xs">{vm.user_id_display}</td>
                                                                                <td class="p-3">
                                                                                    <Badge variant=BadgeVariant::Outline>{vm.has_keys_badge}</Badge>
                                                                                </td>
                                                                                <td class="p-3 text-xs">
                                                                                    <span class="font-bold mr-1">{vm.otk_count}</span>
                                                                                    <Badge variant=BadgeVariant::Secondary>{vm.otk_status}</Badge>
                                                                                </td>
                                                                            </tr>
                                                                        }
                                                                    }).collect::<Vec<_>>().into_any()
                                                                }
                                                            }
                                                        }
                                                    </tbody>
                                                </table>
                                            </div>
                                        </CardContent>
                                    </Card>
                                </div>
                            }.into_any()
                        }
                        MessengerAdminTab::Federation => {
                            view! {
                                <Card>
                                    <CardHeader>
                                        <CardTitle>{lbl.federation_title.clone()}</CardTitle>
                                        <CardDescription>
                                            "Bridges native RusToK conversations with external Matrix/Element homeservers"
                                        </CardDescription>
                                    </CardHeader>
                                    <CardContent>
                                        <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
                                            <div class="rounded-lg border border-border p-4 bg-muted/30">
                                                <div class="text-xs text-muted-foreground">{lbl.federation_server.clone()}</div>
                                                <div class="font-mono font-medium text-base mt-1">"rustok.org"</div>
                                            </div>
                                            <div class="rounded-lg border border-border p-4 bg-muted/30">
                                                <div class="text-xs text-muted-foreground">{lbl.federation_rooms.clone()}</div>
                                                <div class="font-mono font-medium text-base mt-1">"0 Active Bridges"</div>
                                            </div>
                                        </div>
                                    </CardContent>
                                </Card>
                            }.into_any()
                        }
                    }
                }
            }
        </div>
    }
}
