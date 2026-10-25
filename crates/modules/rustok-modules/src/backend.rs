//! Canonical database-backend contract for the module control plane.
//!
//! PostgreSQL and SQLite are the only supported backends. Stores validate
//! the connection backend once at entry through [`SupportedBackend`] and
//! then dispatch exhaustively on it. A wildcard fallthrough that silently
//! treats an unsupported backend (for example MySQL) as SQLite is
//! forbidden: it would emit SQLite-flavored SQL and value encodings
//! against a different engine and fail — or worse, misbehave — far from
//! the boundary where the configuration mistake was made.

use sea_orm::{ConnectionTrait, DbBackend, DbErr, QueryResult, Statement, Value};
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

    /// Classifies only a durable uniqueness violation. Callers use this to
    /// distinguish an idempotent insert race from a database outage or an
    /// unrelated constraint failure; every other database error remains an
    /// infrastructure failure and must not be replayed as a conflict.
    pub(crate) fn is_unique_violation(self, error: &DbErr) -> bool {
        let message = error.to_string().to_ascii_lowercase();
        match self {
            Self::Postgres => {
                message.contains("23505")
                    || message.contains("duplicate key value violates unique constraint")
            }
            Self::Sqlite => message.contains("unique constraint failed"),
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

    /// SQL expression for the database's current timestamp: native
    /// `NOW()` on PostgreSQL, RFC 3339 text on SQLite.
    ///
    /// SQLite has exactly one canonical timestamp text representation in
    /// this crate — RFC 3339 with millisecond precision and a `+00:00`
    /// offset — because that is the representation sqlx binds for
    /// `DateTime<Utc>` values (see [`Self::datetime_value`]). SQL-side
    /// timestamps must share it: the space-separated text produced by
    /// `datetime('now')`/`CURRENT_TIMESTAMP` sorts before every RFC 3339
    /// value of the same calendar day (`' ' < 'T'`), so lexical
    /// comparisons across the two representations (lease expiry,
    /// availability, retention cutoffs) silently evaluate as if the
    /// RFC 3339 side were up to a day in the future. Every SQL-side
    /// timestamp producer goes through this owner or a `strftime` call
    /// with the same format string; [`parse_sqlite_datetime`] still
    /// accepts the legacy space-separated form when reading.
    pub(crate) fn now_sql(self) -> &'static str {
        match self {
            Self::Postgres => "NOW()",
            Self::Sqlite => "strftime('%Y-%m-%dT%H:%M:%f+00:00', 'now')",
        }
    }

    /// Bind representation for a timestamp: native `timestamptz` on
    /// PostgreSQL, RFC 3339 text on SQLite (the canonical text
    /// representation this crate's readers parse back; see
    /// [`Self::now_sql`]).
    pub(crate) fn datetime_value(self, value: chrono::DateTime<chrono::Utc>) -> Value {
        match self {
            Self::Postgres => Value::ChronoDateTimeUtc(Some(value)),
            Self::Sqlite => sqlite_datetime_text(value).into(),
        }
    }

    /// [`Self::datetime_value`] for nullable timestamp columns.
    pub(crate) fn optional_datetime_value(
        self,
        value: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Value {
        match (self, value) {
            (Self::Postgres, value) => Value::ChronoDateTimeUtc(value),
            (Self::Sqlite, Some(value)) => sqlite_datetime_text(value).into(),
            (Self::Sqlite, None) => Value::String(None),
        }
    }

    pub(crate) fn uuid_value(self, value: Uuid) -> Value {
        match self {
            Self::Postgres => Value::Uuid(Some(value)),
            Self::Sqlite => value.to_string().into(),
        }
    }

    /// [`Self::uuid_value`] for nullable UUID columns.
    pub(crate) fn optional_uuid_value(self, value: Option<Uuid>) -> Value {
        match (self, value) {
            (Self::Postgres, value) => Value::Uuid(value),
            (Self::Sqlite, Some(value)) => value.to_string().into(),
            (Self::Sqlite, None) => Value::String(None),
        }
    }

    /// Bind representation for a boolean: native `BOOLEAN` on PostgreSQL
    /// and the integer representation used by this crate's SQLite schema.
    pub(crate) fn bool_value(self, value: bool) -> Value {
        match self {
            Self::Postgres => Value::Bool(Some(value)),
            Self::Sqlite => Value::Int(Some(i32::from(value))),
        }
    }

    /// Decode a required UUID using the backend's native representation.
    /// This is the canonical row decoder for new control-plane code; a
    /// PostgreSQL `uuid` must never be decoded through `String`.
    ///
    /// SQLite schemas in this crate use both SeaORM's native UUID storage
    /// and canonical hyphenated text. Accept the native representation
    /// first and fall back to text so one decoder remains correct across
    /// those durable schemas.
    pub(crate) fn uuid_from_row(
        self,
        row: &QueryResult,
        column: &str,
    ) -> Result<Uuid, DbErr> {
        match self {
            Self::Postgres => row.try_get("", column),
            Self::Sqlite => match row.try_get::<Uuid>("", column) {
                Ok(value) => Ok(value),
                Err(native_error) => {
                    let value: String = row.try_get("", column).map_err(|_| native_error)?;
                    Uuid::parse_str(&value).map_err(|error| {
                        DbErr::Custom(format!(
                            "invalid UUID in SQLite column `{column}`: {error}"
                        ))
                    })
                }
            },
        }
    }

    /// Decode a nullable UUID using the backend's native representation.
    /// SQLite accepts the same native-or-text representations as
    /// [`Self::uuid_from_row`].
    pub(crate) fn optional_uuid_from_row(
        self,
        row: &QueryResult,
        column: &str,
    ) -> Result<Option<Uuid>, DbErr> {
        match self {
            Self::Postgres => row.try_get("", column),
            Self::Sqlite => match row.try_get::<Option<Uuid>>("", column) {
                Ok(value) => Ok(value),
                Err(native_error) => {
                    let value: Option<String> =
                        row.try_get("", column).map_err(|_| native_error)?;
                    value
                        .as_deref()
                        .map(|value| {
                            Uuid::parse_str(value).map_err(|error| {
                                DbErr::Custom(format!(
                                    "invalid UUID in SQLite column `{column}`: {error}"
                                ))
                            })
                        })
                        .transpose()
                }
            },
        }
    }

    /// Decode a required boolean from either native PostgreSQL `BOOLEAN`
    /// or SQLite's integer/boolean storage.
    pub(crate) fn bool_from_row(
        self,
        row: &QueryResult,
        column: &str,
    ) -> Result<bool, DbErr> {
        match self {
            Self::Postgres => row.try_get("", column),
            Self::Sqlite => match row.try_get::<i64>("", column) {
                Ok(value) => Ok(value != 0),
                Err(_) => row.try_get("", column),
            },
        }
    }

    /// Decode a required UTC timestamp from native `timestamptz` or the
    /// canonical SQLite RFC 3339 text representation.
    pub(crate) fn datetime_from_row(
        self,
        row: &QueryResult,
        column: &str,
    ) -> Result<chrono::DateTime<chrono::Utc>, DbErr> {
        match self {
            Self::Postgres => row.try_get("", column),
            Self::Sqlite => {
                let value: String = row.try_get("", column)?;
                parse_sqlite_datetime(&value).map_err(|error| {
                    DbErr::Custom(format!(
                        "invalid timestamp in SQLite column `{column}`: {error}"
                    ))
                })
            }
        }
    }

    /// Decode a nullable UTC timestamp.
    pub(crate) fn optional_datetime_from_row(
        self,
        row: &QueryResult,
        column: &str,
    ) -> Result<Option<chrono::DateTime<chrono::Utc>>, DbErr> {
        match self {
            Self::Postgres => row.try_get("", column),
            Self::Sqlite => {
                let value: Option<String> = row.try_get("", column)?;
                value
                    .as_deref()
                    .map(|value| {
                        parse_sqlite_datetime(value).map_err(|error| {
                            DbErr::Custom(format!(
                                "invalid timestamp in SQLite column `{column}`: {error}"
                            ))
                        })
                    })
                    .transpose()
            }
        }
    }
}

/// Decode a stored count or byte length. Signed storage must never be cast
/// directly to `u64`: a corrupt negative value would wrap into a plausible
/// enormous count instead of failing closed at the row boundary.
pub(crate) fn nonnegative_count_from_row(
    row: &QueryResult,
    column: &str,
) -> Result<u64, DbErr> {
    let value: i64 = row.try_get("", column)?;
    u64::try_from(value).map_err(|_| {
        DbErr::Custom(format!("stored count in column `{column}` is negative"))
    })
}

/// Parse a SQLite text timestamp written by this crate. The canonical
/// representation is RFC 3339 ([`SupportedBackend::datetime_value`] and
/// [`SupportedBackend::now_sql`]); the space-separated fallback only
/// reads rows written before the representations were unified (and by
/// `CURRENT_TIMESTAMP` column defaults). It runs only when RFC 3339
/// parsing fails, so it cannot reinterpret a valid RFC 3339 value.
/// The one canonical SQLite timestamp text representation: RFC 3339 with
/// fixed millisecond precision and a `+00:00` offset, byte-identical to
/// what [`SupportedBackend::now_sql`] produces inside SQL. Variable
/// precision (`DateTime::to_rfc3339` emits 0/3/6/9 fractional digits) sorts
/// *almost* consistently, but equal instants in different precisions
/// order differently, which lease/retention cutoffs must never observe.
/// Every SQLite timestamp bind goes through this formatter; the matching
/// reader is [`parse_sqlite_datetime`].
pub(crate) fn sqlite_datetime_text(value: chrono::DateTime<chrono::Utc>) -> String {
    value.format("%Y-%m-%dT%H:%M:%S%.3f+00:00").to_string()
}

pub(crate) fn parse_sqlite_datetime(
    value: &str,
) -> Result<chrono::DateTime<chrono::Utc>, chrono::ParseError> {
    chrono::DateTime::parse_from_rfc3339(value)
        .map(|timestamp| timestamp.with_timezone(&chrono::Utc))
        .or_else(|_| {
            chrono::NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S")
                .map(|timestamp| timestamp.and_utc())
        })
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
