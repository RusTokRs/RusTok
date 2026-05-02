use rustok_core::error::{Error as CoreError, ErrorKind, RichError};
use thiserror::Error;
use uuid::Uuid;

/// Blog module errors
///
/// Uses both legacy Error enum and new RichError system.
/// Gradually migrate to RichError for better context.
#[derive(Error, Debug)]
pub enum BlogError {
    #[error("Database error: {0}")]
    Database(#[from] sea_orm::DbErr),

    #[error("Post not found: {0}")]
    PostNotFound(Uuid),

    #[error("Comment not found: {0}")]
    CommentNotFound(Uuid),

    #[error("Category not found: {0}")]
    CategoryNotFound(Uuid),

    #[error("Tag not found: {0}")]
    TagNotFound(Uuid),

    #[error("Duplicate slug: {slug} already exists for locale {locale}")]
    DuplicateSlug { slug: String, locale: String },

    #[error("Cannot delete published post")]
    CannotDeletePublished,

    #[error("Cannot publish archived post")]
    CannotPublishArchived,

    #[error("Author required")]
    AuthorRequired,

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Forbidden: {0}")]
    Forbidden(String),

    #[error("Content error: {0}")]
    Content(#[from] rustok_content::ContentError),

    #[error("Comments error: {0}")]
    Comments(#[from] rustok_comments::CommentsError),

    #[error("Rich error: {0}")]
    Rich(#[from] Box<RichError>),

    #[error("Core error: {0}")]
    Core(#[from] CoreError),
}

pub type BlogResult<T> = Result<T, BlogError>;

// Conversion from BlogError to RichError for API responses
impl From<BlogError> for RichError {
    fn from(err: BlogError) -> Self {
        match err {
            BlogError::Database(db_err) => {
                RichError::new(ErrorKind::Database, "Database operation failed")
                    .with_user_message("Unable to access blog data")
                    .with_source(db_err)
            }
            BlogError::PostNotFound(id) => {
                RichError::new(ErrorKind::NotFound, format!("Post {} not found", id))
                    .with_user_message("The requested blog post does not exist")
                    .with_field("post_id", id.to_string())
                    .with_error_code("POST_NOT_FOUND")
            }
            BlogError::CommentNotFound(id) => {
                RichError::new(ErrorKind::NotFound, format!("Comment {} not found", id))
                    .with_user_message("The requested comment does not exist")
                    .with_field("comment_id", id.to_string())
                    .with_error_code("COMMENT_NOT_FOUND")
            }
            BlogError::CategoryNotFound(id) => {
                RichError::new(ErrorKind::NotFound, format!("Category {} not found", id))
                    .with_user_message("The requested blog category does not exist")
                    .with_field("category_id", id.to_string())
                    .with_error_code("CATEGORY_NOT_FOUND")
            }
            BlogError::TagNotFound(id) => {
                RichError::new(ErrorKind::NotFound, format!("Tag {} not found", id))
                    .with_user_message("The requested tag does not exist")
                    .with_field("tag_id", id.to_string())
                    .with_error_code("TAG_NOT_FOUND")
            }
            BlogError::DuplicateSlug { slug, locale } => RichError::new(
                ErrorKind::Conflict,
                format!("Slug '{}' already exists for locale '{}'", slug, locale),
            )
            .with_user_message("A post with this URL slug already exists")
            .with_field("slug", slug)
            .with_field("locale", locale)
            .with_error_code("DUPLICATE_SLUG"),
            BlogError::CannotDeletePublished => {
                RichError::new(ErrorKind::BusinessLogic, "Cannot delete published post")
                    .with_user_message(
                        "Published posts cannot be deleted. Unpublish or archive them first.",
                    )
                    .with_error_code("CANNOT_DELETE_PUBLISHED")
            }
            BlogError::CannotPublishArchived => {
                RichError::new(ErrorKind::BusinessLogic, "Cannot publish archived post")
                    .with_user_message("Archived posts must be restored before publishing.")
                    .with_error_code("CANNOT_PUBLISH_ARCHIVED")
            }
            BlogError::AuthorRequired => RichError::new(ErrorKind::Validation, "Author required")
                .with_user_message("An author must be specified for blog posts")
                .with_error_code("AUTHOR_REQUIRED"),
            BlogError::Validation(msg) => {
                RichError::new(ErrorKind::Validation, msg).with_user_message("Invalid input data")
            }
            BlogError::Forbidden(msg) => RichError::new(ErrorKind::Forbidden, msg)
                .with_user_message("You do not have permission to perform this action"),
            BlogError::Content(content_err) => content_err.into(),
            BlogError::Comments(err) => match err {
                rustok_comments::CommentsError::Database(db_err) => {
                    RichError::new(ErrorKind::Database, "Database operation failed")
                        .with_user_message("Unable to access comment data")
                        .with_source(db_err)
                }
                rustok_comments::CommentsError::CommentNotFound(id) => {
                    RichError::new(ErrorKind::NotFound, format!("Comment {} not found", id))
                        .with_user_message("The requested comment does not exist")
                        .with_field("comment_id", id.to_string())
                        .with_error_code("COMMENT_NOT_FOUND")
                }
                rustok_comments::CommentsError::CommentThreadNotFound {
                    target_type,
                    target_id,
                } => RichError::new(
                    ErrorKind::NotFound,
                    format!("Comment thread for {}:{} not found", target_type, target_id),
                )
                .with_user_message("The requested comment thread does not exist")
                .with_field("target_type", target_type)
                .with_field("target_id", target_id.to_string())
                .with_error_code("COMMENT_THREAD_NOT_FOUND"),
                rustok_comments::CommentsError::CommentThreadClosed {
                    target_type,
                    target_id,
                } => RichError::new(
                    ErrorKind::Conflict,
                    format!("Comment thread for {}:{} is closed", target_type, target_id),
                )
                .with_user_message("Comments are closed for this thread")
                .with_field("target_type", target_type)
                .with_field("target_id", target_id.to_string())
                .with_error_code("COMMENT_THREAD_CLOSED"),
                rustok_comments::CommentsError::Forbidden(message) => {
                    RichError::new(ErrorKind::Forbidden, message).with_user_message(
                        "You do not have permission to perform this comment action",
                    )
                }
                rustok_comments::CommentsError::Validation(message) => {
                    RichError::new(ErrorKind::Validation, message)
                        .with_user_message("Invalid comment data")
                }
            },
            BlogError::Rich(rich) => *rich,
            BlogError::Core(core) => core.into(),
        }
    }
}

/// Helper functions for creating common blog errors
impl BlogError {
    /// Create a post not found error
    pub fn post_not_found(post_id: Uuid) -> Self {
        BlogError::PostNotFound(post_id)
    }

    /// Create a comment not found error
    pub fn comment_not_found(comment_id: Uuid) -> Self {
        BlogError::CommentNotFound(comment_id)
    }

    /// Create a category not found error
    pub fn category_not_found(category_id: Uuid) -> Self {
        BlogError::CategoryNotFound(category_id)
    }

    /// Create a tag not found error
    pub fn tag_not_found(tag_id: Uuid) -> Self {
        BlogError::TagNotFound(tag_id)
    }

    /// Create a duplicate slug error
    pub fn duplicate_slug(slug: impl Into<String>, locale: impl Into<String>) -> Self {
        BlogError::DuplicateSlug {
            slug: slug.into(),
            locale: locale.into(),
        }
    }

    /// Create a validation error
    pub fn validation(message: impl Into<String>) -> Self {
        BlogError::Validation(message.into())
    }

    /// Create a forbidden error
    pub fn forbidden(message: impl Into<String>) -> Self {
        BlogError::Forbidden(message.into())
    }
}

impl From<rustok_taxonomy::TaxonomyError> for BlogError {
    fn from(value: rustok_taxonomy::TaxonomyError) -> Self {
        match value {
            rustok_taxonomy::TaxonomyError::Database(err) => Self::Database(err),
            rustok_taxonomy::TaxonomyError::Forbidden(message) => Self::Forbidden(message),
            rustok_taxonomy::TaxonomyError::Validation(message)
            | rustok_taxonomy::TaxonomyError::DuplicateCanonicalKey(message)
            | rustok_taxonomy::TaxonomyError::DuplicateSlug(message)
            | rustok_taxonomy::TaxonomyError::DuplicateAlias(message) => Self::Validation(message),
            rustok_taxonomy::TaxonomyError::TermNotFound(term_id) => {
                Self::Validation(format!("Taxonomy term not found: {term_id}"))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_post_not_found_conversion() {
        let id = Uuid::new_v4();
        let err = BlogError::post_not_found(id);
        let rich: RichError = err.into();

        assert_eq!(rich.kind, ErrorKind::NotFound);
        assert_eq!(rich.status_code, 404);
        assert!(rich.fields.contains_key("post_id"));
    }

    #[test]
    fn test_duplicate_slug_conversion() {
        let err = BlogError::duplicate_slug("my-post", "en");
        let rich: RichError = err.into();

        assert_eq!(rich.kind, ErrorKind::Conflict);
        assert_eq!(rich.status_code, 409);
        assert_eq!(rich.fields.get("slug"), Some(&"my-post".to_string()));
    }

    #[test]
    fn test_cannot_delete_published_conversion() {
        let err = BlogError::CannotDeletePublished;
        let rich: RichError = err.into();

        assert_eq!(rich.kind, ErrorKind::BusinessLogic);
        assert_eq!(rich.error_code, Some("CANNOT_DELETE_PUBLISHED".to_string()));
    }
}
