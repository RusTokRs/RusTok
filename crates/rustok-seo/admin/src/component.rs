use leptos::ev::SubmitEvent;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_ui_routing::{use_route_query_value, use_route_query_writer};
use rustok_api::{AdminQueryKey, UiRouteContext};

use crate::api;
use crate::model::{SeoAdminTab, SeoRedirectForm, SeoSettingsForm};
use crate::sections::{
    SeoAdminHeader, SeoAdminTabs, SeoBusyFooter, SeoDefaultsPane, SeoDiagnosticsPane,
    SeoRedirectsPane, SeoRobotsPane, SeoSitemapsPane,
};

#[component]
pub fn SeoAdmin() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let ui_locale = StoredValue::new(route_context.locale.clone());
    let tab_query = use_route_query_value(AdminQueryKey::Tab.as_str());
    let query_writer = use_route_query_writer();

    let redirect_form = RwSignal::new(SeoRedirectForm::default());
    let settings_form = RwSignal::new(SeoSettingsForm::default());
    let busy_key = RwSignal::new(Option::<String>::None);
    let status_message = RwSignal::new(Option::<String>::None);
    let redirects_nonce = RwSignal::new(0_u64);
    let settings_nonce = RwSignal::new(0_u64);
    let sitemap_nonce = RwSignal::new(0_u64);

    let redirects = Resource::new(
        move || redirects_nonce.get(),
        move |_| async move { api::fetch_redirects().await },
    );
    let settings = Resource::new(
        move || settings_nonce.get(),
        move |_| async move { api::fetch_settings().await },
    );
    let robots_preview = Resource::new(
        move || settings_nonce.get(),
        move |_| async move { api::fetch_robots_preview().await },
    );
    let sitemap_status = Resource::new(
        move || sitemap_nonce.get(),
        move |_| async move { api::fetch_sitemap_status().await },
    );
    let active_tab = Signal::derive(move || {
        tab_query
            .get()
            .as_deref()
            .and_then(SeoAdminTab::from_str)
            .unwrap_or(SeoAdminTab::Redirects)
    });

    Effect::new(move |_| {
        if let Some(Ok(settings_value)) = settings.get() {
            settings_form.set(SeoSettingsForm::from_settings(&settings_value));
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

        busy_key.set(Some("save-redirect".to_string()));
        spawn_local(async move {
            match api::save_redirect(input).await {
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

        busy_key.set(Some("save-settings".to_string()));
        spawn_local(async move {
            match api::save_settings(input).await {
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
        if matches!(
            sitemap_status.get_untracked(),
            Some(Ok(rustok_seo::SeoSitemapStatusRecord {
                enabled: false,
                ..
            }))
        ) {
            status_message.set(Some(
                "Sitemap generation is disabled in SEO defaults".to_string(),
            ));
            return;
        }

        busy_key.set(Some("generate-sitemaps".to_string()));
        spawn_local(async move {
            match api::generate_sitemaps().await {
                Ok(_) => {
                    status_message.set(Some("Sitemaps generated".to_string()));
                    sitemap_nonce.update(|value| *value += 1);
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
    view! {
        <div class="space-y-6">
            <SeoAdminHeader ui_locale=ui_locale.get_value() status_message=status_message />
            <SeoAdminTabs
                ui_locale=ui_locale.get_value()
                active_tab=active_tab
                on_select=select_tab
            />

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
                />
            </Show>

            <SeoBusyFooter busy_key=busy_key />
        </div>
    }
}
