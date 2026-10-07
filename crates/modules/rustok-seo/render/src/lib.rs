//! Compatibility re-export crate for `rustok-seo-storefront`.
//!
//! Canonical head rendering and SEO storefront logic now resides in `rustok-seo-storefront`.

pub use rustok_seo_storefront::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reexport_render_head_html() {
        let context = SeoPageContext::default();
        let head = render_head_html(&context);
        assert!(head.is_empty() || head.contains("<meta") || head.contains("<link"));
    }
}
