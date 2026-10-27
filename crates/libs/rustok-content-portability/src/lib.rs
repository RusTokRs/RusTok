//! # RusToK Content Portability
//!
//! Unified content import and export contracts and implementation services for the RusToK platform.

mod contracts;
mod errors;
mod formats;
mod io;
mod progress;
mod services;
mod validation;

#[cfg(test)]
mod contract_tests;

pub use contracts::{
    BatchImportResult, ContentExporter, ContentImporter, ExportContext, ExportFilters,
    ExportResult, ImportContext, ImportResult,
};
pub use errors::PortabilityError;
pub use formats::{CsvFormatHandler, Format, FormatOptions, JsonFormatHandler};
pub use io::{file_exists, file_size, read_file, write_file};
pub use progress::{ProgressCallback, ProgressReporter};
pub use services::{ExportService, ImportService};
pub use validation::{FieldValidation, Validatable, ValidationResult};
