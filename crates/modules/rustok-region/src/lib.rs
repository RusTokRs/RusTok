pub mod dto;
pub mod entities;
pub mod error;
pub mod migrations;
pub mod module;
pub mod ports;
pub mod services;
pub mod translation_changes;
pub mod translation_target;

pub use dto::*;
pub use error::{RegionError, RegionResult};
pub use module::RegionModule;
pub use ports::*;
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
mod contract_tests;
