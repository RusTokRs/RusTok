use std::sync::Arc;
use std::time::{Duration, Instant};

use sea_orm::{
    ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbBackend, Statement,
    TransactionTrait,
};
use uuid::Uuid;

use rustok_core::search_projection::SearchProjectionRefreshScope;
use rustok_core::{Error, Result};
use rustok_telemetry::metrics;

use crate::{MAX_SEARCH_PROJECTION_PAGE_SIZE, SearchProjectionDocument, SearchProjectionSource};

/// Temporary staging table used to make one scope replacement atomic.
///
/// `ON COMMIT DROP` keeps it session-local, so two owners rebuilding at the same
/// time cannot collide even though the name is fixed.
const STAGE_TABLE: &str = "search_projection_stage";
const MAX_TARGET_ENTITY_DOCUMENTS: usize = 32;

/// Search-owned storage driver for one module's projection source.
///
/// Ownership split: the source module decides *which* documents exist and what
/// they contain (`SearchProjectionSource`), while Search owns *how* they are
/// stored in `search_documents`. The driver therefore knows nothing about any
/// module: it validates against `source.source_module()`, scopes deletes to that
/// module and resolves the refresh width through `source.refresh_scope`, so a new
/// owner only registers its source and gets the same transactional guarantees.
#[derive(Clone)]
pub(crate) struct SearchProjectionDriver {
    db: DatabaseConnection,
    source: Arc<dyn SearchProjectionSource>,
}

impl SearchProjectionDriver {
    pub(crate) fn new(db: DatabaseConnection, source: Arc<dyn SearchProjectionSource>) -> Self {
        Self { db, source }
    }

    pub(crate) fn source_module(&self) -> &'static str {
        self.source.source_module()
    }

    pub(crate) async fn rebuild_tenant(&self, tenant_id: Uuid) -> Result<()> {
        self.ensure_postgres()?;
        let started_at = Instant::now();
        let tx = self.db.begin().await.map_err(Error::Database)?;
        let result = async {
            self.create_stage(&tx).await?;
            let mut cursor = None;
            loop {
                let page = self
                    .source
                    .list_public_documents(
                        tenant_id,
                        cursor.clone(),
                        MAX_SEARCH_PROJECTION_PAGE_SIZE,
                    )
                    .await?;
                if page.documents.len() > MAX_SEARCH_PROJECTION_PAGE_SIZE {
                    return Err(Error::Validation(format!(
                        "{} Search projection source exceeded the bounded page size",
                        self.source.source_module()
                    )));
                }
                for document in page.documents {
                    self.validate_document(tenant_id, &document)?;
                    insert_document(&tx, STAGE_TABLE, document).await?;
                }

                match page.next_cursor {
                    Some(next_cursor) if cursor.as_deref() != Some(next_cursor.as_str()) => {
                        cursor = Some(next_cursor);
                    }
                    Some(_) => {
                        return Err(Error::Validation(format!(
                            "{} Search projection cursor did not advance",
                            self.source.source_module()
                        )));
                    }
                    None => break,
                }
            }

            self.delete_scope(&tx, tenant_id).await?;
            tx.execute_unprepared(&format!(
                r#"
                INSERT INTO search_documents (
                    document_key, tenant_id, document_id, source_module, entity_type,
                    locale, status, is_public, title, subtitle, slug, handle, body,
                    keywords_text, facets, payload, published_at, created_at, updated_at
                )
                SELECT
                    document_key, tenant_id, document_id, source_module, entity_type,
                    locale, status, is_public, title, subtitle, slug, handle, body,
                    keywords_text, facets, payload, published_at, created_at, updated_at
                FROM {STAGE_TABLE}
                "#
            ))
            .await
            .map_err(Error::Database)?;
            tx.commit().await.map_err(Error::Database)
        }
        .await;
        record_operation(
            "rebuild_projection_scope",
            self.source.source_module(),
            tenant_id,
            &result,
            started_at.elapsed(),
        );
        result
    }

    pub(crate) async fn refresh_entity(
        &self,
        tenant_id: Uuid,
        entity_type: &str,
        entity_id: Uuid,
    ) -> Result<()> {
        self.ensure_postgres()?;
        if entity_type.trim().is_empty() {
            return Err(Error::Validation(
                "Search projection entity type must not be empty".to_string(),
            ));
        }
        // The owner declares how wide one entity refresh has to be; Search does not
        // infer it from an entity-type name.
        if self.source.refresh_scope(entity_type) == SearchProjectionRefreshScope::Tenant {
            return self.rebuild_tenant(tenant_id).await;
        }
        let started_at = Instant::now();
        let documents = self
            .source
            .load_public_entity(tenant_id, entity_type, entity_id)
            .await?;
        if documents.len() > MAX_TARGET_ENTITY_DOCUMENTS {
            return Err(Error::Validation(format!(
                "{} Search projection entity exceeded {MAX_TARGET_ENTITY_DOCUMENTS} locale documents",
                self.source.source_module()
            )));
        }

        let tx = self.db.begin().await.map_err(Error::Database)?;
        let result = async {
            self.delete_entity(&tx, tenant_id, entity_type, entity_id)
                .await?;
            for document in documents {
                self.validate_document(tenant_id, &document)?;
                if document.entity_type != entity_type || document.document_id != entity_id {
                    return Err(Error::Validation(format!(
                        "{} Search projection entity response changed target identity",
                        self.source.source_module()
                    )));
                }
                insert_document(&tx, "search_documents", document).await?;
            }
            tx.commit().await.map_err(Error::Database)
        }
        .await;
        record_operation(
            "refresh_projection_entity",
            self.source.source_module(),
            tenant_id,
            &result,
            started_at.elapsed(),
        );
        result
    }

    pub(crate) async fn delete_tenant(&self, tenant_id: Uuid) -> Result<()> {
        self.ensure_postgres()?;
        let started_at = Instant::now();
        let result = self.delete_scope(&self.db, tenant_id).await;
        record_operation(
            "delete_projection_scope",
            self.source.source_module(),
            tenant_id,
            &result,
            started_at.elapsed(),
        );
        result
    }

    async fn create_stage(&self, tx: &DatabaseTransaction) -> Result<()> {
        tx.execute_unprepared(&format!(
            r#"
            CREATE TEMP TABLE {STAGE_TABLE}
            (LIKE search_documents INCLUDING DEFAULTS)
            ON COMMIT DROP
            "#
        ))
        .await
        .map_err(Error::Database)?;
        Ok(())
    }

    fn ensure_postgres(&self) -> Result<()> {
        if self.db.get_database_backend() != DbBackend::Postgres {
            return Err(Error::External(format!(
                "SearchProjectionDriver requires PostgreSQL backend for `{}`",
                self.source.source_module()
            )));
        }
        Ok(())
    }

    fn validate_document(&self, tenant_id: Uuid, document: &SearchProjectionDocument) -> Result<()> {
        if document.tenant_id != tenant_id
            || document.source_module != self.source.source_module()
            || !document.is_public
        {
            return Err(Error::Validation(format!(
                "{} Search projection source returned a foreign or non-public document",
                self.source.source_module()
            )));
        }
        // A module cannot seed the storage with an unbounded or blank entity type;
        // which types exist is the owner's contract, not Search's list.
        if document.entity_type.trim().is_empty() || document.entity_type.len() > 64 {
            return Err(Error::Validation(
                "Search projection document must declare a bounded entity type".to_string(),
            ));
        }
        if document.document_key.is_empty()
            || document.document_key.len() > 200
            || document.locale.trim().is_empty()
            || document.locale.len() > 16
            || document.status.trim().is_empty()
            || document.status.len() > 32
        {
            return Err(Error::Validation(
                "Search projection document exceeded storage bounds".to_string(),
            ));
        }
        Ok(())
    }

    /// Deletes every stored document of this source module for one tenant.
    ///
    /// Scoping by `source_module` (instead of a hard-coded module name and entity
    /// list) is what keeps a rebuild of one owner from touching another owner's
    /// documents.
    async fn delete_scope<C>(&self, conn: &C, tenant_id: Uuid) -> Result<()>
    where
        C: ConnectionTrait,
    {
        let statement = Statement::from_sql_and_values(
            DbBackend::Postgres,
            "DELETE FROM search_documents WHERE tenant_id = $1 AND source_module = $2",
            vec![tenant_id.into(), self.source.source_module().into()],
        );
        conn.execute_raw(statement).await.map_err(Error::Database)?;
        Ok(())
    }

    async fn delete_entity<C>(
        &self,
        conn: &C,
        tenant_id: Uuid,
        entity_type: &str,
        entity_id: Uuid,
    ) -> Result<()>
    where
        C: ConnectionTrait,
    {
        let statement = Statement::from_sql_and_values(
            DbBackend::Postgres,
            "DELETE FROM search_documents WHERE tenant_id = $1 AND source_module = $2 AND entity_type = $3 AND document_id = $4",
            vec![
                tenant_id.into(),
                self.source.source_module().into(),
                entity_type.to_string().into(),
                entity_id.into(),
            ],
        );
        conn.execute_raw(statement).await.map_err(Error::Database)?;
        Ok(())
    }
}

async fn insert_document<C>(conn: &C, table: &str, document: SearchProjectionDocument) -> Result<()>
where
    C: ConnectionTrait,
{
    let sql = format!(
        r#"
        INSERT INTO {table} (
            document_key, tenant_id, document_id, source_module, entity_type,
            locale, status, is_public, title, subtitle, slug, handle, body,
            keywords_text, facets, payload, published_at, created_at, updated_at
        ) VALUES (
            $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13,
            $14, $15, $16, $17, $18, $19
        )
        "#
    );
    let statement = Statement::from_sql_and_values(
        DbBackend::Postgres,
        sql,
        vec![
            document.document_key.into(),
            document.tenant_id.into(),
            document.document_id.into(),
            document.source_module.into(),
            document.entity_type.into(),
            document.locale.into(),
            document.status.into(),
            document.is_public.into(),
            document.title.into(),
            document.subtitle.into(),
            document.slug.into(),
            document.handle.into(),
            document.body.into(),
            document.keywords_text.into(),
            document.facets.into(),
            document.payload.into(),
            document.published_at.into(),
            document.created_at.into(),
            document.updated_at.into(),
        ],
    );
    conn.execute_raw(statement).await.map_err(Error::Database)?;
    Ok(())
}

fn record_operation(
    operation: &str,
    source_module: &str,
    tenant_id: Uuid,
    result: &Result<()>,
    duration: Duration,
) {
    let status = if result.is_ok() { "success" } else { "error" };
    metrics::record_search_indexing_operation(
        operation,
        "projection_driver",
        status,
        duration.as_secs_f64(),
    );
    if let Err(error) = result {
        metrics::record_module_error("search", "projection_driver", "error");
        tracing::error!(
            operation,
            source_module,
            tenant_id = %tenant_id,
            error = %error,
            duration_ms = duration.as_millis() as u64,
            "Search projection operation failed"
        );
    }
}
