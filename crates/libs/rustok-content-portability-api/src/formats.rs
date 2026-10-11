//! Content format descriptors.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Supported content formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Format {
    /// JSON format (universal).
    #[default]
    Json,
    /// CSV format (tabular data).
    Csv,
    /// WordPress XML export format.
    WordPressXml,
    /// Markdown with frontmatter.
    Markdown,
    /// Custom format (identifier in metadata).
    Custom,
}

impl Format {
    /// Get the file extension for this format.
    pub fn extension(&self) -> &'static str {
        match self {
            Format::Json => "json",
            Format::Csv => "csv",
            Format::WordPressXml => "xml",
            Format::Markdown => "md",
            Format::Custom => "dat",
        }
    }

    /// Get the MIME type for this format.
    pub fn mime_type(&self) -> &'static str {
        match self {
            Format::Json => "application/json",
            Format::Csv => "text/csv",
            Format::WordPressXml => "application/xml",
            Format::Markdown => "text/markdown",
            Format::Custom => "application/octet-stream",
        }
    }

    /// Check if this format supports batch operations natively.
    pub fn supports_batch(&self) -> bool {
        match self {
            Format::Json | Format::Csv | Format::WordPressXml => true,
            Format::Markdown | Format::Custom => false,
        }
    }

    /// Check if this format is human-readable.
    pub fn is_human_readable(&self) -> bool {
        matches!(self, Format::Json | Format::Csv | Format::Markdown)
    }
}

impl fmt::Display for Format {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Format::Json => write!(f, "JSON"),
            Format::Csv => write!(f, "CSV"),
            Format::WordPressXml => write!(f, "WordPress XML"),
            Format::Markdown => write!(f, "Markdown"),
            Format::Custom => write!(f, "Custom"),
        }
    }
}

/// Format-specific options.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FormatOptions {
    /// CSV delimiter (default: comma).
    pub csv_delimiter: Option<u8>,
    /// CSV quote character (default: double quote).
    pub csv_quote: Option<u8>,
    /// Whether to include headers in CSV.
    pub csv_include_headers: Option<bool>,
    /// JSON pretty-print.
    pub json_pretty: Option<bool>,
    /// Custom format identifier.
    pub custom_format_id: Option<String>,
}

impl FormatOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_csv_delimiter(mut self, delimiter: u8) -> Self {
        self.csv_delimiter = Some(delimiter);
        self
    }

    pub fn with_csv_headers(mut self, include: bool) -> Self {
        self.csv_include_headers = Some(include);
        self
    }

    pub fn with_json_pretty(mut self, pretty: bool) -> Self {
        self.json_pretty = Some(pretty);
        self
    }
}
