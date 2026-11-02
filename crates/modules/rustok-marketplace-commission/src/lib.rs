//! # RusToK Marketplace Commission
//!
//! Native module owner for Marketplace Family versioned commission rules and order line assessments.
//! Computes platform commission and seller net proceeds using immutable snapshot rules and checked integer minor unit math.

#![deny(clippy::float_arithmetic)]

mod allocation_filter;
#[cfg(test)]
mod cancelled_allocation_tests;
mod commission_service;
mod receipts;
mod service;
#[cfg(test)]
mod tests;

pub mod dto;
pub mod entities;
pub mod error;
pub mod migrations;
pub mod module;
pub mod ports;

pub use commission_service::MarketplaceCommissionService;
pub use dto::{
    AssessMarketplaceOrderCommissionsInput, AssessMarketplaceOrderCommissionsResponse,
    CreateMarketplaceCommissionRuleVersionInput, ListMarketplaceCommissionAssessmentsByOrderRequest,
    ListMarketplaceCommissionAssessmentsBySellerRequest, ListMarketplaceCommissionRulesRequest,
    MAX_COMMISSION_ASSESSMENTS_PER_PAGE, MAX_COMMISSION_RULES_PER_PAGE,
    MarketplaceCommissionAssessmentListResponse, MarketplaceCommissionAssessmentResponse,
    MarketplaceCommissionAssessmentStatus, MarketplaceCommissionRuleListResponse,
    MarketplaceCommissionRuleResponse, MarketplaceCommissionRuleStatus,
    ReadMarketplaceCommissionAssessmentRequest,
};
pub use error::{MarketplaceCommissionError, MarketplaceCommissionResult};
pub use module::MarketplaceCommissionModule;
pub use ports::{
    MarketplaceCommissionCommandPort, MarketplaceCommissionReadPort,
    in_process_marketplace_commission_command_port,
};

#[cfg(test)]
mod contract_tests;
