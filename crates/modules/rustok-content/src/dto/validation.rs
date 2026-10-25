/// Input validation for content module DTOs
///
/// Provides validation rules and custom validators for all content-related inputs.
///
/// FIXED: Added i18n support for error messages
use rustok_api::TenantLocale;
use validator::ValidationError;

/// Every Content node kind: the write-side allowlist and the Search corpus declaration.
///
/// The Search projector writes `source_module = node.kind` (see
/// `rustok-search::projector_core`), so this list is exactly the set of `source_module`
/// values the Content module can place into `search_documents`. The host declares every
/// entry as public discovery material (`apps/server::search_public_source_modules`).
///
/// Keep this list closed: a kind that is absent is rejected on write, so it can never
/// reach the index and silently disappear from the public corpus. Adding a kind here is
/// therefore the only way to introduce a new Content document class.
pub const CONTENT_NODE_KINDS: &[&str] = &[
    // Core CMS kinds
    "post",
    "page",
    "article",
    "custom",
    "block",
    "menu",
    "menu_item",
    // Forum domain kinds
    "forum_category",
    "forum_topic",
    "forum_reply",
    // Blog domain kinds
    "blog_post",
    "comment",
    "category",
    "tag",
];

/// Custom validator for kind
///
/// Accepts exactly [`CONTENT_NODE_KINDS`]. The list is shared with the Search corpus
/// declaration so validation and public discoverability cannot drift apart.
pub fn validate_kind(kind: &str) -> Result<(), ValidationError> {
    if CONTENT_NODE_KINDS.contains(&kind) {
        Ok(())
    } else {
        Err(ValidationError::new("invalid_kind"))
    }
}

#[cfg(test)]
mod tests {
    use super::{CONTENT_NODE_KINDS, validate_kind};

    #[test]
    fn every_declared_kind_is_accepted() {
        for kind in CONTENT_NODE_KINDS {
            validate_kind(kind).expect("declared node kinds must validate");
        }
    }

    #[test]
    fn every_accepted_kind_is_declared_in_the_search_corpus() {
        // The Search projector writes `source_module = kind`, so an accepted kind that is
        // missing from the declaration would be indexed and then denied in public Search.
        for kind in ["post", "page", "article", "custom", "block", "menu", "menu_item"] {
            assert!(CONTENT_NODE_KINDS.contains(&kind));
        }
        assert!(validate_kind("secret_lab_note").is_err());
        assert!(validate_kind("").is_err());
        assert!(validate_kind("POST").is_err());
    }

    #[test]
    fn declared_kinds_are_unique_and_normalized() {
        let mut seen = std::collections::BTreeSet::new();
        for kind in CONTENT_NODE_KINDS {
            assert!(seen.insert(*kind), "duplicate Content node kind: {kind}");
            assert!(
                kind.chars()
                    .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_'),
                "Content node kinds feed Search source-module names: {kind}"
            );
        }
    }
}

/// Validate a tenant-localized content locale through the platform locale contract.
///
/// This intentionally shares the same accepted BCP47 subset and `und` rejection
/// semantics as every other tenant/runtime locale boundary.
pub fn validate_locale(locale: &str) -> Result<(), ValidationError> {
    TenantLocale::new(locale)
        .map(|_| ())
        .map_err(|_| ValidationError::new("invalid_locale_format"))
}

/// Custom validator for position (should be non-negative)
pub fn validate_position(position: &i32) -> Result<(), ValidationError> {
    if *position < 0 {
        return Err(ValidationError::new("position_must_be_non_negative"));
    }
    if *position > 100000 {
        return Err(ValidationError::new("position_too_large"));
    }
    Ok(())
}

/// Custom validator for depth (should be reasonable)
pub fn validate_depth(depth: &i32) -> Result<(), ValidationError> {
    if *depth < 0 {
        return Err(ValidationError::new("depth_must_be_non_negative"));
    }
    if *depth > 100 {
        return Err(ValidationError::new("depth_too_large"));
    }
    Ok(())
}

/// Custom validator for reply count
pub fn validate_reply_count(count: &i32) -> Result<(), ValidationError> {
    if *count < 0 {
        return Err(ValidationError::new("reply_count_must_be_non_negative"));
    }
    Ok(())
}

/// Custom validator for slug format
///
/// FIXED: More comprehensive slug validation
pub fn validate_slug(slug: &str) -> Result<(), ValidationError> {
    // Slug should be lowercase alphanumeric with hyphens
    if slug.is_empty() {
        return Err(ValidationError::new("slug_empty"));
    }

    if slug.len() > 255 {
        return Err(ValidationError::new("slug_too_long"));
    }

    // Check if slug matches pattern: lowercase letters, numbers, hyphens
    if !slug
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return Err(ValidationError::new("slug_invalid_characters"));
    }

    // Should not start or end with hyphen
    if slug.starts_with('-') || slug.ends_with('-') {
        return Err(ValidationError::new("slug_hyphen_boundary"));
    }

    // Should not contain consecutive hyphens
    if slug.contains("--") {
        return Err(ValidationError::new("slug_consecutive_hyphens"));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_kind_valid() {
        assert!(validate_kind("post").is_ok());
        assert!(validate_kind("page").is_ok());
        assert!(validate_kind("article").is_ok());
        assert!(validate_kind("custom").is_ok());
        assert!(validate_kind("block").is_ok());
        assert!(validate_kind("menu").is_ok());
        assert!(validate_kind("menu_item").is_ok());
        assert!(validate_kind("blog_post").is_ok());
        assert!(validate_kind("comment").is_ok());
        assert!(validate_kind("category").is_ok());
        assert!(validate_kind("tag").is_ok());
    }

    #[test]
    fn test_validate_kind_invalid() {
        assert!(validate_kind("invalid").is_err());
        assert!(validate_kind("").is_err());
    }

    #[test]
    fn test_validate_locale_valid() {
        assert!(validate_locale("en").is_ok());
        assert!(validate_locale("ru").is_ok());
        assert!(validate_locale("en-US").is_ok());
        assert!(validate_locale("ru-RU").is_ok());
        assert!(validate_locale("zh-CN").is_ok());
        assert!(validate_locale("pt-BR").is_ok());
        assert!(validate_locale("es-419").is_ok());
        assert!(validate_locale("zh-Hant-TW").is_ok());
        assert!(validate_locale("pt_BR").is_ok());
        assert!(validate_locale("en-US-extra").is_ok());
    }

    #[test]
    fn test_validate_locale_invalid() {
        assert!(validate_locale("e").is_err());
        assert!(validate_locale("toolonglocale").is_err());
        assert!(validate_locale("en123").is_err());
        assert!(validate_locale("en-").is_err());
        assert!(validate_locale("-US").is_err());
        assert!(validate_locale("en-1").is_err());
        assert!(validate_locale("en-U1").is_err());
        assert!(validate_locale("und").is_err());
    }

    #[test]
    fn test_validate_position_valid() {
        assert!(validate_position(&0).is_ok());
        assert!(validate_position(&100).is_ok());
        assert!(validate_position(&10000).is_ok());
        assert!(validate_position(&100000).is_ok());
    }

    #[test]
    fn test_validate_position_invalid() {
        assert!(validate_position(&-1).is_err());
        assert!(validate_position(&-100).is_err());
        assert!(validate_position(&100001).is_err());
    }

    #[test]
    fn test_validate_depth_valid() {
        assert!(validate_depth(&0).is_ok());
        assert!(validate_depth(&5).is_ok());
        assert!(validate_depth(&50).is_ok());
        assert!(validate_depth(&100).is_ok());
    }

    #[test]
    fn test_validate_depth_invalid() {
        assert!(validate_depth(&-1).is_err());
        assert!(validate_depth(&101).is_err());
        assert!(validate_depth(&1000).is_err());
    }

    #[test]
    fn test_validate_slug_valid() {
        assert!(validate_slug("my-post").is_ok());
        assert!(validate_slug("hello-world").is_ok());
        assert!(validate_slug("post-123").is_ok());
        assert!(validate_slug("a").is_ok());
        assert!(validate_slug("2024-news").is_ok());
    }

    #[test]
    fn test_validate_slug_invalid() {
        assert!(validate_slug("").is_err()); // Empty
        assert!(validate_slug("My-Post").is_err()); // Uppercase
        assert!(validate_slug("my_post").is_err()); // Underscore
        assert!(validate_slug("-mypost").is_err()); // Starts with hyphen
        assert!(validate_slug("mypost-").is_err()); // Ends with hyphen
        assert!(validate_slug("my post").is_err()); // Space
        assert!(validate_slug("my--post").is_err()); // Consecutive hyphens
        assert!(validate_slug(&"a".repeat(256)).is_err()); // Too long
    }

    #[test]
    fn test_validate_reply_count() {
        assert!(validate_reply_count(&0).is_ok());
        assert!(validate_reply_count(&100).is_ok());
        assert!(validate_reply_count(&-1).is_err());
    }
}
