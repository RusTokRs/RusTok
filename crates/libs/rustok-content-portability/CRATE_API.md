# rustok-content-portability / CRATE_API

## Role
Concrete implementations of content import and export services, format handlers (JSON, CSV), and file I/O utilities for the RusToK platform.

## Public modules
- Root crate exports: `ImportService`, `ExportService`, `JsonFormatHandler`, `CsvFormatHandler`, `read_file`, `write_file`, `file_exists`, `file_size`.

## Primary public types and functions
- `pub struct ImportService`: Service for importing content from files, byte streams, and format handlers.
- `pub struct ExportService`: Service for serializing and exporting entity collections to files and byte streams.
- `pub struct JsonFormatHandler`: JSON format parser and serializer with pretty-print options.
- `pub struct CsvFormatHandler`: CSV format parser and serializer with configurable delimiters and headers.
- `pub async fn read_file(path: impl AsRef<Path>) -> Result<Vec<u8>, PortabilityError>`
- `pub async fn write_file(path: impl AsRef<Path>, data: &[u8]) -> Result<(), PortabilityError>`
- `pub async fn file_exists(path: impl AsRef<Path>) -> bool`
- `pub async fn file_size(path: impl AsRef<Path>) -> Result<u64, PortabilityError>`

## Contract invariants
- **Format compliance**: Parsing validates format syntax against `FormatOptions` and returns structured `PortabilityError::Format` errors.
- **Atomic file writes**: File output operations write complete buffers without leaving partially-written files on failure.
- **Fail-safe byte parsing**: Parsing operates over immutable byte slices without executing untrusted code or arbitrary deserialization side-effects.

## Errors
- Reuses [`PortabilityError`](rustok_content_portability_api::PortabilityError) covering format parsing, I/O errors, validation failures, and unsupported formats.
