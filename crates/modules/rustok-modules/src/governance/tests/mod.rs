use uuid::Uuid;
use semver::Version;
//! Governance integration and contract tests.

mod fixtures;

mod error_and_contract_tests;
mod staging_platform_tests;
mod staging_external_tests;
mod validation_job_tests;
mod validation_alloy_tests;
mod validation_stage_and_remote_tests;
mod action_tests;
mod publication_evidence_tests;
mod snapshot_projection_tests;
mod publication_tests;
