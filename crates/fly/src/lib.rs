//! Framework-neutral page-builder engine and lossless GrapesJS compatibility model.
//!
//! The crate is deliberately independent from UI frameworks, browser APIs, RusTok modules,
//! persistence, and transports. Consumers own persistence and framework adapters.

mod asset;
mod binding;
mod codec;
mod command;
mod context_compatibility;
mod context_contract;
mod context_dependency;
mod context_json_schema;
mod context_migration;
mod context_scenario;
mod context_schema;
mod dynamic;
mod error;
mod fragment;
mod ids;
mod locale_coverage;
mod locale_policy;
mod model;
mod page;
mod page_metadata;
mod page_metadata_locale;
mod placement;
mod registry;
mod render;
mod runtime_gate;
mod runtime_locale;
mod runtime_pipeline;
mod runtime_render;
mod runtime_scenario_release;
mod runtime_scenario_render;
mod runtime_scenario_snapshot;
mod runtime_validation;
mod style_rule;
mod trait_model;
mod translation;
mod validation;

pub use asset::*;
pub use binding::*;
pub use codec::*;
pub use command::*;
pub use context_compatibility::*;
pub use context_contract::*;
pub use context_dependency::*;
pub use context_json_schema::*;
pub use context_migration::*;
pub use context_scenario::*;
pub use context_schema::*;
pub use dynamic::*;
pub use error::*;
pub use fragment::*;
pub use ids::*;
pub use locale_coverage::*;
pub use locale_policy::*;
pub use model::*;
pub use page::*;
pub use page_metadata::*;
pub use page_metadata_locale::*;
pub use placement::*;
pub use registry::*;
pub use render::*;
pub use runtime_gate::*;
pub use runtime_locale::*;
pub use runtime_pipeline::*;
pub use runtime_render::*;
pub use runtime_scenario_release::*;
pub use runtime_scenario_render::*;
pub use runtime_scenario_snapshot::*;
pub use runtime_validation::*;
pub use style_rule::*;
pub use trait_model::*;
pub use translation::*;
pub use validation::*;

impl Copy for ConditionOperator {}
impl Copy for EmptyRepeaterBehavior {}

pub const GRAPESJS_V1: &str = "grapesjs_v1";
pub const FLY_FRAGMENT_V1: &str = "fly_fragment_v1";
pub const RICH_TEXT_PAYLOAD_V1: &str = "fly_rich_text_payload_v1";

pub type FlyResult<T> = Result<T, FlyError>;

#[cfg(test)]
mod tests;
