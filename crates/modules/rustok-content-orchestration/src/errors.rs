//! Error conversion helpers for content orchestration cross-module boundaries.

#[cfg(all(feature = "mod-content", feature = "mod-blog"))]
pub(crate) fn blog_error_to_content_error(
    error: rustok_blog::BlogError,
) -> rustok_content::ContentError {
    match error {
        rustok_blog::BlogError::Database(error) => rustok_content::ContentError::Database(error),
        rustok_blog::BlogError::Forbidden(message) => {
            rustok_content::ContentError::forbidden(message)
        }
        rustok_blog::BlogError::DuplicateSlug { slug } => {
            rustok_content::ContentError::duplicate_slug(slug, "")
        }
        rustok_blog::BlogError::Content(error) => error,
        rustok_blog::BlogError::Core(error) => rustok_content::ContentError::Core(error),
        other => rustok_content::ContentError::validation(other.to_string()),
    }
}

#[cfg(all(feature = "mod-content", feature = "mod-forum"))]
pub(crate) fn forum_error_to_content_error(
    error: rustok_forum::ForumError,
) -> rustok_content::ContentError {
    match error {
        rustok_forum::ForumError::Database(error) => rustok_content::ContentError::Database(error),
        rustok_forum::ForumError::Content(error) => error,
        rustok_forum::ForumError::Internal(error) => rustok_content::ContentError::Core(error),
        other => rustok_content::ContentError::validation(other.to_string()),
    }
}
