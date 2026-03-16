#[cfg(feature = "mod-alloy")]
pub mod alloy;
pub mod auth;
#[cfg(feature = "mod-blog")]
pub mod blog;
#[cfg(feature = "mod-commerce")]
pub mod commerce;
pub mod common;
pub mod connection;
#[cfg(any(feature = "mod-content", feature = "mod-blog"))]
pub mod content;
pub mod errors;
#[cfg(feature = "mod-forum")]
pub mod forum;
pub mod loaders;
pub mod mutations;
pub mod oauth;
pub mod observability;
#[cfg(feature = "mod-pages")]
pub mod pages;
pub mod persisted;
pub mod queries;
pub mod schema;
pub mod settings;
pub mod subscriptions;
pub mod types;

pub use schema::{build_schema, AppSchema, SharedGraphqlSchema};
