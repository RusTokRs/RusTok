//! # RusToK Content Portability
//!
//! Content import and export implementation services for the RusToK platform.
//!
//! This crate provides concrete implementations for importing and exporting
//! content in various formats (JSON, CSV, etc.).
//!
//! # Architecture
//!
//! ```text
//! rustok-content-portability-api  ← contracts
//! rustok-content-portability      ← implementation (this crate)
//!   ├─ Services (ImportService, ExportService)
//!   ├─ Format handlers (JSON, CSV)
//!   ├─ File I/O helpers
//!   └─ Validation framework
//! ```
//!
//! # Example
//!
//! ```rust,ignore
//! use rustok_content_portability::{ImportService, JsonFormatHandler};
//! use rustok_content_portability_api::{ImportContext, Format};
//!
//! let service = ImportService::new(JsonFormatHandler);
//! let context = ImportContext::new(tenant_id, user_id, Format::Json);
//!
//! let result = service.import_from_file("posts.json", context).await?;
//! println!("Imported {} items", result.success_count());
//! ```

mod formats;
mod io;
mod services;

#[cfg(test)]
mod contract_tests;

pub use formats::{CsvFormatHandler, JsonFormatHandler};
pub use io::{file_exists, file_size, read_file, write_file};
pub use services::{ExportService, ImportService};
