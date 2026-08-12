use std::sync::Arc;
use std::time::{Duration, Instant};

use sea_orm::{
    ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbBackend, Statement,
    TransactionTrait,
};
use uuid::Uuid;

use rustok_core::{Error, Result};
use rustok_telemetry::metrics;

use crate::{MAX_SEARCH_PROJECTION_PAGE_SIZE, SearchProjectionDocument, SearchProjectionSource};

const FORUM_SOURCE_MODULE: &str = "forum";
const FORUM_CATEGORY_ENTITY_TYPE: &str = "forum_category";
const FORUM_TOPIC_ENTITY_TYPE: &str = "forum_topic";
const FORUM_REPLY_ENTITY_TYPE: &str = "forum_reply";
const MAX_TARGET_ENTITY_DOCUMENTS: usize = 32;
const STAGE_TABLE: &str = "forum_search_projection_stage";

#[derive(Clone)]
pub(crate) struct ForumSearchProjector {
    db: DatabaseConnection,
    source: Arc<dyn SearchProjectionSource>,
}

impl ForumSearchProjector {
    pub(crate) fn new(db: DatabaseConnection, source: Arc<dyn SearchProjectionSource>) -> Self {
        Self { db, source }
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
                    return Err(Error::Validation(
                        "Forum Search projection source exceeded the bounded page size".to_string(),
                    ));
                }
                for document in page.documents {
                    validate_document(tenant_id, &document)?;
                    insert_document(&tx, STAGE_TABLE, document).await?;
                }

                match page.next_cursor {
                    Some(next_cursor) if cursor.as_deref() != Some(next_cursor.as_str()) => {
                        cursor = Some(next_cursor);
                    }
                    Some(_) => {
                        return Err(Error::Validation(
                            "Forum Search projection cursor did not advance".to_string(),
                        ));
                    }
                    None => break,
                }
            }

            delete_forum_scope(&tx, tenant_id).await?;
            tx.execute_unprepared(
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
                FROM forum_search_projection_stage
                "#,
            )
            .await
            .map_err(Error::Database)?;
            tx.commit().await.map_err(Error::Database)
        }
        .await;
        record_operation(
            "rebuild_forum_scope",
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
        validate_entity_type(entity_type)?;
        if entity_type == FORUM_TOPIC_ENTITY_TYPE {
            return self.rebuild_tenant(tenant_id).await;
        }
        let started_at = Instant::now();
        let documents = self
            .source
            .load_public_entity(tenant_id, entity_type, entity_id)
            .await?;
        if documents.len() > MAX_TARGET_ENTITY_DOCUMENTS {
            return Err(Error::Validation(format!(
                "Forum Search projection entity exceeded {MAX_TARGET_ENTITY_DOCUMENTS} locale documents"
            )));
        }

        let tx = self.db.begin().await.map_err(Error::Database)?;
        let result = async {
            delete_forum_entity(&tx, tenant_id, entity_type, entity_id).await?;
            for document in documents {
                validate_document(tenant_id, &document)?;
                if document.entity_type != entity_type || document.document_id != entity_id {
                    return Err(Error::Validation(
                        "Forum Search projection entity response changed target identity"
                            .to_string(),
                    ));
                }
                insert_document(&tx, "search_documents", document).await?;
            }
            tx.commit().await.map_err(Error::Database)
        }
        .await;
        record_operation(
            "refresh_forum_entity",
            tenant_id,
            &result,
            started_at.elapsed(),
        );
        result
    }

    pub(crate) async fn delete_tenant(&self, tenant_id: Uuid) -> Result<()> {
        self.ensure_postgres()?;
        let started_at = Instant::now();
        let result = delete_forum_scope(&self.db, tenant_id).await;
        record_operation(
            "delete_forum_scope",
            tenant_id,
            &result,
            started_at.elapsed(),
        );
        result
    }

    async fn create_stage(&self, tx: &DatabaseTransaction) -> Result<()> {
        tx.execute_unprepared(
            r#"
            CREATE TEMP TABLE forum_search_projection_stage
            (LIKE search_documents INCLUDING DEFAULTS)
            ON COMMIT DROP
            "#,
        )
        .await
        .map_err(Error::Database)?;
        Ok(())
    }

    fn ensure_postgres(&self) -> Result<()> {
        if self.db.get_database_backend() != DbBackend::Postgres {
            return Err(Error::External(
                "ForumSearchProjector requires PostgreSQL backend".to_string(),
            ));
        }
        Ok(())
    }
}

fn validate_entity_type(entity_type: &str) -> Result<()> {
    if matches!(
        entity_type,
        FORUM_CATEGORY_ENTITY_TYPE | FORUM_TOPIC_ENTITY_TYPE | FORUM_REPLY_ENTITY_TYPE
    ) {
        Ok(())
    } else {
        Err(Error::Validation(format!(
            "Unsupported Forum Search projection entity type `{entity_type}`"
        )))
    }
}

fn validate_document(tenant_id: Uuid, document: &SearchProjectionDocument) -> Result<()> {
    if document.tenant_id != tenant_id
        || document.source_module != FORUM_SOURCE_MODULE
        || !document.is_public
    {
        return Err(Error::Validation(
            "Forum Search projection source returned a foreign or non-public document".to_string(),
        ));
    }
    validate_entity_type(document.entity_type.as_str())?;
    if document.document_key.is_empty()
        || document.document_key.len() > 200
        || document.locale.trim().is_empty()
        || document.locale.len() > 16
        || document.status.trim().is_empty()
        || document.status.len() > 32
    {
        return Err(Error::Validation(
            "Forum Search projection document exceeded storage bounds".to_string(),
        ));
    }
    Ok(())
}

async fn delete_forum_scope<C>(conn: &C, tenant_id: Uuid) -> Result<()>
where
    C: ConnectionTrait,
{
    let statement = Statement::from_sql_and_values(
        DbBackend::Postgres,
        "DELETE FROM search_documents WHERE tenant_id = $1 AND source_module = 'forum' AND entity_type IN ('forum_category', 'forum_topic', 'forum_reply')",
        vec![tenant_id.into()],
    );
    conn.execute(statement).await.map_err(Error::Database)?;
    Ok(())
}

async fn delete_forum_entity<C>(
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
        "DELETE FROM search_documents WHERE tenant_id = $1 AND source_module = 'forum' AND entity_type = $2 AND document_id = $3",
        vec![
            tenant_id.into(),
            entity_type.to_string().into(),
            entity_id.into(),
        ],
    );
    conn.execute(statement).await.map_err(Error::Database)?;
    Ok(())
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
    conn.execute(statement).await.map_err(Error::Database)?;
    Ok(())
}

fn record_operation(operation: &str, tenant_id: Uuid, result: &Result<()>, duration: Duration) {
    let status = if result.is_ok() { "success" } else { "error" };
    metrics::record_search_indexing_operation(
        operation,
        "forum_projection",
        status,
        duration.as_secs_f64(),
    );
    if let Err(error) = result {
        metrics::record_module_error("search", "forum_projection", "error");
        tracing::error!(
            operation,
            tenant_id = %tenant_id,
            error = %error,
            duration_ms = duration.as_millis() as u64,
            "Forum Search projection operation failed"
        );
    }
}
