use leptos::ev::SubmitEvent;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_ui_routing::{use_route_query_value, use_route_query_writer};
use rustok_ui_core::{AdminQueryKey, UiRouteContext};
use uuid::Uuid;

use crate::core::{
    SeoAdminBusyKey, SeoAdminTab, SeoBulkActionForm, SeoBulkFilterForm, SeoIndexReplayForm,
    SeoRedirectForm, SeoSettingsForm, format_index_repair_replay_result,
    validate_sitemap_generation_enabled,
};
use crate::sections::{
    SeoAdminHeader, SeoAdminTabs, SeoBulkPane, SeoBusyFooter, SeoDefaultsPane, SeoDiagnosticsPane,
    SeoIndexPane, SeoRedirectsPane, SeoRobotsPane, SeoSitemapsPane,
};
use crate::transport;

#[component]
pub fn SeoAdmin() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let ui_locale = StoredValue::new(route_context.locale.clone());
    let tab_query = use_route_query_value(AdminQueryKey::Tab.as_str());
    let query_writer = use_route_query_writer();

    let redirect_form = RwSignal::new(SeoRedirectForm::default());
    let settings_form = RwSignal::new(SeoSettingsForm::default());
    let bulk_filter_form = RwSignal::new(SeoBulkFilterForm::new(route_context.locale.as_deref()));
    let bulk_action_form = RwSignal::new(SeoBulkActionForm::default());
    let index_replay_form = RwSignal::new(SeoIndexReplayForm::default());
    let bulk_selected_ids = RwSignal::new(Vec::<Uuid>::new());
    let bulk_selection_preview = RwSignal::new(None::<i32>);
    let busy_key = RwSignal::new(Option::<String>::None);
    let status_message = RwSignal::new(Option::<String>::None);
    let redirects_nonce = RwSignal::new(0_u64);
    let settings_nonce = RwSignal::new(0_u64);
    let sitemap_nonce = RwSignal::new(0_u64);
    let index_nonce = RwSignal::new(0_u64);
    let bulk_nonce = RwSignal::new(0_u64);
    let bulk_jobs_nonce = RwSignal::new(0_u64);
    let diagnostics_locale = route_context.locale.clone();

    let redirects = Resource::new(
        move || redirects_nonce.get(),
        move |_| async move { transport::fetch_redirects().await },
    );
    let settings = Resource::new(
        move || settings_nonce.get(),
        move |_| async move { transport::fetch_settings().await },
    );
    let robots_preview = Resource::new(
        move || settings_nonce.get(),
        move |_| async move { transport::fetch_robots_preview().await },
    );
    let diagnostics = Resource::new(
        move || settings_nonce.get(),
        move |_| {
            let locale = diagnostics_locale.clone();
            async move { transport::fetch_diagnostics(locale).await }
        },
    );
    let sitemap_status = Resource::new(
        move || sitemap_nonce.get(),
        move |_| async move { transport::fetch_sitemap_status().await },
    );
    let index_status = Resource::new(
        move || {
            (
                index_nonce.get(),
                index_replay_form.get().status_target_type(),
            )
        },
        move |(_, target_type)| async move { transport::fetch_index_delivery_status(target_type).await },
    );
    let bulk_items = Resource::new(
        move || bulk_nonce.get(),
        move |_| async move {
            match bulk_filter_form.get_untracked().build_input() {
                Ok(input) => transport::fetch_bulk_items(input).await,
                Err(err) => Err(transport::ApiError::ServerFn(err)),
            }
        },
    );
    let bulk_targets = Resource::new(
        || (),
        move |_| async move { transport::fetch_bulk_targets().await },
    );
    let bulk_jobs = Resource::new(
        move || bulk_jobs_nonce.get(),
        move |_| async move { transport::fetch_bulk_jobs(Some(20), None).await },
    );
    let active_tab = Signal::derive(move || {
        tab_query
            .get()
            .as_deref()
            .and_then(SeoAdminTab::from_str)
            .unwrap_or(SeoAdminTab::Index)
    });

    Effect::new(move |_| {
        if let Some(Ok(settings_value)) = settings.get() {
            settings_form.set(SeoSettingsForm::from_settings(&settings_value));
        }
    });

    Effect::new(move |_| {
        if let Some(Ok(entries)) = bulk_targets.get() {
            let Some(first) = entries.first() else {
                return;
            };
            let current_target = bulk_filter_form.get().target_kind.clone();
            if entries.iter().any(|entry| entry.slug == current_target) {
                return;
            }

            bulk_filter_form.update(|draft| {
                draft.target_kind = first.slug.clone();
                draft.page = 1;
            });
            bulk_selected_ids.set(Vec::new());
            bulk_selection_preview.set(None);
        }
    });

    let save_redirect = Callback::new(move |ev: SubmitEvent| {
        ev.prevent_default();
        status_message.set(None);
        let input = match redirect_form.get_untracked().build_input() {
            Ok(input) => input,
            Err(err) => {
                status_message.set(Some(err));
                return;
            }
        };

        busy_key.set(Some(SeoAdminBusyKey::SaveRedirect.to_busy_key()));
        spawn_local(async move {
            match transport::save_redirect(input).await {
                Ok(_) => {
                    status_message.set(Some("Redirect saved".to_string()));
                    redirects_nonce.update(|value| *value += 1);
                }
                Err(err) => status_message.set(Some(err.to_string())),
            }
            busy_key.set(None);
        });
    });

    let save_settings = Callback::new(move |ev: SubmitEvent| {
        ev.prevent_default();
        status_message.set(None);
        let input = settings_form.get_untracked().build_settings();

        busy_key.set(Some(SeoAdminBusyKey::SaveSettings.to_busy_key()));
        spawn_local(async move {
            match transport::save_settings(input).await {
                Ok(saved) => {
                    settings_form.set(SeoSettingsForm::from_settings(&saved));
                    status_message.set(Some("SEO defaults saved".to_string()));
                    settings_nonce.update(|value| *value += 1);
                    sitemap_nonce.update(|value| *value += 1);
                }
                Err(err) => status_message.set(Some(err.to_string())),
            }
            busy_key.set(None);
        });
    });

    let generate_sitemaps = Callback::new(move |_| {
        status_message.set(None);
        let current_status = sitemap_status.get_untracked();
        if let Err(err) = validate_sitemap_generation_enabled(
            current_status
                .as_ref()
                .and_then(|status| status.as_ref().ok()),
        ) {
            status_message.set(Some(err));
            return;
        }

        busy_key.set(Some(SeoAdminBusyKey::GenerateSitemaps.to_busy_key()));
        spawn_local(async move {
            match transport::generate_sitemaps().await {
                Ok(_) => {
                    status_message.set(Some("Sitemaps generated".to_string()));
                    sitemap_nonce.update(|value| *value += 1);
                }
                Err(err) => status_message.set(Some(err.to_string())),
            }
            busy_key.set(None);
        });
    });

    let refresh_index = Callback::new(move |_| {
        index_nonce.update(|value| *value += 1);
    });

    let run_index_repair_only = Callback::new(move |_| {
        status_message.set(None);
        let input = match index_replay_form
            .get_untracked()
            .build_confirmed_input(false)
        {
            Ok(input) => input,
            Err(err) => {
                status_message.set(Some(err));
                return;
            }
        };

        busy_key.set(Some(SeoAdminBusyKey::IndexRepairOnly.to_busy_key()));
        spawn_local(async move {
            match transport::run_index_repair_replay(input).await {
                Ok(result) => {
                    status_message.set(Some(format_index_repair_replay_result(&result, false)));
                    index_nonce.update(|value| *value += 1);
                }
                Err(err) => status_message.set(Some(err.to_string())),
            }
            busy_key.set(None);
        });
    });

    let run_index_repair_replay = Callback::new(move |_| {
        status_message.set(None);
        let input = match index_replay_form
            .get_untracked()
            .build_confirmed_input(true)
        {
            Ok(input) => input,
            Err(err) => {
                status_message.set(Some(err));
                return;
            }
        };

        busy_key.set(Some(SeoAdminBusyKey::IndexRepairReplay.to_busy_key()));
        spawn_local(async move {
            match transport::run_index_repair_replay(input).await {
                Ok(result) => {
                    status_message.set(Some(format_index_repair_replay_result(&result, true)));
                    index_nonce.update(|value| *value += 1);
                }
                Err(err) => status_message.set(Some(err.to_string())),
            }
            busy_key.set(None);
        });
    });

    let refresh_bulk = Callback::new(move |_| {
        bulk_selected_ids.set(Vec::new());
        bulk_selection_preview.set(None);
        bulk_nonce.update(|value| *value += 1);
    });

    let preview_bulk_selection = Callback::new(move |_| {
        status_message.set(None);
        let filter = match bulk_filter_form.get_untracked().build_input() {
            Ok(input) => input,
            Err(err) => {
                status_message.set(Some(err));
                return;
            }
        };
        let selection = bulk_action_form
            .get_untracked()
            .build_selection(filter, &bulk_selected_ids.get_untracked());

        busy_key.set(Some(SeoAdminBusyKey::PreviewBulkSelection.to_busy_key()));
        spawn_local(async move {
            match transport::preview_bulk_selection(selection).await {
                Ok(preview) => bulk_selection_preview.set(Some(preview.count)),
                Err(err) => status_message.set(Some(err.to_string())),
            }
            busy_key.set(None);
        });
    });

    let queue_bulk_apply = Callback::new(move |_| {
        status_message.set(None);
        let filter = match bulk_filter_form.get_untracked().build_input() {
            Ok(input) => input,
            Err(err) => {
                status_message.set(Some(err));
                return;
            }
        };
        let action = bulk_action_form.get_untracked();
        let selected_ids = bulk_selected_ids.get_untracked();
        let input = match action.build_apply_input(filter, &selected_ids) {
            Ok(input) => input,
            Err(err) => {
                status_message.set(Some(err));
                return;
            }
        };

        busy_key.set(Some(SeoAdminBusyKey::QueueBulkApply.to_busy_key()));
        spawn_local(async move {
            match transport::preview_bulk_selection(input.selection.clone()).await {
                Ok(preview) if preview.count > 0 => {
                    match transport::queue_bulk_apply(input).await {
                        Ok(job) => {
                            bulk_selection_preview.set(Some(preview.count));
                            status_message.set(Some(format!("Queued bulk apply job {}", job.id)));
                            bulk_jobs_nonce.update(|value| *value += 1);
                        }
                        Err(err) => status_message.set(Some(err.to_string())),
                    }
                }
                Ok(_) => status_message.set(Some("Bulk selection is empty".to_string())),
                Err(err) => status_message.set(Some(err.to_string())),
            }
            busy_key.set(None);
        });
    });

    let queue_bulk_export = Callback::new(move |_| {
        status_message.set(None);
        let filter = match bulk_filter_form.get_untracked().build_input() {
            Ok(input) => input,
            Err(err) => {
                status_message.set(Some(err));
                return;
            }
        };
        let input = bulk_action_form.get_untracked().build_export_input(filter);

        busy_key.set(Some(SeoAdminBusyKey::QueueBulkExport.to_busy_key()));
        spawn_local(async move {
            match transport::queue_bulk_export(input).await {
                Ok(job) => {
                    status_message.set(Some(format!("Queued bulk export job {}", job.id)));
                    bulk_jobs_nonce.update(|value| *value += 1);
                }
                Err(err) => status_message.set(Some(err.to_string())),
            }
            busy_key.set(None);
        });
    });

    let queue_bulk_import = Callback::new(move |_| {
        status_message.set(None);
        let filter = match bulk_filter_form.get_untracked().build_input() {
            Ok(input) => input,
            Err(err) => {
                status_message.set(Some(err));
                return;
            }
        };
        let input = match bulk_action_form.get_untracked().build_import_input(&filter) {
            Ok(input) => input,
            Err(err) => {
                status_message.set(Some(err));
                return;
            }
        };

        busy_key.set(Some(SeoAdminBusyKey::QueueBulkImport.to_busy_key()));
        spawn_local(async move {
            match transport::queue_bulk_import(input).await {
                Ok(job) => {
                    status_message.set(Some(format!("Queued bulk import job {}", job.id)));
                    bulk_jobs_nonce.update(|value| *value += 1);
                }
                Err(err) => status_message.set(Some(err.to_string())),
            }
            busy_key.set(None);
        });
    });

    let select_tab_query_writer = query_writer.clone();
    let select_tab = Callback::new(move |tab: SeoAdminTab| {
        select_tab_query_writer.replace_value(AdminQueryKey::Tab.as_str(), tab.as_str());
    });

    let queue_schema_fix = Callback::new(
        move |(target_kind, apply_mode, payload): (
            rustok_seo_targets::SeoTargetSlug,
            rustok_seo::SeoBulkApplyMode,
            String,
        )| {
            let select_tab = select_tab;
            select_tab.run(SeoAdminTab::Bulk);
            bulk_action_form.update(|draft| {
                draft.prefill_schema_fix(apply_mode, payload);
            });
            bulk_filter_form.update(|draft| {
                draft.target_kind = target_kind;
            });
            status_message.set(Some(
                "Pre-filled bulk action for schema fix. Review and click Queue apply.".to_string(),
            ));
        },
    );

    view! {
        <div class="space-y-6">
            <SeoAdminHeader ui_locale=ui_locale.get_value() status_message=status_message />
            <SeoAdminTabs
                ui_locale=ui_locale.get_value()
                active_tab=active_tab
                on_select=select_tab
            />

            <Show when=move || active_tab.get() == SeoAdminTab::Index>
                <SeoIndexPane
                    ui_locale=ui_locale.get_value()
                    index_status=index_status
                    replay_form=index_replay_form
                    busy_key=busy_key
                    on_refresh=refresh_index
                    on_run_repair_only=run_index_repair_only
                    on_run_repair_replay=run_index_repair_replay
                />
            </Show>

            <Show when=move || active_tab.get() == SeoAdminTab::Bulk>
                <SeoBulkPane
                    ui_locale=ui_locale.get_value()
                    bulk_filter_form=bulk_filter_form
                    bulk_action_form=bulk_action_form
                    bulk_selected_ids=bulk_selected_ids
                    bulk_selection_preview=bulk_selection_preview
                    bulk_targets=bulk_targets
                    bulk_items=bulk_items
                    bulk_jobs=bulk_jobs
                    busy_key=busy_key
                    on_refresh=refresh_bulk
                    on_preview_selection=preview_bulk_selection
                    on_queue_apply=queue_bulk_apply
                    on_queue_export=queue_bulk_export
                    on_queue_import=queue_bulk_import
                />
            </Show>

            <Show when=move || active_tab.get() == SeoAdminTab::Redirects>
                <SeoRedirectsPane
                    ui_locale=ui_locale.get_value()
                    redirect_form=redirect_form
                    redirects=redirects
                    busy_key=busy_key
                    on_save=save_redirect
                />
            </Show>

            <Show when=move || active_tab.get() == SeoAdminTab::Sitemaps>
                <SeoSitemapsPane
                    ui_locale=ui_locale.get_value()
                    sitemap_status=sitemap_status
                    busy_key=busy_key
                    on_generate=generate_sitemaps
                />
            </Show>

            <Show when=move || active_tab.get() == SeoAdminTab::Robots>
                <SeoRobotsPane
                    ui_locale=ui_locale.get_value()
                    robots_preview=robots_preview
                />
            </Show>

            <Show when=move || active_tab.get() == SeoAdminTab::Defaults>
                <SeoDefaultsPane
                    ui_locale=ui_locale.get_value()
                    settings_form=settings_form
                    settings=settings
                    busy_key=busy_key
                    on_save=save_settings
                />
            </Show>

            <Show when=move || active_tab.get() == SeoAdminTab::Diagnostics>
                <SeoDiagnosticsPane
                    ui_locale=ui_locale.get_value()
                    settings=settings
                    redirects=redirects
                    sitemap_status=sitemap_status
                    robots_preview=robots_preview
                    diagnostics=diagnostics
                    on_queue_schema_fix=queue_schema_fix
                />
            </Show>

            <SeoBusyFooter busy_key=busy_key />
        </div>
    }
}
