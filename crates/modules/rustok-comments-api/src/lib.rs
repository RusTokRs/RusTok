mod model;
#[cfg(feature = "server")]
mod provider;

pub use model::{
    CommentListItem, CommentRecord, CommentStatus, CreateCommentInput, ListCommentsFilter,
    SetCommentStatusRequest, UpdateCommentInput,
};
#[cfg(feature = "server")]
pub use provider::CommentsThreadPort;
