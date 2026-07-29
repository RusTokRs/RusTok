use std::fmt;

use async_trait::async_trait;
use thiserror::Error;

use crate::domain::{IndexQuery, SchemaRef};

use super::{
    IndexQueryPage, PostgresQueryDecodeError, PostgresQueryPageBuildError, QueryPlanError,
};

/// Persisted schema state that prevents a compiled query from executing safely.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PersistedSchemaReadinessFailure {
    Missing,
    Inactive,
    FingerprintMismatch,
    ContractMismatch,
}

impl fmt::Display for PersistedSchemaReadinessFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Missing => "missing",
            Self::Inactive => "inactive",
            Self::FingerprintMismatch => "fingerprint mismatch",
            Self::ContractMismatch => "contract mismatch",
        })
    }
}

/// Transport-neutral failure contract for executing one validated Index query.
#[derive(Debug, Error)]
pub enum IndexQueryExecutionError {
    #[error(transparent)]
    Plan(#[from] QueryPlanError),
    #[error(transparent)]
    Build(#[from] PostgresQueryPageBuildError),
    #[error(transparent)]
    Decode(#[from] PostgresQueryDecodeError),
    #[error("Index query execution requires a PostgreSQL connection")]
    UnsupportedBackend,
    #[error("persisted Index schema is not query-ready: {reference} ({reason})")]
    SchemaNotReady {
        reference: SchemaRef,
        reason: PersistedSchemaReadinessFailure,
    },
    #[error("Index query exact-count statement returned no row")]
    MissingExactCountRow,
    #[error("Index query result column {alias} could not be decoded as {expected}")]
    InvalidRowColumn {
        alias: String,
        expected: &'static str,
        details: String,
    },
    #[error("Index query contract preparation failed for {reference}")]
    ContractPreparation {
        reference: SchemaRef,
        details: String,
    },
    #[error("Index query storage operation failed during {operation}")]
    Storage {
        operation: &'static str,
        details: String,
    },
}

impl IndexQueryExecutionError {
    pub(crate) fn storage(operation: &'static str, error: impl fmt::Display) -> Self {
        Self::Storage {
            operation,
            details: error.to_string(),
        }
    }

    pub(crate) fn invalid_row_column(
        alias: impl Into<String>,
        expected: &'static str,
        error: impl fmt::Display,
    ) -> Self {
        Self::InvalidRowColumn {
            alias: alias.into(),
            expected,
            details: error.to_string(),
        }
    }

    pub(crate) fn contract_preparation(
        reference: SchemaRef,
        error: impl fmt::Display,
    ) -> Self {
        Self::ContractPreparation {
            reference,
            details: error.to_string(),
        }
    }
}

/// Owner boundary for executing structured, tenant-scoped Index queries.
///
/// The query already carries tenant and locale scope. Authentication and transport
/// policy remain caller responsibilities; implementations must not widen that scope.
#[async_trait]
pub trait IndexQueryPort: Send + Sync {
    async fn execute_query(
        &self,
        query: IndexQuery,
    ) -> Result<IndexQueryPage, IndexQueryExecutionError>;
}
