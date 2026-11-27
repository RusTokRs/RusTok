use std::{
    collections::{BTreeMap, HashSet},
    sync::Arc,
};

use async_trait::async_trait;
use rustok_api::{
    PortError,
    localized_index::{
        LocalizedIndexKey, LocalizedIndexReader, LocalizedIndexRow, LocalizedIndexSource,
        MAX_LOCALIZED_INDEX_LOAD,
    },
};
use rustok_core::{
    ModuleEventListenerContext, ModuleEventListenerRegistry, ModuleRuntimeExtensions,
};
use rustok_index::{
    EntityKey, FieldName, IndexMutation, IndexRecord, IndexSource, IndexSourceCursor,
    IndexSourceFailure, IndexSourceLoadBatch, IndexSourceLoadRequest, IndexSourcePage,
    IndexSourceRegistrationError, IndexSourceScanRequest, IndexValue, LocaleKey,
    PostgresIndexSourceFactory, SchemaRef, derive_index_schema_source_event_id,
    register_index_source,
};
use sea_orm::DatabaseConnection;
use uuid::Uuid;

pub(super) struct Factory {
    pub owner: LocalizedIndexSource,
    pub reference: SchemaRef,
    pub name: String,
}

impl Factory {
    pub(super) fn source(&self, db: DatabaseConnection) -> Source {
        Source {
            reader: (self.owner.build)(db),
            schema: self.reference.clone(),
        }
    }
}

impl PostgresIndexSourceFactory for Factory {
    fn register_source(
        &self,
        extensions: &mut ModuleRuntimeExtensions,
        db: DatabaseConnection,
    ) -> Result<(), IndexSourceRegistrationError> {
        register_index_source(
            extensions,
            self.owner.module_slug,
            self.name.clone(),
            [self.reference.clone()],
            self.source(db),
        )?;
        Ok(())
    }

    fn register_event_listeners(
        &self,
        registry: &mut ModuleEventListenerRegistry,
        context: &ModuleEventListenerContext<'_>,
    ) {
        registry.register(super::consumer::Consumer::new(self, context));
    }
}

pub(super) struct Source {
    reader: Arc<dyn LocalizedIndexReader>,
    schema: SchemaRef,
}

#[async_trait]
impl IndexSource for Source {
    async fn scan(
        &self,
        request: IndexSourceScanRequest,
    ) -> Result<IndexSourcePage, IndexSourceFailure> {
        self.require_schema(request.schema())?;
        let after = request
            .cursor()
            .map(|cursor| serde_json::from_value(cursor.value().clone()))
            .transpose()
            .map_err(|_| permanent("localized_index.cursor_invalid"))?;
        let page = self
            .reader
            .scan(
                request.tenant_id(),
                request.locale().map(|locale| locale.as_str().to_owned()),
                after,
                request.limit(),
            )
            .await
            .map_err(owner_failure)?;
        if let Some(next) = &page.next
            && (page.rows.len() != request.limit()
                || page.rows.last().map(|row| &row.key) != Some(next))
        {
            return Err(permanent("localized_index.page_invalid"));
        }
        let cursor = page
            .next
            .map(|next| {
                let json = serde_json::to_value(next)
                    .map_err(|_| permanent("localized_index.cursor_invalid"))?;
                IndexSourceCursor::new(json)
                    .map_err(|_| permanent("localized_index.cursor_invalid"))
            })
            .transpose()?;
        let mutations = self.mutations(request.tenant_id(), page.rows)?;
        IndexSourcePage::new(&request, mutations, cursor)
            .map_err(|_| permanent("localized_index.page_invalid"))
    }

    async fn load(
        &self,
        request: IndexSourceLoadRequest,
    ) -> Result<IndexSourceLoadBatch, IndexSourceFailure> {
        self.require_schema(request.schema())?;
        let keys = request
            .keys()
            .iter()
            .map(|key| {
                let locale = key
                    .locale
                    .as_ref()
                    .ok_or_else(|| permanent("localized_index.locale_required"))?;
                Ok(LocalizedIndexKey {
                    id: key.entity_id,
                    locale: locale.as_str().to_owned(),
                })
            })
            .collect::<Result<Vec<_>, IndexSourceFailure>>()?;
        let rows = self
            .reader
            .load(request.tenant_id(), keys)
            .await
            .map_err(owner_failure)?;
        IndexSourceLoadBatch::new(&request, self.mutations(request.tenant_id(), rows)?)
            .map_err(|_| permanent("localized_index.batch_invalid"))
    }
}

impl Source {
    pub(super) async fn refresh(
        &self,
        tenant_id: Uuid,
        id: Uuid,
        minimum_revision: u64,
    ) -> Result<Vec<IndexMutation>, IndexSourceFailure> {
        let rows = self
            .reader
            .load_record(tenant_id, id)
            .await
            .map_err(owner_failure)?;
        let first = rows
            .first()
            .ok_or_else(|| permanent("localized_index.owner_record_missing"))?;
        if first.revision.get() < minimum_revision {
            return Err(
                IndexSourceFailure::retryable("localized_index.owner_revision_not_ready")
                    .expect("static localized Index failure code"),
            );
        }
        let mut locales = HashSet::with_capacity(rows.len().min(MAX_LOCALIZED_INDEX_LOAD));
        if rows.len() > MAX_LOCALIZED_INDEX_LOAD
            || rows.iter().any(|row| {
                row.key.id != id
                    || row.revision != first.revision
                    || row.published_title.is_some() != first.published_title.is_some()
                    || !locales.insert(&row.key.locale)
            })
        {
            return Err(permanent("localized_index.refresh_invalid"));
        }
        self.mutations(tenant_id, rows)
    }

    fn require_schema(&self, schema: &SchemaRef) -> Result<(), IndexSourceFailure> {
        if &self.schema != schema {
            return Err(permanent("localized_index.schema_mismatch"));
        }
        Ok(())
    }

    fn mutations(
        &self,
        tenant_id: Uuid,
        rows: Vec<LocalizedIndexRow>,
    ) -> Result<Vec<IndexMutation>, IndexSourceFailure> {
        rows.into_iter()
            .map(|row| {
                let invalid_row = || permanent("localized_index.record_invalid");
                let locale = LocaleKey::new(&row.key.locale).map_err(|_| invalid_row())?;
                if row.tenant_id != tenant_id
                    || row.key.id.is_nil()
                    || locale.as_str() != row.key.locale
                {
                    return Err(invalid_row());
                }
                let source_version = row.revision.get();
                let event_id = derive_index_schema_source_event_id(
                    "native.localized-record-replay-v1",
                    tenant_id,
                    &self.schema,
                    row.key.id,
                    Some(&locale),
                    source_version,
                )
                .map_err(|_| invalid_row())?;
                let key = EntityKey {
                    tenant_id,
                    schema: self.schema.clone(),
                    entity_id: row.key.id,
                    locale: Some(locale),
                };
                let Some(title) = row.published_title else {
                    return Ok(IndexMutation::Delete {
                        event_id,
                        key,
                        source_version,
                    });
                };
                let fields = BTreeMap::from([
                    (
                        FieldName::new("id").map_err(|_| invalid_row())?,
                        IndexValue::Uuid(row.key.id),
                    ),
                    (
                        FieldName::new("title").map_err(|_| invalid_row())?,
                        IndexValue::String(title),
                    ),
                ]);
                Ok(IndexMutation::Upsert {
                    event_id,
                    record: IndexRecord {
                        key,
                        source_version,
                        fields,
                        links: Vec::new(),
                    },
                })
            })
            .collect()
    }
}

fn permanent(code: &'static str) -> IndexSourceFailure {
    IndexSourceFailure::permanent(code).expect("static localized Index failure code")
}

fn owner_failure(error: PortError) -> IndexSourceFailure {
    if error.retryable {
        IndexSourceFailure::retryable("localized_index.owner_unavailable")
            .expect("static localized Index failure code")
    } else {
        permanent("localized_index.owner_read_refused")
    }
}
