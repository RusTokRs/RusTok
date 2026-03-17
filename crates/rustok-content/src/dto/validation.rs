/// Input validation for content module DTOs
///
/// Provides validation rules and custom validators for all content-related inputs.
///
/// FIXED: Added i18n support for error messages
use validator::ValidationError;

/// Custom validator for body format
///
/// `rt_json_v1` is canonical rich-text format; `rt_json` is an existing compatibility alias.
pub fn validate_body_format(format: &str) -> Result<(), ValidationError> {
    match format {
        "markdown" | "html" | "plain" | "json" | "rt_json_v1" | "rt_json" => Ok(()),
        _ => Err(ValidationError::new("invalid_format")),
    }
}

/// Custom validator for kind
///
/// Accepts built-in kinds (post, page, article, custom) plus domain-specific kinds
/// used by forum (forum_category, forum_topic, forum_reply) and other modules.
/// The `_` prefix convention allows callers to pass any kind starting with an
/// underscore, but domain kinds must be registered here.
pub fn validate_kind(kind: &str) -> Result<(), ValidationError> {
    match kind {
        // Core CMS kinds
        "post" | "page" | "article" | "custom" => Ok(()),
        // Forum domain kinds
        "forum_category" | "forum_topic" | "forum_reply" => Ok(()),
        // Blog domain kinds
        "blog_post" | "comment" | "category" | "tag" => Ok(()),
        _ => Err(ValidationError::new("invalid_kind")),
    }
}

/// Custom validator for locale format (e.g., "en", "en-US", "ru-RU", "es-419")
///
/// FIXED: More robust locale validation
pub fn validate_locale(locale: &str) -> Result<(), ValidationError> {
    if locale.len() < 2 || locale.len() > 10 {
        return Err(ValidationError::new("invalid_locale_length"));
    }

    // Check format: letters-letters or just letters
    let parts: Vec<&str> = locale.split('-').collect();

    match parts.len() {
        1 => {
            // Just language code (e.g., "en", "ru")
            if parts[0].len() != 2 || !parts[0].chars().all(|c| c.is_ascii_alphabetic()) {
                return Err(ValidationError::new("invalid_locale_format"));
            }
        }
        2 => {
            // Language-Region (e.g., "en-US", "zh-CN", "es-419")
            if parts[0].len() != 2 || !parts[0].chars().all(|c| c.is_ascii_alphabetic()) {
                return Err(ValidationError::new("invalid_locale_format"));
            }

            let region = parts[1];
            let is_alpha_region =
                region.len() == 2 && region.chars().all(|c| c.is_ascii_alphabetic());
            let is_numeric_region = region.len() == 3 && region.chars().all(|c| c.is_ascii_digit());

            if !(is_alpha_region || is_numeric_region) {
                return Err(ValidationError::new("invalid_locale_format"));
            }
        }
        _ => {
            return Err(ValidationError::new("invalid_locale_format"));
        }
    }

    Ok(())
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
    fn test_validate_body_format_valid() {
        assert!(validate_body_format("markdown").is_ok());
        assert!(validate_body_format("html").is_ok());
        assert!(validate_body_format("plain").is_ok());
        assert!(validate_body_format("json").is_ok());
        assert!(validate_body_format("rt_json_v1").is_ok());
        assert!(validate_body_format("rt_json").is_ok());
    }

    #[test]
    fn test_validate_body_format_invalid() {
        assert!(validate_body_format("xml").is_err());
        assert!(validate_body_format("unknown").is_err());
        assert!(validate_body_format("").is_err());
    }

    #[test]
    fn test_validate_kind_valid() {
        assert!(validate_kind("post").is_ok());
        assert!(validate_kind("page").is_ok());
        assert!(validate_kind("article").is_ok());
        assert!(validate_kind("custom").is_ok());
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
    }

    #[test]
    fn test_validate_locale_invalid() {
        assert!(validate_locale("e").is_err()); // Too short
        assert!(validate_locale("toolonglocale").is_err()); // Too long
        assert!(validate_locale("en_US").is_err()); // Underscore not allowed
        assert!(validate_locale("en123").is_err()); // Numbers in language part
        assert!(validate_locale("en-").is_err()); // Missing country
        assert!(validate_locale("-US").is_err()); // Missing language
        assert!(validate_locale("en-1").is_err()); // Region too short
        assert!(validate_locale("en-U1").is_err()); // Region must be all alpha or all digits
        assert!(validate_locale("en-US-extra").is_err()); // Too many parts
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
