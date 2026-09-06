//! Framework-neutral page-builder engine and lossless GrapesJS compatibility model.
//!
//! The crate is deliberately independent from UI frameworks, browser APIs, RusTok modules,
//! persistence, and transports. Consumers own persistence and framework adapters.

mod action;
mod asset;
mod audit;
mod binding;
mod codec;
mod command;
mod component_visit;
mod context_contract;
mod context_dependency;
mod context_json_schema;
mod context_scenario;
mod context_schema;
mod dynamic;
mod error;
mod fragment;
mod ids;
mod interaction_capability;
mod interaction_capability_gate;
mod interaction_route;
mod internal_link;
mod landing_contract;
mod landing_property;
mod landing_readiness;
mod locale_coverage;
mod locale_policy;
mod localized_route;
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
mod safe_url;
mod snapshot;
mod style_rule;
mod trait_model;
mod translation;
mod validation;

pub use action::*;
pub use asset::*;
pub use audit::*;
pub use binding::*;
pub use codec::*;
pub use command::*;
pub use component_visit::{ComponentVisit, visit_project_components};
pub use context_contract::*;
pub use context_dependency::*;
pub use context_json_schema::*;
pub use context_scenario::*;
pub use context_schema::*;
pub use dynamic::*;
pub use error::*;
pub use fragment::*;
pub use ids::*;
pub use interaction_capability::*;
pub use interaction_capability_gate::*;
pub use internal_link::*;
pub use landing_contract::*;
pub use landing_property::*;
pub use landing_readiness::*;
pub use locale_coverage::*;
pub use locale_policy::*;
pub use localized_route::*;
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
pub use snapshot::*;
pub use style_rule::*;
pub use trait_model::*;
pub use translation::*;
pub use validation::*;

impl Copy for ConditionOperator {}
impl Copy for EmptyRepeaterBehavior {}

pub const GRAPESJS_FORMAT: &str = "grapesjs";
pub const FLY_FRAGMENT_FORMAT: &str = "fly_fragment";
pub const RICH_TEXT_PAYLOAD_FORMAT: &str = "fly_rich_text_payload";

pub type FlyResult<T> = Result<T, FlyError>;

#[cfg(test)]
mod tests;
