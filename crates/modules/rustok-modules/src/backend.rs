//! Canonical database-backend contract for the module control plane.
//!
//! PostgreSQL and SQLite are the only supported backends. Stores validate
//! the connection backend once at entry through [`SupportedBackend`] and
//! then dispatch exhaustively on it. A wildcard fallthrough that silently
//! treats an unsupported backend (for example MySQL) as SQLite is
//! forbidden: it would emit SQLite-flavored SQL and value encodings
//! against a different engine and fail — or worse, misbehave — far from
//! the boundary where the configuration mistake was made.

use sea_orm::{DbBackend, DbErr, Value};
use thiserror::Error;
use uuid::Uuid;

/// Database backend validated at a store boundary. Matching on this type
/// is exhaustive, so SQL construction never needs a panic or a guess for
/// the unsupported case.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SupportedBackend {
    Postgres,
    Sqlite,
}

/// The connection uses a backend this crate does not support.
#[derive(Debug, Error)]
#[error(
    "unsupported database backend for the module control plane: {0:?} \
     (only PostgreSQL and SQLite are supported)"
)]
pub(crate) struct UnsupportedBackend(pub DbBackend);

impl From<UnsupportedBackend> for DbErr {
    fn from(error: UnsupportedBackend) -> Self {
        DbErr::Custom(error.to_string())
    }
}

impl SupportedBackend {
    pub(crate) fn new(backend: DbBackend) -> Result<Self, UnsupportedBackend> {
        match backend {
            DbBackend::Postgres => Ok(Self::Postgres),
            DbBackend::Sqlite => Ok(Self::Sqlite),
            other => Err(UnsupportedBackend(other)),
        }
    }

    pub(crate) fn db_backend(self) -> DbBackend {
        match self {
            Self::Postgres => DbBackend::Postgres,
            Self::Sqlite => DbBackend::Sqlite,
        }
    }

    /// Positional bind-parameter placeholder (`$n` on PostgreSQL, `?n` on
    /// SQLite).
    pub(crate) fn placeholder(self, position: usize) -> String {
        match self {
            Self::Postgres => format!("${position}"),
            Self::Sqlite => format!("?{position}"),
        }
    }

    /// Placeholder prefix for composing parameters inside larger format
    /// strings (`{prefix}1`, `{prefix}2`, ...).
    pub(crate) fn placeholder_prefix(self) -> &'static str {
        match self {
            Self::Postgres => "$",
            Self::Sqlite => "?",
        }
    }

    /// SQL expression for the database's current timestamp in the
    /// representation this crate persists: native `NOW()` on PostgreSQL,
    /// `datetime('now')` text on SQLite.
    pub(crate) fn now_sql(self) -> &'static str {
        match self {
            Self::Postgres => "NOW()",
            Self::Sqlite => "datetime('now')",
        }
    }

    /// UUID bind value in the persisted representation: native `uuid` on
    /// PostgreSQL, hyphenated text on SQLite.
    pub(crate) fn uuid_value(self, value: Uuid) -> Value {
        match self {
            Self::Postgres => Value::Uuid(Some(value)),
            Self::Sqlite => value.to_string().into(),
        }
    }
}
