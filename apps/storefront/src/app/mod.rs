use crate::modules::{
    StorefrontPageLookup, StorefrontSlot, components_for_slot, init_modules, page_for_route_segment,
};
use crate::pages::home::HomePage;
use crate::shared::context::enabled_modules::{EnabledModulesProvider, use_enabled_modules};
use crate::shared::context::module_request::ModuleRequestProvider;
use crate::shared::local::{featured_products, locale_strings};
use crate::widgets::footer::Footer;
use crate::widgets::header::Header;
use leptos::prelude::*;
use std::collections::HashMap;

#[component]
fn StorefrontLayout(locale: String, body: AnyView) -> impl IntoView {
    let strings = locale_strings(locale.as_str());
    let enabled_modules = use_enabled_modules().get_untracked();
    let header_navigation_views =
        components_for_slot(StorefrontSlot::HeaderNavigation, Some(&enabled_modules))
            .into_iter()
            .map(|module| (module.render)())
            .collect::<Vec<_>>();
    let footer_navigation_views =
        components_for_slot(StorefrontSlot::FooterNavigation, Some(&enabled_modules))
            .into_iter()
            .map(|module| (module.render)())
            .collect::<Vec<_>>();

    view! {
        <div class="min-h-screen bg-background text-foreground">
            <Header
                locale=locale.clone()
                nav_home=strings.nav_home
                nav_catalog=strings.nav_catalog
                nav_about=strings.nav_about
                nav_contact=strings.nav_contact
                nav_language=strings.nav_language
                cta_primary=strings.cta_primary
                navigation_views=header_navigation_views
            />

            {body}

            <Footer
                tagline=strings.footer_tagline
                navigation_views=footer_navigation_views
            />
        </div>
    }
}

#[component]
fn StorefrontShellContent(locale: String) -> impl IntoView {
    let strings = locale_strings(locale.as_str());
    let products = featured_products(locale.as_str());
    let enabled_modules = use_enabled_modules().get_untracked();
    let after_hero_views =
        components_for_slot(StorefrontSlot::HomeAfterHero, Some(&enabled_modules))
            .into_iter()
            .map(|module| (module.render)())
            .collect::<Vec<_>>();
    let after_catalog_views =
        components_for_slot(StorefrontSlot::HomeAfterCatalog, Some(&enabled_modules))
            .into_iter()
            .map(|module| (module.render)())
            .collect::<Vec<_>>();
    let before_footer_views =
        components_for_slot(StorefrontSlot::HomeBeforeFooter, Some(&enabled_modules))
            .into_iter()
            .map(|module| (module.render)())
            .collect::<Vec<_>>();

    let body = view! {
        <HomePage
            hero_title=strings.hero_title
            hero_subtitle=strings.hero_subtitle
            cta_primary=strings.cta_primary
            cta_secondary=strings.cta_secondary
            featured_title=strings.featured_title
            featured_subtitle=strings.featured_subtitle
            products=products
            badge_new=strings.badge_new
            cta_view=strings.cta_view
            story_title=strings.story_title
            story_body=strings.story_body
            newsletter_title=strings.newsletter_title
            newsletter_body=strings.newsletter_body
            newsletter_placeholder=strings.newsletter_placeholder
            newsletter_cta=strings.newsletter_cta
            newsletter_note=strings.newsletter_note
            after_hero_views=after_hero_views
            after_catalog_views=after_catalog_views
            before_footer_views=before_footer_views
        />
    }
    .into_any();

    view! { <StorefrontLayout locale=locale body=body /> }
}

#[component]
fn StorefrontModulePageContent(locale: String, route_segment: String) -> impl IntoView {
    let enabled_modules = use_enabled_modules().get_untracked();
    let body = match page_for_route_segment(route_segment.as_str(), Some(&enabled_modules)) {
        StorefrontPageLookup::Found(page) => {
            view! {
                <main class="container-app px-4 py-16">
                    <section class="mx-auto max-w-5xl space-y-6">
                        <div class="space-y-2">
                            <p class="text-xs font-semibold uppercase tracking-[0.24em] text-muted-foreground">
                                {page.module_slug}
                            </p>
                            <h1 class="text-4xl font-bold text-foreground">{page.title}</h1>
                        </div>
                        {(page.render)()}
                    </section>
                </main>
            }
            .into_any()
        }
        StorefrontPageLookup::Disabled => view! {
            <main class="container-app px-4 py-16">
                <section class="mx-auto max-w-3xl rounded-2xl border border-border bg-card p-8 shadow-sm">
                    <h1 class="text-3xl font-bold text-card-foreground">"Module unavailable"</h1>
                    <p class="mt-3 text-sm text-muted-foreground">
                        "This storefront page belongs to a module that is not enabled for the current tenant."
                    </p>
                </section>
            </main>
        }
        .into_any(),
        StorefrontPageLookup::Missing => view! {
            <main class="container-app px-4 py-16">
                <section class="mx-auto max-w-3xl rounded-2xl border border-border bg-card p-8 shadow-sm">
                    <h1 class="text-3xl font-bold text-card-foreground">"Page not found"</h1>
                    <p class="mt-3 text-sm text-muted-foreground">
                        "The requested module page is not registered in the storefront host."
                    </p>
                </section>
            </main>
        }
        .into_any(),
    };

    view! { <StorefrontLayout locale=locale body=body /> }
}

#[component]
pub fn StorefrontShell(
    locale: String,
    enabled_modules: Vec<String>,
    query_params: HashMap<String, String>,
) -> impl IntoView {
    init_modules();

    view! {
        <EnabledModulesProvider initial_modules=enabled_modules>
            <ModuleRequestProvider
                locale=Some(locale.clone())
                route_segment=None
                subpath=None
                query_params=query_params
            >
                <StorefrontShellContent locale=locale />
            </ModuleRequestProvider>
        </EnabledModulesProvider>
    }
}

#[component]
pub fn StorefrontModulePage(
    locale: String,
    enabled_modules: Vec<String>,
    route_segment: String,
    query_params: HashMap<String, String>,
) -> impl IntoView {
    init_modules();

    view! {
        <EnabledModulesProvider initial_modules=enabled_modules>
            <ModuleRequestProvider
                locale=Some(locale.clone())
                route_segment=Some(route_segment.clone())
                subpath=None
                query_params=query_params
            >
                <StorefrontModulePageContent locale=locale route_segment=route_segment />
            </ModuleRequestProvider>
        </EnabledModulesProvider>
    }
}
