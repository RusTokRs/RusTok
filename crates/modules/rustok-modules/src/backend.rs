//! Canonical database-backend contract for the module control plane.
//!
//! PostgreSQL and SQLite are the only supported backends. Stores validate
//! the connection backend once at entry through [`SupportedBackend`] and
//! then dispatch exhaustively on it. A wildcard fallthrough that silently
//! treats an unsupported backend (for example MySQL) as SQLite is
//! forbidden: it would emit SQLite-flavored SQL and value encodings
//! against a different engine and fail — or worse, misbehave — far from
//! the boundary where the configuration mistake was made.

use sea_orm::{ConnectionTrait, DbBackend, DbErr, Statement, Value};
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
    /// representation most of this crate persists: native `NOW()` on
    /// PostgreSQL, `datetime('now')` text (`YYYY-MM-DD HH:MM:SS`) on
    /// SQLite.
    pub(crate) fn now_sql(self) -> &'static str {
        match self {
            Self::Postgres => "NOW()",
            Self::Sqlite => "datetime('now')",
        }
    }

    /// SQL expression for the database's current timestamp in the RFC3339
    /// representation the artifact-settings-recovery family persists and
    /// parses back with `DateTime::parse_from_rfc3339`: native `NOW()` on
    /// PostgreSQL, `strftime('%Y-%m-%dT%H:%M:%fZ', 'now')` text on SQLite.
    ///
    /// The two representations must never be mixed within one column:
    /// `now_sql` text is not RFC3339-parseable, and lexical comparison
    /// across the formats does not order chronologically.
    pub(crate) fn now_rfc3339_sql(self) -> &'static str {
        match self {
            Self::Postgres => "NOW()",
            Self::Sqlite => "strftime('%Y-%m-%dT%H:%M:%fZ', 'now')",
        }
    }

    /// UUID bind value in the persisted representation: native `uuid` on
    /// PostgreSQL, hyphenated text on SQLite.
    /// Bind representation for a timestamp: native `timestamptz` on
    /// PostgreSQL, RFC 3339 text on SQLite (the representation this
    /// crate's text-column readers parse back).
    pub(crate) fn datetime_value(self, value: chrono::DateTime<chrono::Utc>) -> Value {
        match self {
            Self::Postgres => Value::ChronoDateTimeUtc(Some(value)),
            Self::Sqlite => value.to_rfc3339().into(),
        }
    }

    pub(crate) fn uuid_value(self, value: Uuid) -> Value {
        match self {
            Self::Postgres => Value::Uuid(Some(value)),
            Self::Sqlite => value.to_string().into(),
        }
    }
}

/// Set the transaction-local tenant RLS scope (`rustok.tenant_id`) on
/// PostgreSQL as the first statement of a tenant-scoped transaction.
///
/// This is the single owner of the GUC name and value encoding; subsystem
/// helpers only adapt the error type. SQLite has no RLS, so the call is a
/// no-op there — but the backend is still validated, so an unsupported
/// backend fails loudly instead of silently skipping scoping. Until the
/// application connects as a non-owner role, application-level `WHERE`
/// scoping remains the authoritative tenant boundary and this setting is
/// defense-in-depth (see the Phase F record in the remediation plan).
pub(crate) async fn set_tenant_scope<C: ConnectionTrait>(
    connection: &C,
    tenant_id: Uuid,
) -> Result<(), DbErr> {
    if SupportedBackend::new(connection.get_database_backend())? == SupportedBackend::Postgres {
        connection
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                "SELECT set_config('rustok.tenant_id', $1, true)",
                vec![tenant_id.to_string().into()],
            ))
            .await?;
    }
    Ok(())
}

/// Set the transaction-local platform RLS scope
/// (`rustok.module_control_plane_owner = 'platform'`) on PostgreSQL as
/// the first statement of a platform-scoped transaction. A platform
/// transaction legitimately examines rows across tenants (installation,
/// distribution, governance, recovery sweeps); the module-control-plane
/// RLS policies accept this setting as the owner assertion. Same SQLite
/// and interim-contract semantics as [`set_tenant_scope`].
pub(crate) async fn set_platform_scope<C: ConnectionTrait>(connection: &C) -> Result<(), DbErr> {
    if SupportedBackend::new(connection.get_database_backend())? == SupportedBackend::Postgres {
        connection
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                "SELECT set_config('rustok.module_control_plane_owner', $1, true)",
                vec!["platform".into()],
            ))
            .await?;
    }
    Ok(())
}
