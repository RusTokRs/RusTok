use crate::{LocaleCoverageReport, ValidationDiagnostic, ValidationSeverity};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum LandingReadinessCategory {
    Seo,
    Content,
    Routes,
    Locales,
    RuntimeContracts,
}

impl LandingReadinessCategory {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Seo => "seo",
            Self::Content => "content",
            Self::Routes => "routes",
            Self::Locales => "locales",
            Self::RuntimeContracts => "runtime_contracts",
        }
    }

    pub(super) const fn all() -> [Self; 5] {
        [
            Self::Seo,
            Self::Content,
            Self::Routes,
            Self::Locales,
            Self::RuntimeContracts,
        ]
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct LandingReadinessPolicy {
    #[serde(default = "default_true")]
    pub require_page_id: bool,
    #[serde(default = "default_true")]
    pub require_title: bool,
    #[serde(default)]
    pub require_description: bool,
    #[serde(default = "default_true")]
    pub require_slug: bool,
    #[serde(default = "default_true")]
    pub require_h1: bool,
    #[serde(default)]
    pub block_on_warnings: bool,
}

impl Default for LandingReadinessPolicy {
    fn default() -> Self {
        Self {
            require_page_id: true,
            require_title: true,
            require_description: false,
            require_slug: true,
            require_h1: true,
            block_on_warnings: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LandingReadinessIssue {
    pub category: LandingReadinessCategory,
    pub diagnostic: ValidationDiagnostic,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LandingReadinessCategorySummary {
    pub category: LandingReadinessCategory,
    pub error_count: usize,
    pub warning_count: usize,
    pub info_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LandingReadinessReport {
    pub ready: bool,
    pub block_on_warnings: bool,
    pub page_count: usize,
    pub issues: Vec<LandingReadinessIssue>,
    pub categories: Vec<LandingReadinessCategorySummary>,
    pub locale_coverage: LocaleCoverageReport,
}

impl LandingReadinessReport {
    pub fn diagnostics(&self) -> impl Iterator<Item = &ValidationDiagnostic> {
        self.issues.iter().map(|issue| &issue.diagnostic)
    }

    pub fn blocking_issues(&self) -> impl Iterator<Item = &LandingReadinessIssue> {
        let block_on_warnings = self.block_on_warnings;
        self.issues.iter().filter(move |issue| {
            issue.diagnostic.severity == ValidationSeverity::Error
                || (block_on_warnings
                    && issue.diagnostic.severity == ValidationSeverity::Warning)
        })
    }
}

const fn default_true() -> bool {
    true
}
