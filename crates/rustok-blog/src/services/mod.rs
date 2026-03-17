//! Services for the Blog module

mod comment;
mod post;

pub use comment::CommentService;
pub use post::PostService;
pub use rustok_content::CategoryService;
pub use rustok_content::TagService;
