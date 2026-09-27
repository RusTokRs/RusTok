//! Transport- and persistence-independent Blog domain policies.

pub const BLOG_CATEGORY_SETTINGS_MAX_BYTES: usize = 64 * 1024;

pub(crate) mod comment_policy;
pub mod richtext;
pub mod state_machine;

#[cfg(test)]
mod state_machine_proptest;
