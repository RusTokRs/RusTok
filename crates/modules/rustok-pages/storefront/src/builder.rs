use crate::model::PublishedLanding;
use leptos::prelude::*;
use rustok_page_builder_storefront::StaticLandingStorefront;

const PUBLISHED_LANDING_CLASS: &str = "rustok-pages-storefront__builder-body";

/// Renders the published landing of a page inline, without an iframe.
///
/// The markup and stylesheet come from the published landing artifact, so they are the output
/// of the reviewed publish. `StaticLandingStorefront` scopes the stylesheet to the landing
/// element and attaches the Fly action runtime when the markup uses it.
#[component]
pub fn PublishedLandingBody(
    landing: PublishedLanding,
    #[prop(optional)] class: Option<String>,
) -> impl IntoView {
    let class = class.unwrap_or_else(|| PUBLISHED_LANDING_CLASS.to_string());
    view! {
        <StaticLandingStorefront
            html=landing.html
            css=landing.css
            class=class
        />
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn published_landing_body_constructs_view() {
        let landing = PublishedLanding {
            locale: "en".to_string(),
            html: "<h1>Landing</h1>".to_string(),
            css: "h1 { color: red; }".to_string(),
            artifact_hash: "hash".to_string(),
        };
        let _view = view! { <PublishedLandingBody landing=landing /> };
    }
}
