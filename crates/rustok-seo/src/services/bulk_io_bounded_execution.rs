use rustok_content::resolve_by_locale_with_fallback;
use sea_orm::QuerySelect;

use crate::dto::SeoModuleSettings;
use crate::entities::{self as seo_meta, meta_translation};

use super::robots::first_open_graph_image_url;
use super::templates::render_generated_record;
use super::{LoadedMeta, TargetState, trimmed_option};

const BULK_IO_CHUNK_SIZE: usize = 50;
const BULK_IO_META_BATCH_SIZE: usize = 256;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct QueuedBulkExportPayload {
    input: SeoBulkExportInput,
    target_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct QueuedBulkImportPayload {
    input: SeoBulkImportInput,
    next_byte_offset: usize,
    next_row_number: usize,
    total_rows: usize,
}

#[derive(Debug)]
struct BulkImportChunk {
    rows: Vec<BulkImportRow>,
    next_byte_offset: usize,
    next_row_number: usize,
    exhausted: bool,
}

impl SeoService {
    pub(super) async fn queue_bulk_export_bounded_io(
        &self,
        tenant: &TenantContext,
        created_by: Option<Uuid>,
        input: SeoBulkExportInput,
    ) -> SeoResult<SeoBulkJobRecord> {
        let filter =
            normalize_bulk_list_input(input.filter.clone(), tenant.default_locale.as_str())?;
        let rows = self.collect_bulk_rows_for_filter(tenant, &filter).await?;
        let target_ids = rows.into_iter().map(|row| row.target_id).collect::<Vec<_>>();
        let payload = QueuedBulkExportPayload {
            input: input.clone(),
            target_ids: target_ids.clone(),
        };
        let now = Utc::now().fixed_offset();
        let model = seo_bulk_job::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant.id),
            operation_kind: Set(SeoBulkJobOperationKind::ExportCsv.as_str().to_string()),
            status: Set(SeoBulkJobStatus::Queued.as_str().to_string()),
            target_kind: Set(filter.target_kind.as_str().to_string()),
            locale: Set(filter.locale.clone()),
            filter_payload: Set(serde_json::to_value(&filter).map_err(|error| {
                SeoError::validation(format!("failed to serialize bulk filter: {error}"))
            })?),
            input_payload: Set(serde_json::to_value(&payload).map_err(|error| {
                SeoError::validation(format!(
                    "failed to serialize bounded bulk export payload: {error}"
                ))
            })?),
            publish_after_write: Set(false),
            matched_count: Set(target_ids.len() as i32),
            processed_count: Set(0),
            succeeded_count: Set(0),
            failed_count: Set(0),
            artifact_count: Set(0),
            last_error: Set(None),
            created_by: Set(created_by),
            started_at: Set(None),
            completed_at: Set(None),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(&self.db)
        .await?;

        self.bulk_job(tenant.id, model.id)
            .await?
            .ok_or(SeoError::NotFound)
    }

    pub(super) async fn queue_bulk_import_bounded_io(
        &self,
        tenant: &TenantContext,
        created_by: Option<Uuid>,
        input: SeoBulkImportInput,
    ) -> SeoResult<SeoBulkJobRecord> {
        let locale = super::normalize_effective_locale(
            input.locale.as_str(),
            tenant.default_locale.as_str(),
        )?;
        let (next_byte_offset, total_rows) = scan_bulk_import_csv(
            input.target_kind.clone(),
            locale.as_str(),
            input.csv_utf8.as_str(),
        )?;
        let payload = QueuedBulkImportPayload {
            input: input.clone(),
            next_byte_offset,
            next_row_number: 2,
            total_rows,
        };
        let now = Utc::now().fixed_offset();
        let model = seo_bulk_job::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant.id),
            operation_kind: Set(SeoBulkJobOperationKind::ImportCsv.as_str().to_string()),
            status: Set(SeoBulkJobStatus::Queued.as_str().to_string()),
            target_kind: Set(input.target_kind.as_str().to_string()),
            locale: Set(locale.clone()),
            filter_payload: Set(json!({
                "target_kind": input.target_kind.as_str(),
                "locale": locale,
            })),
            input_payload: Set(serde_json::to_value(&payload).map_err(|error| {
                SeoError::validation(format!(
                    "failed to serialize bounded bulk import payload: {error}"
                ))
            })?),
            publish_after_write: Set(input.publish_after_write),
            matched_count: Set(total_rows as i32),
            processed_count: Set(0),
            succeeded_count: Set(0),
            failed_count: Set(0),
            artifact_count: Set(0),
            last_error: Set(None),
            created_by: Set(created_by),
            started_at: Set(None),
            completed_at: Set(None),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(&self.db)
        .await?;

        self.bulk_job(tenant.id, model.id)
            .await?
            .ok_or(SeoError::NotFound)
    }

    pub(super) async fn execute_next_bulk_job_fully_bounded(
        &self,
    ) -> SeoResult<Option<SeoBulkJobRecord>> {
        let running = seo_bulk_job::Entity::find()
            .filter(seo_bulk_job::Column::Status.eq(SeoBulkJobStatus::Running.as_str()))
            .filter(seo_bulk_job::Column::OperationKind.is_in([
                SeoBulkJobOperationKind::Apply.as_str(),
                SeoBulkJobOperationKind::ExportCsv.as_str(),
                SeoBulkJobOperationKind::ImportCsv.as_str(),
            ]))
            .order_by_asc(seo_bulk_job::Column::UpdatedAt)
            .one(&self.db)
            .await?;

        let running = if let Some(job) = running {
            job
        } else {
            let Some(job) = seo_bulk_job::Entity::find()
                .filter(seo_bulk_job::Column::Status.eq(SeoBulkJobStatus::Queued.as_str()))
                .order_by_asc(seo_bulk_job::Column::CreatedAt)
                .one(&self.db)
                .await?
            else {
                return Ok(None);
            };

            let now = Utc::now().fixed_offset();
            let mut active: seo_bulk_job::ActiveModel = job.into();
            active.status = Set(SeoBulkJobStatus::Running.as_str().to_string());
            active.started_at = Set(Some(now));
            active.updated_at = Set(now);
            active.last_error = Set(None);
            active.update(&self.db).await?
        };

        let result = match SeoBulkJobOperationKind::parse(running.operation_kind.as_str()) {
            Some(SeoBulkJobOperationKind::Apply) => self.execute_apply_job_chunk(&running).await,
            Some(SeoBulkJobOperationKind::ExportCsv) => {
                self.execute_export_job_chunk(&running).await
            }
            Some(SeoBulkJobOperationKind::ImportCsv) => {
                self.execute_import_job_chunk(&running).await
            }
            None => Err(SeoError::validation(format!(
                "unknown bulk operation kind `{}`",
                running.operation_kind
            ))),
        };

        if let Err(error) = result {
            self.fail_bulk_job(&running, error.to_string()).await?;
        }

        self.bulk_job(running.tenant_id, running.id).await
    }

    async fn execute_export_job_chunk(&self, job: &seo_bulk_job::Model) -> SeoResult<()> {
        let tenant = self.load_tenant_context(job.tenant_id).await?;
        let payload = self.decode_bounded_export_payload(&tenant, job).await?;
        let processed_items = seo_bulk_job_item::Entity::find()
            .filter(seo_bulk_job_item::Column::JobId.eq(job.id))
            .all(&self.db)
            .await?;
        let processed_ids = processed_items
            .iter()
            .map(|item| item.target_id)
            .collect::<HashSet<_>>();
        let chunk = payload
            .target_ids
            .iter()
            .copied()
            .filter(|target_id| !processed_ids.contains(target_id))
            .take(BULK_IO_CHUNK_SIZE)
            .collect::<Vec<_>>();

        if chunk.is_empty() {
            let progress = self.load_bulk_job_progress(job.id).await?;
            return self
                .finish_bulk_job(
                    job,
                    progress.processed,
                    progress.succeeded,
                    progress.failed,
                    progress.artifacts,
                    None,
                )
                .await;
        }

        let filter = normalize_bulk_list_input(
            payload.input.filter.clone(),
            tenant.default_locale.as_str(),
        )?;
        let projections = self
            .load_bulk_export_projection_chunk(&tenant, &filter, chunk.as_slice())
            .await?;
        let chunk_start = processed_ids.len() + 1;
        let chunk_end = chunk_start + chunk.len() - 1;
        let mut writer = WriterBuilder::new()
            .has_headers(false)
            .from_writer(Vec::<u8>::new());
        writer.write_record(CSV_HEADERS).map_err(|error| {
            SeoError::validation(format!("failed to write export CSV header: {error}"))
        })?;
        let mut failure_rows = Vec::new();

        for target_id in chunk {
            if let Some(projection) = projections.get(&target_id) {
                writer
                    .write_record(export_bulk_projection_row(
                        filter.target_kind.clone(),
                        target_id,
                        filter.locale.as_str(),
                        projection,
                    ))
                    .map_err(|error| {
                        SeoError::validation(format!(
                            "failed to serialize export row for {target_id}: {error}"
                        ))
                    })?;
                self.insert_bulk_job_item(job, target_id, None, None)
                    .await?;
            } else {
                let message = "SEO target not found".to_string();
                self.insert_bulk_job_item(job, target_id, Some(message.clone()), None)
                    .await?;
                failure_rows.push((
                    empty_csv_row(
                        filter.target_kind.as_str(),
                        target_id,
                        filter.locale.as_str(),
                    ),
                    message,
                ));
            }
        }

        let bytes = writer.into_inner().map_err(|error| {
            SeoError::validation(format!("failed to finalize export CSV writer: {error}"))
        })?;
        let content = String::from_utf8(bytes).map_err(|error| {
            SeoError::validation(format!("export CSV is not valid UTF-8: {error}"))
        })?;
        self.insert_bulk_job_artifact(
            job,
            "export_csv",
            format!(
                "seo-bulk-export-{}-{}-{}-{}-{}.csv",
                filter.target_kind.as_str(),
                filter.locale,
                job.id,
                chunk_start,
                chunk_end,
            ),
            CSV_MIME_TYPE,
            content,
        )
        .await?;

        if !failure_rows.is_empty() {
            let content = build_failure_csv(&failure_rows)?;
            self.insert_bulk_job_artifact(
                job,
                "failure_report",
                format!(
                    "seo-bulk-export-failures-{}-{}-{}.csv",
                    job.id, chunk_start, chunk_end
                ),
                CSV_MIME_TYPE,
                content,
            )
            .await?;
        }

        let progress = self.load_bulk_job_progress(job.id).await?;
        if progress.processed as usize >= payload.target_ids.len() {
            self.finish_bulk_job(
                job,
                progress.processed,
                progress.succeeded,
                progress.failed,
                progress.artifacts,
                None,
            )
            .await
        } else {
            self.checkpoint_bulk_io_job(
                job,
                payload.target_ids.len(),
                progress,
                serde_json::to_value(&payload).map_err(|error| {
                    SeoError::validation(format!(
                        "failed to checkpoint bounded bulk export payload: {error}"
                    ))
                })?,
            )
            .await
        }
    }

    async fn execute_import_job_chunk(&self, job: &seo_bulk_job::Model) -> SeoResult<()> {
        let tenant = self.load_tenant_context(job.tenant_id).await?;
        let mut payload = self.decode_bounded_import_payload(job).await?;
        let chunk = read_bulk_import_chunk(&payload)?;

        if chunk.rows.is_empty() {
            let progress = self.load_bulk_job_progress(job.id).await?;
            return self
                .finish_bulk_job(
                    job,
                    progress.processed,
                    progress.succeeded,
                    progress.failed,
                    progress.artifacts,
                    None,
                )
                .await;
        }

        let locale = super::normalize_effective_locale(
            payload.input.locale.as_str(),
            tenant.default_locale.as_str(),
        )?;
        let chunk_start = payload.next_row_number;
        let chunk_end = chunk.next_row_number.saturating_sub(1);
        let mut failure_rows = Vec::new();

        for row in chunk.rows {
            match self
                .import_bulk_row(
                    &tenant,
                    job.id,
                    payload.input.target_kind.clone(),
                    locale.as_str(),
                    &row,
                    job.publish_after_write,
                )
                .await
            {
                Ok(revision) => {
                    self.insert_bulk_job_item(job, row.target_id, None, revision)
                        .await?;
                }
                Err(error) => {
                    let message = error.to_string();
                    self.insert_bulk_job_item(job, row.target_id, Some(message.clone()), None)
                        .await?;
                    failure_rows.push((
                        export_csv_row_values(
                            payload.input.target_kind.clone(),
                            locale.as_str(),
                            &row,
                        ),
                        format!("row {}: {}", row.row_number, message),
                    ));
                }
            }
        }

        if !failure_rows.is_empty() {
            let content = build_failure_csv(&failure_rows)?;
            self.insert_bulk_job_artifact(
                job,
                "failure_report",
                format!(
                    "seo-bulk-import-failures-{}-{}-{}.csv",
                    job.id, chunk_start, chunk_end
                ),
                CSV_MIME_TYPE,
                content,
            )
            .await?;
        }

        payload.next_byte_offset = chunk.next_byte_offset;
        payload.next_row_number = chunk.next_row_number;
        let progress = self.load_bulk_job_progress(job.id).await?;
        if chunk.exhausted || progress.processed as usize >= payload.total_rows {
            self.finish_bulk_job(
                job,
                progress.processed,
                progress.succeeded,
                progress.failed,
                progress.artifacts,
                None,
            )
            .await
        } else {
            self.checkpoint_bulk_io_job(
                job,
                payload.total_rows,
                progress,
                serde_json::to_value(&payload).map_err(|error| {
                    SeoError::validation(format!(
                        "failed to checkpoint bounded bulk import payload: {error}"
                    ))
                })?,
            )
            .await
        }
    }

    async fn decode_bounded_export_payload(
        &self,
        tenant: &TenantContext,
        job: &seo_bulk_job::Model,
    ) -> SeoResult<QueuedBulkExportPayload> {
        if let Ok(payload) =
            serde_json::from_value::<QueuedBulkExportPayload>(job.input_payload.clone())
        {
            return Ok(payload);
        }

        let input = serde_json::from_value::<SeoBulkExportInput>(job.input_payload.clone())
            .map_err(|error| {
                SeoError::validation(format!("failed to decode bulk export payload: {error}"))
            })?;
        let filter =
            normalize_bulk_list_input(input.filter.clone(), tenant.default_locale.as_str())?;
        let rows = self.collect_bulk_rows_for_filter(tenant, &filter).await?;
        Ok(QueuedBulkExportPayload {
            input,
            target_ids: rows.into_iter().map(|row| row.target_id).collect(),
        })
    }

    async fn decode_bounded_import_payload(
        &self,
        job: &seo_bulk_job::Model,
    ) -> SeoResult<QueuedBulkImportPayload> {
        if let Ok(payload) =
            serde_json::from_value::<QueuedBulkImportPayload>(job.input_payload.clone())
        {
            return Ok(payload);
        }

        let input = serde_json::from_value::<SeoBulkImportInput>(job.input_payload.clone())
            .map_err(|error| {
                SeoError::validation(format!("failed to decode bulk import payload: {error}"))
            })?;
        let (next_byte_offset, total_rows) = scan_bulk_import_csv(
            input.target_kind.clone(),
            job.locale.as_str(),
            input.csv_utf8.as_str(),
        )?;
        let progress = self.load_bulk_job_progress(job.id).await?;
        advance_legacy_import_cursor(
            QueuedBulkImportPayload {
                input,
                next_byte_offset,
                next_row_number: 2,
                total_rows,
            },
            progress.processed.max(0) as usize,
        )
    }

    async fn checkpoint_bulk_io_job(
        &self,
        job: &seo_bulk_job::Model,
        matched_count: usize,
        progress: BulkJobProgress,
        input_payload: Value,
    ) -> SeoResult<()> {
        let now = Utc::now().fixed_offset();
        let mut active: seo_bulk_job::ActiveModel = job.clone().into();
        active.status = Set(SeoBulkJobStatus::Running.as_str().to_string());
        active.input_payload = Set(input_payload);
        active.matched_count = Set(matched_count as i32);
        active.processed_count = Set(progress.processed);
        active.succeeded_count = Set(progress.succeeded);
        active.failed_count = Set(progress.failed);
        active.artifact_count = Set(progress.artifacts);
        active.last_error = Set(None);
        active.completed_at = Set(None);
        active.updated_at = Set(now);
        active.update(&self.db).await?;
        Ok(())
    }

    async fn load_bulk_export_projection_chunk(
        &self,
        tenant: &TenantContext,
        filter: &NormalizedBulkListFilter,
        target_ids: &[Uuid],
    ) -> SeoResult<HashMap<Uuid, super::bulk_read_model::BulkReadProjection>> {
        if target_ids.is_empty() || !self.is_enabled(tenant.id).await? {
            return Ok(HashMap::new());
        }
        let Some(provider) = self.registry.get(&filter.target_kind) else {
            return Ok(HashMap::new());
        };
        let selected = target_ids.iter().copied().collect::<HashSet<_>>();
        let summaries = provider
            .list_bulk_summaries(
                &self.target_runtime(),
                SeoTargetBulkListRequest {
                    tenant_id: tenant.id,
                    default_locale: tenant.default_locale.as_str(),
                    locale: filter.locale.as_str(),
                },
            )
            .await
            .map_err(|error| {
                SeoError::validation(format!(
                    "SEO target provider `{}` failed to collect bounded export summaries: {error}",
                    filter.target_kind.as_str()
                ))
            })?;
        let available = summaries
            .into_iter()
            .filter(|summary| selected.contains(&summary.target_id))
            .map(|summary| summary.target_id)
            .collect::<HashSet<_>>();
        let mut explicit_by_target = self
            .load_bulk_io_explicit_meta_batches(
                tenant.id,
                filter.target_kind.as_str(),
                target_ids,
            )
            .await?;
        let settings = self.load_settings(tenant.id).await?;
        let mut projections = HashMap::new();

        for target_id in target_ids.iter().copied() {
            if !available.contains(&target_id) {
                continue;
            }
            let state = self
                .load_target_state(
                    tenant,
                    filter.target_kind.clone(),
                    target_id,
                    filter.locale.as_str(),
                )
                .await?;
            let explicit = explicit_by_target.remove(&target_id);
            if let Some(projection) = resolve_bulk_io_projection(
                tenant,
                filter.target_kind.clone(),
                target_id,
                filter.locale.as_str(),
                explicit,
                state,
                &settings,
            ) {
                projections.insert(target_id, projection);
            }
        }

        Ok(projections)
    }

    async fn load_bulk_io_explicit_meta_batches(
        &self,
        tenant_id: Uuid,
        target_kind: &str,
        target_ids: &[Uuid],
    ) -> SeoResult<HashMap<Uuid, LoadedMeta>> {
        let mut loaded = HashMap::new();
        for target_ids in target_ids.chunks(BULK_IO_META_BATCH_SIZE) {
            let metas = seo_meta::Entity::find()
                .filter(seo_meta::Column::TenantId.eq(tenant_id))
                .filter(seo_meta::Column::TargetType.eq(target_kind))
                .filter(seo_meta::Column::TargetId.is_in(target_ids.iter().copied()))
                .all(&self.db)
                .await?;
            if metas.is_empty() {
                continue;
            }

            let meta_ids = metas.iter().map(|meta| meta.id).collect::<Vec<_>>();
            let translations = meta_translation::Entity::find()
                .filter(meta_translation::Column::MetaId.is_in(meta_ids))
                .order_by_asc(meta_translation::Column::MetaId)
                .order_by_asc(meta_translation::Column::Locale)
                .all(&self.db)
                .await?;
            let mut translations_by_meta =
                HashMap::<Uuid, Vec<meta_translation::Model>>::new();
            for translation in translations {
                translations_by_meta
                    .entry(translation.meta_id)
                    .or_default()
                    .push(translation);
            }

            for meta in metas {
                loaded.insert(
                    meta.target_id,
                    LoadedMeta {
                        translations: translations_by_meta.remove(&meta.id).unwrap_or_default(),
                        meta,
                    },
                );
            }
        }
        Ok(loaded)
    }
}

fn scan_bulk_import_csv(
    expected_kind: SeoTargetSlug,
    expected_locale: &str,
    csv_utf8: &str,
) -> SeoResult<(usize, usize)> {
    let mut reader = ReaderBuilder::new()
        .has_headers(true)
        .flexible(false)
        .from_reader(csv_utf8.as_bytes());
    let headers = reader
        .headers()
        .map_err(|error| SeoError::validation(format!("failed to read CSV headers: {error}")))?
        .clone();
    validate_csv_headers(&headers)?;
    let next_byte_offset = reader.position().byte() as usize;
    let mut total_rows = 0_usize;
    for (index, result) in reader.records().enumerate() {
        let record = result.map_err(|error| {
            SeoError::validation(format!("failed to read CSV row {}: {error}", index + 2))
        })?;
        parse_bulk_csv_record(&record, &expected_kind, expected_locale, index + 2)?;
        total_rows += 1;
    }
    Ok((next_byte_offset, total_rows))
}

fn read_bulk_import_chunk(payload: &QueuedBulkImportPayload) -> SeoResult<BulkImportChunk> {
    let bytes = payload.input.csv_utf8.as_bytes();
    if payload.next_byte_offset >= bytes.len() {
        return Ok(BulkImportChunk {
            rows: Vec::new(),
            next_byte_offset: bytes.len(),
            next_row_number: payload.next_row_number,
            exhausted: true,
        });
    }

    let mut reader = ReaderBuilder::new()
        .has_headers(false)
        .flexible(false)
        .from_reader(&bytes[payload.next_byte_offset..]);
    let mut rows = Vec::new();
    let mut next_row_number = payload.next_row_number;
    {
        let mut records = reader.records();
        while rows.len() < BULK_IO_CHUNK_SIZE {
            let Some(result) = records.next() else {
                break;
            };
            let record = result.map_err(|error| {
                SeoError::validation(format!(
                    "failed to read CSV row {next_row_number}: {error}"
                ))
            })?;
            rows.push(parse_bulk_csv_record(
                &record,
                &payload.input.target_kind,
                payload.input.locale.as_str(),
                next_row_number,
            )?);
            next_row_number += 1;
        }
    }
    let next_byte_offset = payload.next_byte_offset + reader.position().byte() as usize;
    Ok(BulkImportChunk {
        rows,
        next_byte_offset,
        next_row_number,
        exhausted: next_byte_offset >= bytes.len(),
    })
}

fn advance_legacy_import_cursor(
    mut payload: QueuedBulkImportPayload,
    processed_rows: usize,
) -> SeoResult<QueuedBulkImportPayload> {
    let bytes = payload.input.csv_utf8.as_bytes();
    if processed_rows == 0 || payload.next_byte_offset >= bytes.len() {
        return Ok(payload);
    }

    let mut reader = ReaderBuilder::new()
        .has_headers(false)
        .flexible(false)
        .from_reader(&bytes[payload.next_byte_offset..]);
    let mut skipped = 0_usize;
    {
        let mut records = reader.records();
        while skipped < processed_rows {
            let Some(result) = records.next() else {
                break;
            };
            result.map_err(|error| {
                SeoError::validation(format!(
                    "failed to resume legacy bulk import at row {}: {error}",
                    payload.next_row_number + skipped
                ))
            })?;
            skipped += 1;
        }
    }
    payload.next_byte_offset += reader.position().byte() as usize;
    payload.next_row_number += skipped;
    Ok(payload)
}

#[allow(clippy::too_many_arguments)]
fn resolve_bulk_io_projection(
    tenant: &TenantContext,
    target_kind: SeoTargetSlug,
    target_id: Uuid,
    requested_locale: &str,
    explicit: Option<LoadedMeta>,
    state: Option<TargetState>,
    settings: &SeoModuleSettings,
) -> Option<super::bulk_read_model::BulkReadProjection> {
    match (explicit, state) {
        (Some(explicit), Some(state)) => {
            let resolved = resolve_by_locale_with_fallback(
                explicit.translations.as_slice(),
                state.effective_locale.as_str(),
                Some(tenant.default_locale.as_str()),
                |item| item.locale.as_str(),
            );
            let translation = resolved.item.cloned();
            Some(super::bulk_read_model::BulkReadProjection {
                effective_locale: resolved.effective_locale,
                source: SeoBulkSource::Explicit,
                title: translation
                    .as_ref()
                    .and_then(|item| trimmed_option(item.title.clone()))
                    .or(Some(state.title)),
                description: translation
                    .as_ref()
                    .and_then(|item| trimmed_option(item.description.clone()))
                    .or(state.description),
                keywords: translation
                    .as_ref()
                    .and_then(|item| trimmed_option(item.keywords.clone())),
                canonical_url: explicit.meta.canonical_url,
                og_title: translation
                    .as_ref()
                    .and_then(|item| trimmed_option(item.og_title.clone())),
                og_description: translation
                    .as_ref()
                    .and_then(|item| trimmed_option(item.og_description.clone())),
                og_image: translation
                    .as_ref()
                    .and_then(|item| trimmed_option(item.og_image.clone())),
                structured_data: explicit.meta.structured_data,
                noindex: explicit.meta.no_index,
                nofollow: explicit.meta.no_follow,
            })
        }
        (Some(explicit), None) => {
            let resolved = resolve_by_locale_with_fallback(
                explicit.translations.as_slice(),
                requested_locale,
                Some(tenant.default_locale.as_str()),
                |item| item.locale.as_str(),
            );
            let translation = resolved.item.cloned();
            Some(super::bulk_read_model::BulkReadProjection {
                effective_locale: resolved.effective_locale,
                source: SeoBulkSource::Explicit,
                title: translation.as_ref().and_then(|item| item.title.clone()),
                description: translation
                    .as_ref()
                    .and_then(|item| item.description.clone()),
                keywords: translation.as_ref().and_then(|item| item.keywords.clone()),
                canonical_url: explicit.meta.canonical_url,
                og_title: translation.as_ref().and_then(|item| item.og_title.clone()),
                og_description: translation
                    .as_ref()
                    .and_then(|item| item.og_description.clone()),
                og_image: translation.as_ref().and_then(|item| item.og_image.clone()),
                structured_data: explicit.meta.structured_data,
                noindex: explicit.meta.no_index,
                nofollow: explicit.meta.no_follow,
            })
        }
        (None, Some(state)) => {
            debug_assert_eq!(state.target_kind, target_kind);
            debug_assert_eq!(state.target_id, target_id);
            let generated = render_generated_record(
                &state,
                &settings.template_defaults,
                settings.template_overrides.get(state.target_kind.as_str()),
            );
            let generated_source = generated.title.is_some()
                || generated.description.is_some()
                || generated.canonical_url.is_some()
                || generated.keywords.is_some()
                || generated.og_title.is_some()
                || generated.og_description.is_some()
                || generated.robots.is_some()
                || generated.twitter_title.is_some()
                || generated.twitter_description.is_some();
            Some(super::bulk_read_model::BulkReadProjection {
                effective_locale: state.effective_locale,
                source: if generated_source {
                    SeoBulkSource::Generated
                } else {
                    SeoBulkSource::Fallback
                },
                title: generated.title.or(Some(state.title)),
                description: generated.description.or(state.description),
                keywords: generated.keywords,
                canonical_url: generated.canonical_url,
                og_title: generated.og_title.or(state.open_graph.title.clone()),
                og_description: generated
                    .og_description
                    .or(state.open_graph.description.clone()),
                og_image: first_open_graph_image_url(&state.open_graph),
                structured_data: Some(state.structured_data),
                noindex: false,
                nofollow: false,
            })
        }
        (None, None) => None,
    }
}

#[cfg(test)]
mod bounded_io_execution_tests {
    use super::*;

    #[test]
    fn io_chunk_size_stays_bounded() {
        assert_eq!(BULK_IO_CHUNK_SIZE, 50);
    }

    #[test]
    fn import_scan_keeps_a_byte_cursor_after_headers() {
        let csv = "target_kind,target_id,locale,title,description,keywords,canonical_url,og_title,og_description,og_image,structured_data,noindex,nofollow\npage,11111111-1111-1111-1111-111111111111,en-US,Title,,,,,,,,false,false\n";
        let (offset, rows) = scan_bulk_import_csv(
            SeoTargetSlug::new("page").expect("valid target kind"),
            "en-US",
            csv,
        )
        .expect("scan import");
        assert!(offset > 0);
        assert_eq!(rows, 1);
    }
}
