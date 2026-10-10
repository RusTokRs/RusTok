//! # rustok-revisions
//!
//! A library for tracking content revisions in Rust applications.
//!
//! This library provides a flexible and efficient way to track changes to content
//! over time, with support for multiple backends and advanced features like
//! named versions, retention policies, and diff computation.
//!
//! ## Features
//!
//! - **Revision tracking**: Automatically track changes to your content
//! - **Multiple backends**: Support for SeaORM (PostgreSQL) and extensible to others
//! - **Diff computation**: Compare revisions and see what changed
//! - **Named versions**: Create named snapshots of important revisions
//! - **Retention policies**: Automatically clean up old revisions
//! - **Restore**: Restore content to any previous revision
//!
//! ## Quick Start
//!
//! ```rust,ignore
//! use rustok_revisions::{RevisionService, SeaOrmBackend, Revisionable};
//! use serde::{Serialize, Deserialize};
//!
//! #[derive(Clone, Serialize, Deserialize, Revisionable)]
//! struct Post {
//!     title: String,
//!     content: String,
//! }
//!
//! // Create service with SeaORM backend
//! let backend = SeaOrmBackend::new(db_connection);
//! let service = RevisionService::new(Box::new(backend));
//!
//! // Track a revision
//! let tracker = RevisionTracker::builder()
//!     .enabled(true)
//!     .retention(RetentionPolicy::KeepLast(100))
//!     .build();
//!
//! service.create_revision_with_tracker(
//!     tenant_id,
//!     content_id,
//!     "en",
//!     &old_post,
//!     &new_post,
//!     user_id,
//!     &tracker,
//!     RevisionEvent::Update,
//! ).await?;
//! ```
//!
//! ## Feature Flags
//!
//! - `derive`: Enable derive macros for `Revisionable` trait
//! - `seaorm`: Enable SeaORM backend support

mod backend;
mod diff;
mod error;
mod revision;
mod service;
mod tracker;
mod traits;

#[cfg(feature = "seaorm")]
mod seaorm_backend;

pub use backend::RevisionBackend;
pub use diff::{apply_diff, compute_diff};
pub use error::{RevisionError, RevisionResult};
pub use revision::{ChangeSource, Revision, RevisionDiff, RevisionEvent, RevisionMetadata};
pub use service::RevisionService;
pub use tracker::{RevisionTracker, RevisionTrackerBuilder};
pub use traits::{RevisionConfig, Revisionable, RetentionPolicy};

#[cfg(feature = "seaorm")]
pub use seaorm_backend::SeaOrmBackend;

// Re-export derive macro if feature is enabled
#[cfg(feature = "derive")]
pub use rustok_revisions_derive::Revisionable;
