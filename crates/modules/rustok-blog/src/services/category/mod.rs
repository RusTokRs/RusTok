mod commands;
pub(super) mod name_projection;
mod owner;
mod taxonomy_sync;

pub use commands::CategoryCommandService;
pub use owner::CategoryService;

pub(super) use commands::{
    CATEGORY_DESCRIPTION_MAX_CHARS, CATEGORY_NAME_MAX_CHARS, CATEGORY_SLUG_MAX_CHARS,
    CategoryCommandCore, record_category_copy_change_in_tx,
};
