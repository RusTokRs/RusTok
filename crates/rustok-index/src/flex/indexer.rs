use async_trait::async_trait;
use rustok_core::events::{EventHandler, HandlerResult};
use rustok_events::{DomainEvent, EventEnvelope};
use sea_orm::{ConnectionTrait, DatabaseBackend, DatabaseConnection, FromQueryResult, Statement};
use serde_json::{Value as JsonValue, json};
use tracing::{debug, info, instrument};
use uuid::Uuid;

use crate::error::{IndexError, IndexResult};
use crate::traits::{Indexer, IndexerContext, IndexerRuntimeConfig, run_bounded_reindex};

#[derive(Debug, FromQueryResult)]
struct FlexEntryRow {
    id: Uuid,
    tenant_id: Uuid,
    schema_id: Uuid,
    schema_slug: String,
    entity_type: Option<String>,
    entity_id: Option<Uuid>,
    status: String,
    shared_data: JsonValue,
    localized_data: JsonValue,
    created_at: chrono::DateTime<chrono::FixedOffset>,
    updated_at: chrono::DateTime<chrono::FixedOffset>,
}

#[derive(Debug, FromQueryResult)]
struct IdRow {
    id: Uuid,
}

#[derive(Clone)]
pub struct FlexIndexer {
    db: DatabaseConnection,
    runtime: IndexerRuntimeConfig,
}

impl FlexIndexer {
    pub fn new(db: DatabaseConnection) -> Self {
        Self::with_runtime(db, IndexerRuntimeConfig::load())
    }

    pub fn with_runtime(db: DatabaseConnection, runtime: IndexerRuntimeConfig) -> Self {
        Self { db, runtime }
    }

    fn backend(&self) -> DatabaseBackend {
        self.db.get_database_backend()
    }

    #[instrument(skip(self, ctx))]
    async fn build_index_entry(
        &self,
        ctx: &IndexerContext,
        entry_id: Uuid,
    ) -> IndexResult<Option<super::model::IndexFlexEntryModel>> {
        let stmt = Statement::from_sql_and_values(
            self.backend(),
            r#"
            SELECT
                e.id,
                e.tenant_id,
                e.schema_id,
                s.slug AS schema_slug,
                e.entity_type,
                e.entity_id,
                e.status,
                e.data AS shared_data,
                COALESCE(localized.localized_data, '{}'::jsonb) AS localized_data,
                e.created_at,
                e.updated_at
            FROM flex_entries e
            INNER JOIN flex_schemas s
                ON s.id = e.schema_id
               AND s.tenant_id = e.tenant_id
            LEFT JOIN LATERAL (
                SELECT jsonb_object_agg(locale, data) AS localized_data
                FROM flex_entry_localized_values
                WHERE entry_id = e.id
                  AND tenant_id = e.tenant_id
            ) localized ON TRUE
            WHERE e.id = $1
              AND e.tenant_id = $2
            "#,
            vec![entry_id.into(), ctx.tenant_id.into()],
        );

        let row = FlexEntryRow::find_by_statement(stmt)
            .one(&self.db)
            .await
            .map_err(IndexError::from)?;

        let Some(row) = row else {
            debug!(entry_id = %entry_id, "Flex entry not found");
            return Ok(None);
        };

        let data_preview = json!({
            "shared": row.shared_data,
            "localized": row.localized_data,
        });

        Ok(Some(super::model::IndexFlexEntryModel {
            id: row.id,
            tenant_id: row.tenant_id,
            schema_id: row.schema_id,
            schema_slug: row.schema_slug,
            entity_type: row.entity_type,
            entity_id: row.entity_id,
            status: row.status,
            data_preview,
            created_at: row.created_at.with_timezone(&chrono::Utc),
            updated_at: row.updated_at.with_timezone(&chrono::Utc),
        }))
    }

    fn build_search_text(model: &super::model::IndexFlexEntryModel) -> String {
        let mut fragments = vec![model.schema_slug.clone(), model.status.clone()];

        if let Some(entity_type) = &model.entity_type {
            fragments.push(entity_type.clone());
        }

        if let Some(entity_id) = model.entity_id {
            fragments.push(entity_id.to_string());
        }

        append_json_fragments(&model.data_preview, &mut fragments);

        fragments.retain(|value| !value.trim().is_empty());
        fragments.join(" ")
    }

    async fn upsert_index_entry(
        &self,
        model: &super::model::IndexFlexEntryModel,
    ) -> IndexResult<()> {
        let search_text = Self::build_search_text(model);
        let stmt = Statement::from_sql_and_values(
            self.backend(),
            r#"
            INSERT INTO index_flex_entries (
                id,
                tenant_id,
                schema_id,
                schema_slug,
                entity_type,
                entity_id,
                status,
                data_preview,
                search_vector,
                created_at,
                updated_at,
                indexed_at
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, to_tsvector('simple', $9), $10, $11, NOW()
            )
            ON CONFLICT (id) DO UPDATE SET
                tenant_id = EXCLUDED.tenant_id,
                schema_id = EXCLUDED.schema_id,
                schema_slug = EXCLUDED.schema_slug,
                entity_type = EXCLUDED.entity_type,
                entity_id = EXCLUDED.entity_id,
                status = EXCLUDED.status,
                data_preview = EXCLUDED.data_preview,
                search_vector = EXCLUDED.search_vector,
                created_at = EXCLUDED.created_at,
                updated_at = EXCLUDED.updated_at,
                indexed_at = NOW()
            "#,
            vec![
                model.id.into(),
                model.tenant_id.into(),
                model.schema_id.into(),
                model.schema_slug.clone().into(),
                model.entity_type.clone().into(),
                model.entity_id.into(),
                model.status.clone().into(),
                model.data_preview.clone().into(),
                search_text.into(),
                model.created_at.into(),
                model.updated_at.into(),
            ],
        );

        self.db
            .execute(stmt)
            .await
            .map(|_| ())
            .map_err(IndexError::from)
    }

    async fn delete_entry_from_index(&self, tenant_id: Uuid, entry_id: Uuid) -> IndexResult<()> {
        let stmt = Statement::from_sql_and_values(
            self.backend(),
            "DELETE FROM index_flex_entries WHERE tenant_id = $1 AND id = $2",
            vec![tenant_id.into(), entry_id.into()],
        );
        self.db
            .execute(stmt)
            .await
            .map(|_| ())
            .map_err(IndexError::from)
    }

    async fn reindex_schema_entries(
        &self,
        ctx: &IndexerContext,
        schema_id: Uuid,
    ) -> IndexResult<u64> {
        let stmt = Statement::from_sql_and_values(
            self.backend(),
            "SELECT id FROM flex_entries WHERE tenant_id = $1 AND schema_id = $2",
            vec![ctx.tenant_id.into(), schema_id.into()],
        );

        let rows = IdRow::find_by_statement(stmt)
            .all(&self.db)
            .await
            .map_err(IndexError::from)?;

        let ids = rows.into_iter().map(|row| row.id).collect();
        run_bounded_reindex(self.clone(), ctx, ids, "reindex_schema_entries")
            .await
            .strict_completed(Indexer::name(self), "reindex_schema_entries")
    }

    async fn delete_schema_entries_from_index(
        &self,
        tenant_id: Uuid,
        schema_id: Uuid,
    ) -> IndexResult<()> {
        let stmt = Statement::from_sql_and_values(
            self.backend(),
            "DELETE FROM index_flex_entries WHERE tenant_id = $1 AND schema_id = $2",
            vec![tenant_id.into(), schema_id.into()],
        );
        self.db
            .execute(stmt)
            .await
            .map(|_| ())
            .map_err(IndexError::from)
    }
}

fn append_json_fragments(value: &JsonValue, fragments: &mut Vec<String>) {
    match value {
        JsonValue::Null => {}
        JsonValue::Bool(boolean) => fragments.push(boolean.to_string()),
        JsonValue::Number(number) => fragments.push(number.to_string()),
        JsonValue::String(text) => fragments.push(text.clone()),
        JsonValue::Array(items) => {
            for item in items {
                append_json_fragments(item, fragments);
            }
        }
        JsonValue::Object(map) => {
            for (key, value) in map {
                fragments.push(key.clone());
                append_json_fragments(value, fragments);
            }
        }
    }
}

#[async_trait]
impl Indexer for FlexIndexer {
    fn name(&self) -> &'static str {
        "flex_indexer"
    }

    #[instrument(skip(self, ctx))]
    async fn index_one(&self, ctx: &IndexerContext, entity_id: Uuid) -> IndexResult<()> {
        match self.build_index_entry(ctx, entity_id).await? {
            Some(model) => {
                self.upsert_index_entry(&model).await?;
                debug!(entry_id = %entity_id, "Indexed flex entry");
            }
            None => {
                self.delete_entry_from_index(ctx.tenant_id, entity_id)
                    .await?;
                debug!(entry_id = %entity_id, tenant_id = %ctx.tenant_id, "Removed stale flex index entry");
            }
        }

        Ok(())
    }

    #[instrument(skip(self, ctx))]
    async fn remove_one(&self, ctx: &IndexerContext, entity_id: Uuid) -> IndexResult<()> {
        debug!(entry_id = %entity_id, "Removing flex entry from index");
        self.delete_entry_from_index(ctx.tenant_id, entity_id).await
    }

    #[instrument(skip(self, ctx))]
    async fn reindex_all(&self, ctx: &IndexerContext) -> IndexResult<u64> {
        info!(tenant_id = %ctx.tenant_id, "Reindexing all flex entries");

        let stmt = Statement::from_sql_and_values(
            self.backend(),
            "SELECT id FROM flex_entries WHERE tenant_id = $1",
            vec![ctx.tenant_id.into()],
        );

        let rows = IdRow::find_by_statement(stmt)
            .all(&self.db)
            .await
            .map_err(IndexError::from)?;

        let ids = rows.into_iter().map(|row| row.id).collect();
        run_bounded_reindex(self.clone(), ctx, ids, "reindex_all")
            .await
            .strict_completed(Indexer::name(self), "reindex_all")
    }
}

#[async_trait]
impl EventHandler for FlexIndexer {
    fn name(&self) -> &'static str {
        "flex_indexer"
    }

    fn handles(&self, event: &DomainEvent) -> bool {
        matches!(
            event,
            DomainEvent::FlexEntryCreated { .. }
                | DomainEvent::FlexEntryUpdated { .. }
                | DomainEvent::FlexEntryDeleted { .. }
                | DomainEvent::FlexSchemaUpdated { .. }
                | DomainEvent::FlexSchemaDeleted { .. }
        ) || matches!(
            event,
            DomainEvent::ReindexRequested { target_type, .. } if target_type == "flex"
        )
    }

    async fn handle(&self, envelope: &EventEnvelope) -> HandlerResult {
        let ctx = IndexerContext::new_with_runtime(
            self.db.clone(),
            envelope.tenant_id,
            self.runtime.clone(),
        );

        match &envelope.event {
            DomainEvent::FlexEntryCreated { entry_id, .. }
            | DomainEvent::FlexEntryUpdated { entry_id, .. } => {
                self.index_one(&ctx, *entry_id).await?;
            }
            DomainEvent::FlexEntryDeleted { entry_id, .. } => {
                self.remove_one(&ctx, *entry_id).await?;
            }
            DomainEvent::FlexSchemaUpdated { schema_id, .. } => {
                self.reindex_schema_entries(&ctx, *schema_id).await?;
            }
            DomainEvent::FlexSchemaDeleted { schema_id, .. } => {
                self.delete_schema_entries_from_index(envelope.tenant_id, *schema_id)
                    .await?;
            }
            DomainEvent::ReindexRequested { target_id, .. } => {
                if let Some(entry_id) = target_id {
                    self.index_one(&ctx, *entry_id).await?;
                } else {
                    self.reindex_all(&ctx).await?;
                }
            }
            _ => {}
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use rustok_core::events::EventHandler;
    use rustok_events::DomainEvent;
    use sea_orm::Database;
    use serde_json::json;
    use uuid::Uuid;

    use super::{FlexIndexer, append_json_fragments};

    #[test]
    fn append_json_fragments_flattens_nested_values() {
        let mut fragments = Vec::new();
        append_json_fragments(
            &json!({
                "shared": {
                    "title": "Landing",
                    "views": 42,
                },
                "localized": {
                    "ru": {
                        "title": "Лендинг"
                    }
                }
            }),
            &mut fragments,
        );

        assert!(fragments.contains(&"shared".to_string()));
        assert!(fragments.contains(&"title".to_string()));
        assert!(fragments.contains(&"Landing".to_string()));
        assert!(fragments.contains(&"42".to_string()));
        assert!(fragments.contains(&"localized".to_string()));
        assert!(fragments.contains(&"ru".to_string()));
        assert!(fragments.contains(&"Лендинг".to_string()));
    }

    #[tokio::test]
    async fn flex_indexer_handles_flex_events_and_reindex_requests() {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("sqlite in-memory db");
        let indexer = FlexIndexer::new(db);

        assert!(indexer.handles(&DomainEvent::FlexEntryCreated {
            tenant_id: Uuid::new_v4(),
            schema_id: Uuid::new_v4(),
            entry_id: Uuid::new_v4(),
            entity_type: None,
            entity_id: None,
        }));
        assert!(indexer.handles(&DomainEvent::FlexEntryUpdated {
            tenant_id: Uuid::new_v4(),
            schema_id: Uuid::new_v4(),
            entry_id: Uuid::new_v4(),
        }));
        assert!(indexer.handles(&DomainEvent::FlexEntryDeleted {
            tenant_id: Uuid::new_v4(),
            schema_id: Uuid::new_v4(),
            entry_id: Uuid::new_v4(),
        }));
        assert!(indexer.handles(&DomainEvent::FlexSchemaUpdated {
            tenant_id: Uuid::new_v4(),
            schema_id: Uuid::new_v4(),
            slug: "landing".to_string(),
        }));
        assert!(indexer.handles(&DomainEvent::FlexSchemaDeleted {
            tenant_id: Uuid::new_v4(),
            schema_id: Uuid::new_v4(),
        }));
        assert!(indexer.handles(&DomainEvent::ReindexRequested {
            target_type: "flex".to_string(),
            target_id: None,
        }));
    }
}
