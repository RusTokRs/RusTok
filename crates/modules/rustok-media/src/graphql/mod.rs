mod mutation;
mod query;
mod types;

pub use mutation::MediaMutation;
pub use query::MediaQuery;
pub use types::*;

pub(crate) const MODULE_SLUG: &str = "media";
