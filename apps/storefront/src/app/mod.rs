use crate::modules::{components_for_slot, StorefrontSlot};
use crate::pages::home::HomePage;
use crate::shared::local::{featured_products, locale_strings};
use crate::widgets::footer::Footer;
use crate::widgets::header::Header;
use leptos::prelude::*;

#[component]
pub fn StorefrontShell(locale: String) -> impl IntoView {
    let strings = locale_strings(locale.as_str());
    let products = featured_products(locale.as_str());
    let module_views = components_for_slot(StorefrontSlot::HomeAfterHero)
        .into_iter()
        .map(|module| (module.render)())
        .collect::<Vec<_>>();

    view! {
        <div class="min-h-screen bg-background text-foreground">
            <Header
                nav_home=strings.nav_home
                nav_catalog=strings.nav_catalog
                nav_about=strings.nav_about
                nav_contact=strings.nav_contact
                nav_language=strings.nav_language
                cta_primary=strings.cta_primary
            />

            <HomePage
                hero_title=strings.hero_title
                hero_subtitle=strings.hero_subtitle
                cta_primary=strings.cta_primary
                cta_secondary:strings.cta_secondary
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
                module_views=module_views
            />

            <Footer tagline=strings.footer_tagline />
        </div>
    }
}
