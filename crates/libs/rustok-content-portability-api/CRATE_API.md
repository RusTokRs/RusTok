# rustok-content-portability-api / CRATE_API

## Role
Domain-agnostic contracts for content import and export pipelines across formats (JSON, CSV, WordPress XML, Markdown) for the RusToK platform.

## Public modules
- Root crate exports: `ContentImporter`, `ContentExporter`, `ImportContext`, `ExportContext`, `ImportResult`, `ExportResult`, `BatchImportResult`, `Format`, `PortabilityError`, `ProgressCallback`, `ProgressReporter`, `FieldValidation`, `ValidationResult`, `Validatable`.

## Primary public types and functions
- `pub trait ContentImporter<Source, Target>`: Asynchronous trait for validating and importing single items and batches.
- `pub trait ContentExporter<Source, Target>`: Asynchronous trait for exporting single items and batches.
- `pub struct ImportContext`: Tenant isolation (`tenant_id`), user identity (`user_id`), format, error continuation flag, and progress reporting callback.
- `pub struct ExportContext`: Tenant isolation (`tenant_id`), user identity (`user_id`), format, export filters, and progress callback.
- `pub enum Format`: Supported formats (`Json`, `Csv`, `WordPressXml`, `Markdown`, `Custom`) with MIME type and extension helpers.
- `pub struct FormatOptions`: Format configuration options (CSV delimiter, headers, JSON pretty-print).
- `pub enum PortabilityError`: Strongly-typed error enum covering validation, format, transformation, persistence, and batch errors.
- `pub struct ValidationResult`: Field-level validation collector with error conversion.

## Contract invariants
- **Multi-tenant scoping**: All import and export contexts strictly require canonical `tenant_id` scoping to prevent cross-tenant leakage.
- **Fail-safe batch semantics**: `continue_on_error = false` fails fast on the first error; `continue_on_error = true` records partial failures while continuing execution.
- **Format neutrality**: Import and export traits decouple business entity serialization from storage or transport engines.

## Errors
- [`PortabilityError`]: Strongly-typed failure covering validation, format parsing, entity transformation, persistence, and batch partial failures.
