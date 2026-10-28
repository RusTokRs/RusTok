use rustok_navigation_storefront::StorefrontNavigationSnapshot;
use rustok_pages_storefront::{StorefrontPageRouteDecision, StorefrontPageRouteDisposition};
use serde::Serialize;
use sha2::{Digest, Sha256};

use super::seo_page_context::ResolvedSeoPageContext;

pub const PAGES_STOREFRONT_COMPOSITION_FORMAT: &str = "pages_storefront_composition_v1";
pub const PAGES_STOREFRONT_REVALIDATE_CACHE_CONTROL: &str = "private, no-cache";

#[derive(Serialize)]
struct PagesCompositionPayload<'a> {
    format: &'static str,
    locale: &'a str,
    canonical_page_id: &'a str,
    canonical_slug: &'a str,
    canonical_locale: &'a str,
    channel_id: Option<&'a str>,
    route_generation: u64,
    page_generation: u64,
    artifact_generation: u64,
    rendered_html_hash: String,
    seo: &'a Option<ResolvedSeoPageContext>,
    navigation: &'a StorefrontNavigationSnapshot,
}

pub fn pages_storefront_composition_etag(
    locale: &str,
    decision: &StorefrontPageRouteDecision,
    seo: &Option<ResolvedSeoPageContext>,
    navigation: &StorefrontNavigationSnapshot,
    rendered_html: &str,
) -> Option<String> {
    if decision.disposition != StorefrontPageRouteDisposition::Canonical
        || rendered_html.contains(" nonce=\"")
    {
        return None;
    }
    let payload = PagesCompositionPayload {
        format: PAGES_STOREFRONT_COMPOSITION_FORMAT,
        locale: locale.trim(),
        canonical_page_id: decision.canonical_page_id.as_deref()?,
        canonical_slug: decision.canonical_slug.as_deref()?,
        canonical_locale: decision.canonical_locale.as_deref()?,
        channel_id: decision.channel_id.as_deref(),
        route_generation: decision.route_generation?,
        page_generation: decision.page_generation?,
        artifact_generation: decision.artifact_generation?,
        rendered_html_hash: hex_digest(&Sha256::digest(rendered_html.as_bytes())),
        seo,
        navigation,
    };
    let encoded = serde_json::to_vec(&payload).ok()?;
    let digest = Sha256::digest(encoded);
    Some(format!(
        "\"{PAGES_STOREFRONT_COMPOSITION_FORMAT}-{}\"",
        hex_digest(&digest)
    ))
}

pub fn if_none_match_matches(if_none_match: Option<&str>, etag: &str) -> bool {
    if_none_match.is_some_and(|value| {
        value.split(',').map(str::trim).any(|candidate| {
            candidate == "*" || candidate == etag || candidate.strip_prefix("W/") == Some(etag)
        })
    })
}

fn hex_digest(digest: &[u8]) -> String {
    use std::fmt::Write as _;

    let mut encoded = String::with_capacity(digest.len() * 2);
    for byte in digest {
        write!(&mut encoded, "{byte:02x}").expect("writing to a String cannot fail");
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustok_navigation_storefront::{
        StorefrontMenu, StorefrontMenuItem, StorefrontMenuLocation,
    };

    fn decision() -> StorefrontPageRouteDecision {
        StorefrontPageRouteDecision {
            disposition: StorefrontPageRouteDisposition::Canonical,
            canonical_path: Some("/en/modules/pages?slug=about".to_string()),
            canonical_slug: Some("about".to_string()),
            canonical_page_id: Some("00000000-0000-0000-0000-000000000001".to_string()),
            canonical_locale: Some("en".to_string()),
            channel_id: Some("00000000-0000-0000-0000-000000000002".to_string()),
            route_generation: Some(3),
            page_generation: Some(5),
            artifact_generation: Some(7),
        }
    }

    fn navigation(title: &str) -> StorefrontNavigationSnapshot {
        StorefrontNavigationSnapshot {
            header: Some(StorefrontMenu {
                id: "header".to_string(),
                effective_locale: "en".to_string(),
                name: "Header".to_string(),
                location: StorefrontMenuLocation::Header,
                items: vec![StorefrontMenuItem {
                    id: "about".to_string(),
                    title: title.to_string(),
                    url: "/en/modules/pages?slug=about".to_string(),
                    icon: None,
                    children: Vec::new(),
                }],
            }),
            footer: None,
        }
    }

    #[test]
    fn composition_etag_is_stable_and_binds_every_dependency() {
        let base = pages_storefront_composition_etag(
            "en",
            &decision(),
            &None,
            &navigation("About"),
            "<html>about</html>",
        )
        .expect("complete canonical decision should produce an ETag");
        assert_eq!(
            base,
            pages_storefront_composition_etag(
                "en",
                &decision(),
                &None,
                &navigation("About"),
                "<html>about</html>",
            )
            .expect("same composition should produce the same ETag")
        );

        let mut generation_changed = decision();
        generation_changed.page_generation = Some(6);
        assert_ne!(
            base,
            pages_storefront_composition_etag(
                "en",
                &generation_changed,
                &None,
                &navigation("About"),
                "<html>about</html>",
            )
            .expect("changed generation should still produce an ETag")
        );
        assert_ne!(
            base,
            pages_storefront_composition_etag(
                "en",
                &decision(),
                &None,
                &navigation("Company"),
                "<html>about</html>",
            )
            .expect("changed menu should still produce an ETag")
        );
        assert_ne!(
            base,
            pages_storefront_composition_etag(
                "en",
                &decision(),
                &None,
                &navigation("About"),
                "<html>company</html>",
            )
            .expect("changed rendered HTML should still produce an ETag")
        );

        let mut seo = ResolvedSeoPageContext::default();
        seo.document.title = "About".to_string();
        assert_ne!(
            base,
            pages_storefront_composition_etag(
                "en",
                &decision(),
                &Some(seo),
                &navigation("About"),
                "<html>about</html>",
            )
            .expect("changed SEO should still produce an ETag")
        );
    }

    #[test]
    fn incomplete_terminal_or_nonce_bearing_documents_do_not_claim_cache_identity() {
        let mut incomplete = decision();
        incomplete.route_generation = None;
        assert!(
            pages_storefront_composition_etag(
                "en",
                &incomplete,
                &None,
                &navigation("About"),
                "<html>about</html>",
            )
            .is_none()
        );

        let mut terminal = decision();
        terminal.disposition = StorefrontPageRouteDisposition::Gone;
        assert!(
            pages_storefront_composition_etag(
                "en",
                &terminal,
                &None,
                &navigation("About"),
                "<html>about</html>",
            )
            .is_none()
        );

        assert!(
            pages_storefront_composition_etag(
                "en",
                &decision(),
                &None,
                &navigation("About"),
                "<html><script nonce=\"request-specific\"></script></html>",
            )
            .is_none()
        );
    }

    #[test]
    fn conditional_request_accepts_strong_weak_and_list_matches() {
        let etag = "\"pages_storefront_composition_v1-deadbeef\"";
        assert!(if_none_match_matches(Some(etag), etag));
        assert!(if_none_match_matches(
            Some("W/\"pages_storefront_composition_v1-deadbeef\""),
            etag
        ));
        assert!(if_none_match_matches(
            Some("\"other\", W/\"pages_storefront_composition_v1-deadbeef\""),
            etag
        ));
        assert!(!if_none_match_matches(Some("\"other\""), etag));
    }
}
