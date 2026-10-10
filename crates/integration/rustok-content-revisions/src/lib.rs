//! # rustok-content-revisions
//!
//! Межмодульный крейт для интеграции revision history в RusTok platform.
//!
//! Этот крейт предоставляет unified API для tracking content revisions
//! across всех модулей RusTok (blog, forum, commerce, etc.).
//!
//! ## Architecture
//!
//! ```text
//! Business Modules (blog, forum, commerce)
//!     │
//!     │ uses
//!     ▼
//! rustok-content-revisions (this crate)
//!     │
//!     │ depends on
//!     ▼
//! rustok-revisions (standalone library)
//! ```
//!
//! ## Example
//!
//! ```rust,ignore
//! use rustok_content_revisions::ContentRevisionService;
//!
//! // В blog module
//! impl BlogService {
//!     async fn update_post(&self, ...) {
//!         let updated = self.db_update(...).await?;
//!         
//!         // Track revision
//!         self.revision_service.track_update(
//!             tenant_id,
//!             "blog_post",
//!             post_id,
//!             &old_post,
//!             &updated,
//!             user_id,
//!         ).await?;
//!         
//!         Ok(updated)
//!     }
//! }
//! ```

mod api;
mod config;
mod error;
mod migrations;
mod service;

pub use api::ContentRevisionApi;
pub use config::{ContentRevisionConfig, ContentTypeConfig};
pub use error::ContentRevisionError;
pub use migrations::Migrator;
pub use service::ContentRevisionService;

// Re-export commonly used types from rustok-revisions
pub use rustok_revisions::{
    ChangeSource, Revision, RevisionDiff, RevisionEvent, RevisionMetadata,
    Revisionable, RetentionPolicy, RevisionTracker,
};
