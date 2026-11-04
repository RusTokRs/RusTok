//! # RusToK Region Module
//!
//! Provides geographical jurisdiction boundaries, country-to-region mapping,
//! currency codes, localized region naming, and regional tax policy definitions.

mod domain;
pub mod dto;
mod entities;
pub mod error;
pub mod migrations;
pub mod module;
pub mod ports;
pub mod services;
pub mod translation_changes;
pub mod translation_target;

pub use dto::{
    CreateRegionInput, RegionCountryTaxPolicyInput, RegionCountryTaxPolicyResponse, RegionResponse,
    RegionTaxPolicy, RegionTranslationInput, RegionTranslationResponse, UpdateRegionInput,
};
pub use error::{RegionError, RegionResult};
pub use module::RegionModule;
pub use ports::{
    RegionListRequest, RegionReadPort, RegionReadProjection, RegionReadRequest, RegionReadSelector,
    in_process_region_read_port,
};
pub use services::{
    RegionService, RegionTranslationExactLocaleApply, RegionTranslationExactLocaleApplyReceipt,
    RegionTranslationExactLocaleError, RegionTranslationExactLocaleRecord,
    RegionTranslationExactLocaleResult, RegionTranslationExactLocaleSnapshot,
    RegionTranslationService,
};
pub use translation_changes::{RegionTranslationChangeLifecycle, RegionTranslationChangeRecord};
pub use translation_target::{
    RegionTranslationTargetProvider, register_region_translation_target_provider,
};

#[cfg(test)]
mod tests;
