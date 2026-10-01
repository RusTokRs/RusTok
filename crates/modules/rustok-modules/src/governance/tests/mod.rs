//! Governance integration and contract tests.
#![allow(unused_imports)]

use super::actions_ownership::*;
use super::actions_review::*;
use super::admissions::*;
use super::commands::*;
use super::constants::*;
use super::error::*;
use super::evidence::*;
use super::helpers::*;
use super::mapping::*;
use super::market_projections::*;
use super::projections::*;
use super::publication::*;
use super::publication_verification::*;
use super::receipts::*;
use super::receipts_reviews::*;
use super::remote_runners::*;
use super::snapshots::*;
use super::staging::*;
use super::staging_alloy::*;
use super::staging_external::*;
use super::types::*;
use super::types_publication::*;
use super::upload::*;
use super::validation_evidence::*;
use super::validation_jobs::*;
use super::validation_results::*;
use super::validation_stages::*;
use super::validation_work_items::*;
use super::*;

mod fixtures;

mod action_tests;
mod error_and_contract_tests;
mod marketplace_projection_tests;
mod publication_evidence_tests;
mod publication_tests;
mod snapshot_projection_tests;
mod staging_external_tests;
mod staging_platform_tests;
mod validation_alloy_tests;
mod validation_job_tests;
mod validation_stage_and_remote_tests;
