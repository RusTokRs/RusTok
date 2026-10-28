//! Tenant-owned policy for Blog's public comment surface.

use serde::Deserialize;
use serde_json::Value;

/// Stable manifest/settings key for the public Blog comment surface policy.
pub const BLOG_COMMENTS_MODE_SETTING: &str = "comments_mode";

/// The policy is owned by Blog; Comments remains the owner of comment data.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum BlogCommentsMode {
    Disabled,
    ReadOnly,
    #[default]
    Open,
}

#[derive(Debug, Default, Deserialize)]
struct BlogCommentSettings {
    #[serde(default)]
    comments_mode: BlogCommentsMode,
}

/// Parses the normalized Blog module document.
///
/// `Open` is deliberately the default for documents written before
/// `comments_mode` was introduced. Adding the optional setting therefore does
/// not close comments when an existing tenant re-enables or updates Blog.
pub(crate) fn parse_comments_mode(settings: &Value) -> Result<BlogCommentsMode, ()> {
    serde_json::from_value::<BlogCommentSettings>(settings.clone())
        .map(|settings| settings.comments_mode)
        .map_err(|_| ())
}

#[cfg(test)]
mod tests {
    use super::{BLOG_COMMENTS_MODE_SETTING, BlogCommentsMode, parse_comments_mode};

    #[test]
    fn prior_normalized_documents_keep_the_open_public_comment_contract() {
        assert_eq!(
            parse_comments_mode(&serde_json::json!({ "use_reactions": false })),
            Ok(BlogCommentsMode::Open)
        );
    }

    #[test]
    fn accepts_only_the_manifest_policy_values() {
        assert_eq!(
            parse_comments_mode(&serde_json::json!({ "comments_mode": "disabled" })),
            Ok(BlogCommentsMode::Disabled)
        );
        assert_eq!(
            parse_comments_mode(&serde_json::json!({ "comments_mode": "read_only" })),
            Ok(BlogCommentsMode::ReadOnly)
        );
        assert!(parse_comments_mode(&serde_json::json!({ "comments_mode": "other" })).is_err());
        assert_eq!(BLOG_COMMENTS_MODE_SETTING, "comments_mode");
    }
}
