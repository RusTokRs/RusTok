//! # RusToK Content Portability API
//!
//! Unified content import/export contracts for the RusTok platform.
//! This crate provides domain-agnostic contracts for importing and exporting
//! business entities (posts, products, subscribers, etc.) across different
//! formats (JSON, CSV, WordPress XML, Markdown).
//!
//! # Architecture
//!
//! ```text
//! rustok-content-portability-api  ← contracts (this crate)
//!   ├─ ContentImporter trait
//!   ├─ ContentExporter trait
//!   ├─ Format descriptors
//!   └─ Batch/progress contracts
//!
//! rustok-content-portability      ← implementation
//!   ├─ Services
//!   ├─ Format handlers (JSON, CSV)
//!   └─ Validation framework
//!
//! Domain modules implement:
//!   rustok-blog      → BlogPostImporter, BlogPostExporter
//!   rustok-forum     → ForumTopicImporter
//!   rustok-commerce  → ProductImporter, ProductExporter
//!   rustok-newsletter → SubscriberImporter, SubscriberExporter
//! ```
//!
//! # Example
//!
//! ```rust,ignore
//! use rustok_content_portability_api::{
//!     ContentImporter, ImportContext, ImportResult, Format,
//! };
//!
//! struct BlogPostImporter;
//!
//! #[async_trait::async_trait]
//! impl ContentImporter<BlogPostInput, BlogPost> for BlogPostImporter {
//!     async fn import(
//!         &self,
//!         source: BlogPostInput,
//!         context: ImportContext,
//!     ) -> Result<ImportResult<BlogPost>, PortabilityError> {
//!         // Validate, transform, persist
//!         todo!()
//!     }
//! }
//! ```

mod contracts;
mod errors;
mod formats;
mod progress;
mod validation;

#[cfg(test)]
mod contract_tests;

pub use contracts::{
    BatchImportResult, ContentExporter, ContentImporter, ExportContext, ExportFilters,
    ExportResult, ImportContext, ImportResult,
};
pub use errors::PortabilityError;
pub use formats::{Format, FormatOptions};
pub use progress::{ProgressCallback, ProgressReporter};
pub use validation::{FieldValidation, Validatable, ValidationResult};
