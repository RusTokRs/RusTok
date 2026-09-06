//! RusToK Test Utilities
//!
//! This crate provides testing utilities for RusToK modules:
//! - Database setup and teardown utilities
//! - Mock event bus for testing event publishing
//! - Test fixtures for common data types
//! - Helper functions for creating test contexts
//!
//! # Example
//!
//! ```rust
//! use rustok_test_utils::{setup_test_db, mock_event_bus, fixtures::UserFixture};
//! use rustok_core::SecurityContext;
//!
//! #[tokio::test]
//! async fn test_example() {
//!     let db = setup_test_db().await;
//!     let event_bus = mock_event_bus();
//!     let user = UserFixture::admin().build();
//!     let security = SecurityContext::system();
//! }
//! ```

pub mod db;
pub mod events;
pub mod fixtures;
pub mod helpers;

pub use db::{
    assert_postgres_column_contract, assert_postgres_column_missing, assert_postgres_table_missing,
    assert_postgres_url, assert_valid_postgres_database_name, connect_postgres,
    create_postgres_database, drop_postgres_database_if_exists, postgres_database_url,
    setup_test_db, unique_postgres_database_name,
};
pub use events::{MockEventBus, MockEventTransport, mock_event_bus, mock_transactional_event_bus};
pub use helpers::*;

#[cfg(test)]
mod contract_tests;
