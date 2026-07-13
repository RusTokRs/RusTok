//! Framework-neutral page-builder engine and lossless GrapesJS compatibility model.
//!
//! The crate is deliberately independent from UI frameworks, browser APIs, RusTok modules,
//! persistence, and transports. Consumers own persistence and framework adapters.

mod asset;
mod codec;
mod command;
mod error;
mod fragment;
mod ids;
mod model;
mod page;
mod placement;
mod registry;
mod style_rule;
mod validation;

pub use asset::*;
pub use codec::*;
pub use command::*;
pub use error::*;
pub use fragment::*;
pub use ids::*;
pub use model::*;
pub use page::*;
pub use placement::*;
pub use registry::*;
pub use style_rule::*;
pub use validation::*;

pub const GRAPESJS_V1: &str = "grapesjs_v1";
pub const FLY_FRAGMENT_V1: &str = "fly_fragment_v1";
pub const RICH_TEXT_PAYLOAD_V1: &str = "fly_rich_text_payload_v1";

pub type FlyResult<T> = Result<T, FlyError>;

#[cfg(test)]
mod tests;
