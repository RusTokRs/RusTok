mod mutation;
mod query;
mod types;

pub use mutation::ProfilesMutation;
pub use query::ProfilesQuery;
pub use types::*;

pub const MODULE_SLUG: &str = "profiles";
