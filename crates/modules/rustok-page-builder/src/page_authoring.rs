//! Authoring contracts shared by the Pages server and every Pages editor host.
//!
//! The server and the admin used to carry two diverging slug functions (Unicode on the server,
//! ASCII-only in the admin) and the admin seeded new pages with a document the static publish
//! policy rejected. Both behaviours now live here once, so the editor and the server cannot drift
//! and the starter document is verified against the real publish policy in this crate's tests.

use serde_json::{Value, json};

/// Maximum slug length, in Unicode scalar values. Mirrors the server storage constraint.
pub const PAGE_SLUG_MAX_CHARS: usize = 255;

/// Normalizes free text into a route slug for editor auto-generation.
///
/// - letters and digits of any script are kept and lowercased (`О компании` → `о-компании`);
/// - every run of other characters collapses into one `-`;
/// - leading and trailing dashes are removed;
/// - the result is truncated to [`PAGE_SLUG_MAX_CHARS`] without leaving a trailing dash.
///
/// Returns an empty string when the input contains no letters or digits. Servers must use
/// [`normalize_page_slug_strict`], which rejects instead of truncating.
pub fn normalize_page_slug(value: &str) -> String {
    collapse_slug(value, Some(PAGE_SLUG_MAX_CHARS))
}

/// Why a slug was rejected by [`normalize_page_slug_strict`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageSlugError {
    /// The input has no letters or digits.
    Empty,
    /// The normalized slug is longer than [`PAGE_SLUG_MAX_CHARS`].
    TooLong,
}

/// Server-side slug normalization: identical character rules to [`normalize_page_slug`], but an
/// over-long slug is rejected instead of silently truncated.
pub fn normalize_page_slug_strict(value: &str) -> Result<String, PageSlugError> {
    let normalized = collapse_slug(value, None);
    if normalized.is_empty() {
        Err(PageSlugError::Empty)
    } else if normalized.chars().count() > PAGE_SLUG_MAX_CHARS {
        Err(PageSlugError::TooLong)
    } else {
        Ok(normalized)
    }
}

fn collapse_slug(value: &str, max_chars: Option<usize>) -> String {
    let limit = max_chars.unwrap_or(usize::MAX);
    let mut normalized = String::with_capacity(value.len());
    let mut chars = 0usize;
    let mut pending_dash = false;
    for ch in value.trim().chars().flat_map(char::to_lowercase) {
        if !ch.is_alphanumeric() {
            pending_dash = true;
            continue;
        }
        if pending_dash && !normalized.is_empty() {
            // A dash is only worth emitting if a character can still follow it.
            if chars.saturating_add(1) >= limit {
                break;
            }
            normalized.push('-');
            chars += 1;
        }
        pending_dash = false;
        if chars >= limit {
            break;
        }
        normalized.push(ch);
        chars += 1;
    }
    normalized
}

/// Fallback title used when a starter document is requested for an empty title.
pub const STARTER_DOCUMENT_FALLBACK_TITLE: &str = "New page";

/// Builds the canonical starter document for a newly created page.
///
/// It uses only structured components (`heading` + `text`) with plain-text content, so a page
/// created from it can be published immediately without manual repair.
pub fn starter_page_document(title: &str) -> Value {
    let title = title.trim();
    let title = if title.is_empty() {
        STARTER_DOCUMENT_FALLBACK_TITLE
    } else {
        title
    };
    json!({
        "assets": [],
        "styles": [],
        "pages": [
            {
                "id": "main",
                "name": title,
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "components": [
                        {
                            "id": "page-section",
                            "type": "section",
                            "tagName": "section",
                            "style": { "padding": "48px 24px" },
                            "components": [
                                {
                                    "id": "page-heading",
                                    "type": "heading",
                                    "tagName": "h1",
                                    "content": title
                                },
                                {
                                    "id": "page-intro",
                                    "type": "text",
                                    "content": "Start writing here, or add sections from the palette."
                                }
                            ]
                        }
                    ]
                }
            }
        ]
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{validate_page_builder_document, validate_static_publish_document};
    use fly::GrapesJsCodec;

    #[test]
    fn slug_keeps_letters_of_any_script() {
        assert_eq!(
            normalize_page_slug("Hello, Current Pages!"),
            "hello-current-pages"
        );
        assert_eq!(normalize_page_slug("  О компании  "), "о-компании");
        assert_eq!(
            normalize_page_slug("Доставка и оплата — 2026"),
            "доставка-и-оплата-2026"
        );
        assert_eq!(
            normalize_page_slug("Ärger über Straße"),
            "ärger-über-straße"
        );
        assert_eq!(normalize_page_slug("--a__b--"), "a-b");
    }

    #[test]
    fn slug_is_empty_without_letters_or_digits() {
        assert_eq!(normalize_page_slug(""), "");
        assert_eq!(normalize_page_slug(" -- !! "), "");
    }

    #[test]
    fn slug_is_bounded_without_trailing_dash() {
        let long = "ab ".repeat(400);
        let slug = normalize_page_slug(&long);
        assert!(slug.chars().count() <= PAGE_SLUG_MAX_CHARS);
        assert!(!slug.ends_with('-'));
        assert!(!slug.starts_with('-'));
    }

    #[test]
    fn strict_slug_rejects_instead_of_truncating() {
        assert_eq!(
            normalize_page_slug_strict(" !! "),
            Err(PageSlugError::Empty)
        );
        assert_eq!(
            normalize_page_slug_strict(&"a".repeat(PAGE_SLUG_MAX_CHARS + 1)),
            Err(PageSlugError::TooLong)
        );
        assert_eq!(
            normalize_page_slug_strict("О компании").as_deref(),
            Ok("о-компании")
        );
    }

    #[test]
    fn lenient_and_strict_slugs_agree_within_the_limit() {
        for input in [
            "О компании",
            "Hello, World",
            "a--b",
            "Ärger über Straße",
            "x",
        ] {
            assert_eq!(
                normalize_page_slug_strict(input).expect("valid"),
                normalize_page_slug(input)
            );
        }
    }

    #[test]
    fn slug_normalization_is_idempotent() {
        for input in ["О компании", "Hello, World", "a--b", "Ärger über Straße"] {
            let once = normalize_page_slug(input);
            assert_eq!(normalize_page_slug(&once), once);
        }
    }

    #[test]
    fn starter_document_passes_document_validation_and_static_publish_policy() {
        for title in ["Landing", "О компании", "", "Tom & Jerry"] {
            let value = starter_page_document(title);
            validate_page_builder_document(&value).expect("starter document must be valid");
            let document = GrapesJsCodec::decode_value(value).expect("starter document decodes");
            validate_static_publish_document(&document)
                .expect("starter document must be publishable without manual repair");
        }
    }

    #[test]
    fn starter_document_uses_the_title_as_plain_heading_text() {
        let value = starter_page_document("  О компании ");
        let heading = &value["pages"][0]["component"]["components"][0]["components"][0];
        assert_eq!(heading["type"], "heading");
        assert_eq!(heading["tagName"], "h1");
        assert_eq!(heading["content"], "О компании");
        assert_eq!(
            starter_page_document("   ")["pages"][0]["name"],
            STARTER_DOCUMENT_FALLBACK_TITLE
        );
    }
}
