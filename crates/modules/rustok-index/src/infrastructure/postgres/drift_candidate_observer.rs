use std::{fmt, str::FromStr};

use async_trait::async_trait;
use rustok_core::ModuleRuntimeExtensions;
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, QueryResult, Statement};
use thiserror::Error;

use crate::{
    IndexDriftCandidate, IndexDriftCandidateConfirmationFailure, IndexDriftCandidateConfirmer,
    IndexDriftCandidateMaterializedObservation, IndexDriftCandidateMaterializedObserver,
    IndexDriftOrphanLinkCandidate, IndexDriftStaleEntityCandidate,
    SharedIndexSourceAbsenceRegistry, SharedIndexSourceRegistry,
};

const BACKEND_UNSUPPORTED: &str = "index_drift_candidate_confirmation_backend_unsupported";
const STORAGE_UNAVAILABLE: &str = "index_drift_candidate_confirmation_storage_unavailable";
const MATERIALIZED_INVALID: &str = "index_drift_candidate_confirmation_materialized_invalid";

#[derive(Clone)]
pub struct PostgresIndexDriftCandidateMaterializedObserver {
    db: DatabaseConnection,
}

impl PostgresIndexDriftCandidateMaterializedObserver {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    async fn observe_stale_entity(
        &self,
        candidate: &IndexDriftStaleEntityCandidate,
    ) -> Result<IndexDriftCandidateMaterializedObservation, IndexDriftCandidateConfirmationFailure>
    {
        let key = candidate.key();
        let row = self
            .db
            .query_one_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                "SELECT CAST(source_version AS TEXT) AS source_version_text, is_deleted FROM index_entities WHERE tenant_id = $1 AND module_name = $2 AND entity_name = $3 AND schema_version = $4 AND entity_id = $5 AND locale_key = $6 LIMIT 1",
                vec![
                    key.tenant_id.into(),
                    key.schema.module.as_str().to_owned().into(),
                    key.schema.entity.as_str().to_owned().into(),
                    i64::from(key.schema.version.get()).into(),
                    key.entity_id.into(),
                    persisted_locale(key.locale.as_ref()).into(),
                ],
            ))
            .await
            .map_err(|_| retryable_failure(STORAGE_UNAVAILABLE))?;
        let Some(row) = row else {
            return Ok(IndexDriftCandidateMaterializedObservation::Changed);
        };
        let source_version = stored_source_version(&row)?;
        let is_deleted = row
            .try_get::<bool>("", "is_deleted")
            .map_err(|_| permanent_failure(MATERIALIZED_INVALID))?;
        if !is_deleted && source_version == candidate.indexed_source_version() {
            Ok(IndexDriftCandidateMaterializedObservation::Unchanged)
        } else {
            Ok(IndexDriftCandidateMaterializedObservation::Changed)
        }
    }

    async fn observe_orphan_link(
        &self,
        candidate: &IndexDriftOrphanLinkCandidate,
    ) -> Result<IndexDriftCandidateMaterializedObservation, IndexDriftCandidateConfirmationFailure>
    {
        let source = candidate.source_key();
        let target = candidate.target();
        let row = self
            .db
            .query_one_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                "SELECT EXISTS (SELECT 1 FROM index_entities s JOIN index_links l ON l.tenant_id = s.tenant_id AND l.source_module = s.module_name AND l.source_entity = s.entity_name AND l.source_schema_version = s.schema_version AND l.source_entity_id = s.entity_id AND l.source_locale_key = s.locale_key AND l.source_version = s.source_version LEFT JOIN index_entities t ON t.tenant_id = l.tenant_id AND t.module_name = l.target_module AND t.entity_name = l.target_entity AND t.schema_version = l.target_schema_version AND t.entity_id = l.target_entity_id AND t.locale_key = l.target_locale_key WHERE s.tenant_id = $1 AND s.module_name = $2 AND s.entity_name = $3 AND s.schema_version = $4 AND s.entity_id = $5 AND s.locale_key = $6 AND s.is_deleted = FALSE AND CAST(s.source_version AS TEXT) = $7 AND l.link_name = $8 AND l.ordinal = $9 AND l.target_module = $10 AND l.target_entity = $11 AND l.target_schema_version = $12 AND l.target_entity_id = $13 AND l.target_locale_key = $14 AND (t.tenant_id IS NULL OR (t.is_deleted = TRUE AND t.source_version > 0))) AS candidate_matches",
                vec![
                    source.tenant_id.into(),
                    source.schema.module.as_str().to_owned().into(),
                    source.schema.entity.as_str().to_owned().into(),
                    i64::from(source.schema.version.get()).into(),
                    source.entity_id.into(),
                    persisted_locale(source.locale.as_ref()).into(),
                    candidate.indexed_source_version().to_string().into(),
                    candidate.link_name().as_str().to_owned().into(),
                    i64::from(candidate.ordinal()).into(),
                    target.schema.module.as_str().to_owned().into(),
                    target.schema.entity.as_str().to_owned().into(),
                    i64::from(target.schema.version.get()).into(),
                    target.entity_id.into(),
                    persisted_locale(target.locale.as_ref()).into(),
                ],
            ))
            .await
            .map_err(|_| retryable_failure(STORAGE_UNAVAILABLE))?
            .ok_or_else(|| retryable_failure(STORAGE_UNAVAILABLE))?;
        let matches = row
            .try_get::<bool>("", "candidate_matches")
            .map_err(|_| permanent_failure(MATERIALIZED_INVALID))?;
        Ok(if matches {
            IndexDriftCandidateMaterializedObservation::Unchanged
        } else {
            IndexDriftCandidateMaterializedObservation::Changed
        })
    }
}

impl fmt::Debug for PostgresIndexDriftCandidateMaterializedObserver {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PostgresIndexDriftCandidateMaterializedObserver")
            .finish_non_exhaustive()
    }
}

#[async_trait]
impl IndexDriftCandidateMaterializedObserver for PostgresIndexDriftCandidateMaterializedObserver {
    async fn observe_candidate(
        &self,
        candidate: &IndexDriftCandidate,
    ) -> Result<IndexDriftCandidateMaterializedObservation, IndexDriftCandidateConfirmationFailure>
    {
        if self.db.get_database_backend() != DbBackend::Postgres {
            return Err(permanent_failure(BACKEND_UNSUPPORTED));
        }
        match candidate {
            IndexDriftCandidate::StaleEntity(candidate) => {
                self.observe_stale_entity(candidate).await
            }
            IndexDriftCandidate::OrphanLink(candidate) => self.observe_orphan_link(candidate).await,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum IndexDriftCandidateObserverCompositionError {
    #[error("PostgreSQL Index drift candidate observer does not support this database backend")]
    UnsupportedBackend,
}

/// Constructs the internal materialized observer without executing SQL or publishing a transport.
pub fn materialize_postgres_index_drift_candidate_observer(
    db: DatabaseConnection,
) -> Result<
    PostgresIndexDriftCandidateMaterializedObserver,
    IndexDriftCandidateObserverCompositionError,
> {
    if db.get_database_backend() != DbBackend::Postgres {
        return Err(IndexDriftCandidateObserverCompositionError::UnsupportedBackend);
    }
    Ok(PostgresIndexDriftCandidateMaterializedObserver::new(db))
}

/// Constructs the internal confirmer from frozen source registries without publishing it.
pub fn materialize_postgres_index_drift_candidate_confirmer(
    extensions: &ModuleRuntimeExtensions,
    db: DatabaseConnection,
) -> Result<Option<IndexDriftCandidateConfirmer>, IndexDriftCandidateObserverCompositionError> {
    let Some(sources) = extensions.get::<SharedIndexSourceRegistry>().cloned() else {
        return Ok(None);
    };
    let observer = materialize_postgres_index_drift_candidate_observer(db)?;
    let confirmer = IndexDriftCandidateConfirmer::new(sources, observer);
    Ok(Some(
        match extensions
            .get::<SharedIndexSourceAbsenceRegistry>()
            .cloned()
        {
            Some(absence) => confirmer.with_absence_registry(absence),
            None => confirmer,
        },
    ))
}

fn stored_source_version(row: &QueryResult) -> Result<u64, IndexDriftCandidateConfirmationFailure> {
    let value = row
        .try_get::<String>("", "source_version_text")
        .map_err(|_| permanent_failure(MATERIALIZED_INVALID))?;
    let source_version = u64::from_str(&value)
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(|| permanent_failure(MATERIALIZED_INVALID))?;
    Ok(source_version)
}

fn persisted_locale(locale: Option<&crate::LocaleKey>) -> String {
    locale
        .map(|value| value.as_str().to_owned())
        .unwrap_or_default()
}

fn retryable_failure(code: &str) -> IndexDriftCandidateConfirmationFailure {
    IndexDriftCandidateConfirmationFailure::retryable(code)
        .expect("static candidate confirmation failure code is valid")
}

fn permanent_failure(code: &str) -> IndexDriftCandidateConfirmationFailure {
    IndexDriftCandidateConfirmationFailure::permanent(code)
        .expect("static candidate confirmation failure code is valid")
}
