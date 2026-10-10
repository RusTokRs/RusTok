//! Content Portability — implementation of import/export services.
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

pub use formats::*;
pub use io::*;
pub use services::*;
